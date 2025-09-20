use std::collections::HashSet;

use anyhow::{anyhow, Result};

use crate::config::{Fail2banConfig, FirewallConfig, InitConfig, SshdConfig};
use crate::fail2ban;
use crate::firewall;
use crate::ssh::{CommandResult, OsType, Session};
use crate::utils::{self, truncate_error_message};

#[derive(Debug)]
pub struct InitServer {
    new_username: String,
    new_password: String,

    pub sshd_config: Option<SshdConfig>,
    pub firewall_config: Option<FirewallConfig>,
    pub fail2ban_config: Option<Fail2banConfig>,

    pub packages: Option<Vec<String>>,
    pub commands: Option<Vec<String>>,
}

impl InitServer {
    pub fn new(init_config: &InitConfig) -> Self {
        Self {
            new_username: init_config.new_username.clone(),
            new_password: init_config.new_password.clone(),
            sshd_config: init_config.sshd.clone(),
            firewall_config: init_config.firewall.clone(),
            fail2ban_config: init_config.fail2ban.clone(),
            packages: init_config.packages.clone(),
            commands: init_config.commands.clone(),
        }
    }

    pub async fn update_system(&self, session: &Session) -> Result<()> {
        let result = utils::update_system(session).await?;
        if result.exit_status != 0 {
            return Err(anyhow!(
                "Failed to update system (exit code: {}) - {}",
                result.exit_status,
                truncate_error_message(&result.output.trim(), 3)
            ));
        }
        Ok(())
    }

    pub async fn create_user(&self, session: &Session) -> Result<()> {
        //let create_cmd = format!("useradd -m -s /bin/bash {}", self.new_username);
        let create_cmd = format!("useradd -m {}", self.new_username);
        session.execute_with_sudo(&create_cmd).await?;

        // verify if user is created
        let verify_cmd = format!("id {}", self.new_username);
        let result = session.execute_with_sudo(&verify_cmd).await?;
        if result.exit_status != 0 {
            return Err(anyhow!(
                "User verification failed (exit code: {}) - {}",
                result.exit_status,
                truncate_error_message(&result.output.trim(), 3)
            ));
        }

        let password_cmd = format!(
            "echo '{}:{}' | chpasswd",
            self.new_username, self.new_password
        );
        session.execute_with_sudo(&password_cmd).await?;

        // verify if password is set, use passwd -S to check
        let verify_cmd = format!("passwd -S {}", self.new_username);
        let result = session.execute_with_sudo(&verify_cmd).await?;
        if !result
            .output
            .contains(format!("{} P", self.new_username).as_str())
        {
            return Err(anyhow!("Password verification failed: {}", result.output));
        }

        Ok(())
    }

    pub async fn install_required(&self, session: &Session) -> Result<()> {
        let mut packages = HashSet::new();
        packages.insert("sudo");

        if self.fail2ban_config.is_some() {
            packages.insert("fail2ban");
        }

        if self.firewall_config.is_some() {
            match session.os_type() {
                OsType::Debian => {
                    packages.insert("iptables-persistent");
                }
                OsType::RedHat => {
                    packages.insert("iptables-services");
                }
                _ => {}
            }
        }

        if let Some(ref pkgs) = self.packages {
            // remove duplicates
            packages.extend(pkgs.iter().map(|s| s.trim()));
        }

        let packages = packages.into_iter().map(|s| s).collect::<Vec<_>>();

        let result = utils::install_packages(session, &packages).await?;
        if result.exit_status != 0 {
            return Err(anyhow!(
                "Failed to install packages (exit code: {}) - {}",
                result.exit_status,
                truncate_error_message(&result.output.trim(), 3)
            ));
        }

        Ok(())
    }

    pub async fn setup_sudo(&self, session: &Session) -> Result<()> {
        // check sudo command exists
        let sudo_cmd = "which sudo";
        let result = session.execute_with_sudo(sudo_cmd).await?;
        if result.exit_status != 0 {
            utils::install(session, "sudo").await?;
        }

        let sudo_cmd = format!(
            "echo '{} ALL=(ALL) NOPASSWD:ALL' > /etc/sudoers.d/{}",
            self.new_username, self.new_username
        );
        session.execute_with_sudo(&sudo_cmd).await?;

        // verify sudo configuration
        let verify_cmd = format!(
            "grep '{} ALL=(ALL) NOPASSWD:ALL' /etc/sudoers.d/{}",
            self.new_username, self.new_username
        );
        let result = session.execute_with_sudo(&verify_cmd).await?;
        if result.exit_status != 0 {
            return Err(anyhow!(
                "Sudo configuration verification failed (exit code: {}) - {}",
                result.exit_status,
                truncate_error_message(&result.output.trim(), 3)
            ));
        }

        Ok(())
    }

    pub async fn setup_firewall(
        &self,
        session: &Session,
        ssh_port: u16,
        config: &FirewallConfig,
    ) -> Result<()> {
        // Setup firewall
        firewall::setup(session, ssh_port, config).await?;

        // Save firewall rules
        firewall::save_rules(session).await?;

        Ok(())
    }

    pub async fn setup_fail2ban(&self, session: &Session, config: &Fail2banConfig) -> Result<()> {
        // Install and start fail2ban
        fail2ban::setup(session, config.backend.as_deref()).await?;

        // Configure fail2ban
        fail2ban::configure(session, config).await?;

        Ok(())
    }

    pub async fn reload_sshd(&self, session: &Session) -> Result<CommandResult> {
        // try two ways to reload sshd
        let mut result = session.execute_with_sudo("systemctl reload sshd").await?;
        if result.exit_status != 0 {
            result = session.execute_with_sudo("service ssh reload").await?;
            if result.exit_status != 0 {
                return Err(anyhow!(
                    "Failed to reload sshd (exit code: {}) - {}",
                    result.exit_status,
                    truncate_error_message(&result.output.trim(), 3)
                ));
            }
        }

        Ok(result)
    }

    pub async fn configure_sshd(&self, session: &Session, config: &SshdConfig) -> Result<()> {
        let config_file = "/etc/ssh/sshd_config.d/biusrv.conf";
        let mut config_content = String::new();

        // First: Add public key to authorized_keys (priority 1)
        if let Some(ref public_key) = config.public_key {
            let ssh_dir = format!("/home/{}/.ssh", self.new_username);
            let auth_file = format!("{}/authorized_keys", ssh_dir);

            // Create .ssh directory and set permissions
            utils::create_dir(session, &ssh_dir, Some("700")).await?;

            // Add public key and set file permissions
            utils::create_file(session, &auth_file, public_key, Some("600")).await?;

            // Set ownership for both directory and file
            let chown_cmd = format!(
                "chown {}:{} {} && chown {}:{} {}",
                self.new_username,
                self.new_username,
                ssh_dir,
                self.new_username,
                self.new_username,
                auth_file
            );
            session.execute_with_sudo(&chown_cmd).await?;

            // Verify public key was added correctly
            let verify_cmd = format!("cat {}", auth_file);
            let result = session.execute_with_sudo(&verify_cmd).await?;
            if !result.output.contains(public_key) {
                return Err(anyhow!("Public key verification failed: {}", result.output));
            }
        }

        // Second: Configure SSH settings (priority 2)
        // Change SSH port
        if let Some(port) = config.new_port {
            config_content.push_str(&format!("Port {}\n", port));
        }

        // Apply SSH configuration options
        if let Some(ref sshd_options) = config.options {
            for (key, value) in sshd_options {
                config_content.push_str(&format!("{} {}\n", key, value));
            }
        }

        // Write configuration to file
        if !config_content.is_empty() {
            utils::create_file(session, config_file, config_content.trim(), Some("644")).await?;

            // Verify content was written correctly
            let verify_cmd = format!("cat {}", config_file);
            let result = session.execute_with_sudo(&verify_cmd).await?;
            if !result.output.contains(config_content.trim()) {
                return Err(anyhow!("SSH config verification failed: {}", result.output));
            }
        }

        Ok(())
    }

    pub async fn execute_custom_commands(
        &self,
        session: &Session,
        commands: &[String],
    ) -> Result<()> {
        for cmd in commands {
            let result = session.execute_with_sudo(&cmd).await?;

            if result.exit_status != 0 {
                return Err(anyhow!(
                    "Failed to execute command '{}' (exit code: {}) - {}",
                    cmd,
                    result.exit_status,
                    truncate_error_message(&result.output.trim(), 3)
                ));
            }
        }

        Ok(())
    }
}
