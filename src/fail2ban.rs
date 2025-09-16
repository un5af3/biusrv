use std::collections::HashMap;

use anyhow::{anyhow, Result};

use crate::config::{Fail2banConfig, Fail2banJailConfig};
use crate::ssh::{CommandResult, Session};
use crate::utils;

/// Install and setup fail2ban
pub async fn setup(session: &Session, backend: Option<&str>) -> Result<CommandResult> {
    // Check if fail2ban is installed
    let check_result = session.execute_with_sudo("which fail2ban-client").await?;
    if check_result.exit_status != 0 {
        utils::install(session, "fail2ban").await?;
    }

    let backend = backend.unwrap_or("systemd");
    if set_backend(session, backend).await?.exit_status != 0 {
        return Err(anyhow!("Fail2ban set backend failed"));
    }

    utils::enable_service(session, "fail2ban").await?;

    let status_result = utils::service_status(session, "fail2ban").await?;
    if status_result.exit_status != 0 {
        utils::start_service(session, "fail2ban").await?;
    }

    Ok(status_result)
}

pub async fn set_backend(session: &Session, backend: &str) -> Result<CommandResult> {
    let result = session
        .execute_with_sudo(&format!(
            "sed -i 's/^backend = auto/backend = {}/' /etc/fail2ban/jail.conf",
            backend
        ))
        .await?;
    Ok(result)
}

/// Configure fail2ban with the given configuration
pub async fn configure(session: &Session, config: &Fail2banConfig) -> Result<()> {
    // If content is specified, use it directly and ignore jail config
    if let Some(ref content) = config.content {
        configure_with_content(session, content).await?;
    } else if let Some(ref jails) = config.jail {
        configure_jails(session, jails).await?;
    } else {
        return Err(anyhow!("No content or jail config provided"));
    }

    // Reload fail2ban to apply changes
    let result = reload(session).await?;
    if result.exit_status != 0 {
        return Err(anyhow!("Fail2ban reload failed"));
    }

    Ok(())
}

/// Configure fail2ban with custom content
async fn configure_with_content(session: &Session, content: &str) -> Result<()> {
    let config_file = "/etc/fail2ban/jail.d/biusrv.conf";

    // Create the configuration file
    utils::create_file(session, config_file, content, Some("644")).await?;

    // Verify content was written correctly
    let verify_cmd = format!("cat {}", config_file);
    let result = session.execute_with_sudo(&verify_cmd).await?;
    if !result.output.contains(content) {
        return Err(anyhow!("Fail2ban config verification failed"));
    }

    Ok(())
}

/// Configure a specific jail
async fn configure_jails(
    session: &Session,
    jails: &HashMap<String, Fail2banJailConfig>,
) -> Result<()> {
    let config_file = "/etc/fail2ban/jail.d/biusrv.conf";
    let mut content = String::new();

    for (jail_name, jail_config) in jails {
        content.push_str(&format!("[{}]\n", jail_name));
        content.push_str(&format!("enabled = {}\n", jail_config.enabled));
        content.push_str(&format!("port = {}\n", jail_config.port));
        content.push_str(&format!("filter = {}\n", jail_config.filter));
        content.push_str(&format!("maxretry = {}\n", jail_config.maxretry));
        content.push_str(&format!("findtime = {}\n", jail_config.findtime));
        content.push_str(&format!("bantime = {}\n", jail_config.bantime));
        if let Some(ref ignoreip) = jail_config.ignoreip {
            content.push_str(&format!("ignoreip = {}\n", ignoreip.join(" ")));
        }
        if let Some(ref logpath) = jail_config.logpath {
            content.push_str(&format!("logpath = {}\n", logpath));
        }
        if let Some(ref options) = jail_config.options {
            for (key, value) in options {
                content.push_str(&format!("{} = {}\n", key, value));
            }
        }
        content.push_str("\n");
    }

    utils::create_file(session, config_file, content.trim(), Some("644")).await?;

    let verify_cmd = format!("cat {}", config_file);
    let result = session.execute_with_sudo(&verify_cmd).await?;
    if !result.output.contains(content.trim()) {
        return Err(anyhow!("Fail2ban config verification failed"));
    }

    Ok(())
}

/// Reload fail2ban configuration
pub async fn reload(session: &Session) -> Result<CommandResult> {
    let result = session.execute_with_sudo("fail2ban-client reload").await?;
    Ok(result)
}

/// Get fail2ban status
pub async fn status(session: &Session) -> Result<CommandResult> {
    let result = session.execute_with_sudo("fail2ban-client status").await?;
    Ok(result)
}

/// Get status of a specific jail
pub async fn jail_status(session: &Session, jail_name: &str) -> Result<CommandResult> {
    let result = session
        .execute_with_sudo(&format!("fail2ban-client status {}", jail_name))
        .await?;
    Ok(result)
}

/// Unban an IP address from a specific jail
pub async fn unban_ip(session: &Session, jail_name: &str, ip: &str) -> Result<()> {
    let cmd = format!("fail2ban-client set {} unbanip {}", jail_name, ip);
    session.execute_with_sudo(&cmd).await?;

    // Verify IP was unbanned
    let verify_result = session
        .execute_with_sudo(&format!(
            "fail2ban-client status {} | grep {}",
            jail_name, ip
        ))
        .await?;
    if verify_result.exit_status == 0 {
        return Err(anyhow!("IP {} is still banned in jail {}", ip, jail_name));
    }

    Ok(())
}

/// Ban an IP address in a specific jail
pub async fn ban_ip(session: &Session, jail_name: &str, ip: &str) -> Result<()> {
    let cmd = format!("fail2ban-client set {} banip {}", jail_name, ip);
    session.execute_with_sudo(&cmd).await?;

    // Verify IP was banned
    let verify_result = session
        .execute_with_sudo(&format!(
            "fail2ban-client status {} | grep {}",
            jail_name, ip
        ))
        .await?;
    if verify_result.exit_status != 0 {
        return Err(anyhow!("IP {} was not banned in jail {}", ip, jail_name));
    }

    Ok(())
}
