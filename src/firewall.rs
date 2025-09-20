use std::collections::HashSet;

use anyhow::{anyhow, Result};

use crate::config::{FirewallConfig, FirewallPolicy};
use crate::ssh::{OsType, Session};
use crate::utils::{self, truncate_error_message};

/// Parse port specification (e.g., "80/tcp", "53/udp", "22", "1234:4567/tcp")
fn parse_port_spec(port_spec: &str) -> Result<(String, String)> {
    let (port_str, protocol) = if let Some(slash_pos) = port_spec.find('/') {
        let port_str = &port_spec[..slash_pos];
        let protocol = port_spec[slash_pos + 1..].trim().to_lowercase();

        if protocol != "tcp" && protocol != "udp" {
            return Err(anyhow!(
                "Invalid protocol: {}. Must be 'tcp' or 'udp'",
                protocol
            ));
        }

        (port_str, protocol)
    } else {
        (port_spec, "tcp".to_string())
    };

    let port_str = if let Some(colon_pos) = port_str.find(":") {
        let start_port = port_str[..colon_pos]
            .parse::<u16>()
            .map_err(|_| anyhow!("Invalid port number: {}", port_str))?;
        let end_port = port_str[colon_pos + 1..]
            .parse::<u16>()
            .map_err(|_| anyhow!("Invalid port number: {}", port_str))?;
        if start_port > end_port {
            return Err(anyhow!("Invalid port range: {} - {}", start_port, end_port));
        }
        format!("{}:{}", start_port, end_port)
    } else {
        let port = port_str
            .parse::<u16>()
            .map_err(|_| anyhow!("Invalid port number: {}", port_str))?;
        format!("{}", port)
    };

    Ok((port_str, protocol))
}

/// Setup iptables with basic rules
pub async fn setup(session: &Session, ssh_port: u16, config: &FirewallConfig) -> Result<()> {
    // Check if iptables is available
    let check_result = session.execute_with_sudo("which iptables").await?;
    if check_result.exit_status != 0 {
        return Err(anyhow!("iptables is not available on this system"));
    }

    // Set default policies
    session
        .execute_with_sudo("iptables -P INPUT ACCEPT")
        .await?;
    session
        .execute_with_sudo("iptables -P FORWARD ACCEPT")
        .await?;
    session
        .execute_with_sudo("iptables -P OUTPUT ACCEPT")
        .await?;

    // Flush and Delete existing rules
    session.execute_with_sudo("iptables -F").await?;
    session.execute_with_sudo("iptables -X").await?;

    // Setup firewall based on policy
    match config.policy {
        FirewallPolicy::Whitelist => setup_whitelist(session, ssh_port, config).await?,
        FirewallPolicy::Blacklist => setup_blacklist(session, ssh_port, config).await?,
    }

    Ok(())
}

async fn setup_whitelist(session: &Session, ssh_port: u16, config: &FirewallConfig) -> Result<()> {
    // Allow loopback
    session
        .execute_with_sudo("iptables -A INPUT -i lo -j ACCEPT")
        .await?;

    // Allow established and related connections
    session
        .execute_with_sudo("iptables -A INPUT -m state --state ESTABLISHED,RELATED -j ACCEPT")
        .await?;

    // Allow SSH (port 22) by default to prevent lockout
    session
        .execute_with_sudo(&format!(
            "iptables -A INPUT -p tcp --dport {} -j ACCEPT",
            ssh_port
        ))
        .await?;

    // Set ICMP rules
    if config.enable_icmp {
        session
            .execute_with_sudo("iptables -A INPUT -p icmp -j ACCEPT")
            .await?;
    } else {
        if let Some(allow_ping) = config.allow_ping {
            if allow_ping {
                session
                    .execute_with_sudo(
                        "iptables -A INPUT -p icmp --icmp-type echo-request -j ACCEPT",
                    )
                    .await?;
            }
        }
    }

    // Set allowed ports
    if let Some(ref allow_ports) = config.allow_ports {
        let mut chk_list = HashSet::new();
        chk_list.insert((ssh_port.to_string(), "tcp".to_string()));
        for port_spec in allow_ports.iter() {
            let (port, protocol) = parse_port_spec(port_spec)?;
            if chk_list.insert((port.clone(), protocol.clone())) {
                session
                    .execute_with_sudo(&format!(
                        "iptables -A INPUT -p {} --dport {} -j ACCEPT",
                        protocol, port
                    ))
                    .await?;
            }
        }
    }

    // Set restrictive default policies
    session.execute_with_sudo("iptables -P INPUT DROP").await?;
    session
        .execute_with_sudo("iptables -P FORWARD DROP")
        .await?;

    Ok(())
}

async fn setup_blacklist(session: &Session, ssh_port: u16, config: &FirewallConfig) -> Result<()> {
    // Set ICMP rules
    if !config.enable_icmp {
        session
            .execute_with_sudo("iptables -A INPUT -p icmp -j DROP")
            .await?;
    } else {
        if let Some(allow_ping) = config.allow_ping {
            if !allow_ping {
                session
                    .execute_with_sudo("iptables -A INPUT -p icmp --icmp-type echo-request -j DROP")
                    .await?;
            }
        }
    }

    // Set denied ports (protect SSH port from being denied)
    if let Some(ref deny_ports) = config.deny_ports {
        let mut chk_list = HashSet::new();
        chk_list.insert((ssh_port.to_string(), "tcp".to_string()));
        for port_spec in deny_ports.iter() {
            let (port, protocol) = parse_port_spec(port_spec)?;
            if chk_list.insert((port.clone(), protocol.clone())) {
                session
                    .execute_with_sudo(&format!(
                        "iptables -A INPUT -p {} --dport {} -j DROP",
                        protocol, port
                    ))
                    .await?;
            }
        }
    }

    Ok(())
}

/// Save iptables rules to make them persistent across reboots
pub async fn save_rules(session: &Session) -> Result<()> {
    match session.os_type() {
        OsType::Debian => save_rules_debian(session).await,
        OsType::RedHat => save_rules_redhat(session).await,
        OsType::Arch => save_rules_arch(session).await,
    }
}

async fn save_rules_debian(session: &Session) -> Result<()> {
    // Try netfilter-persistent first (best for Debian/Ubuntu)
    let check_result = session
        .execute_with_sudo("which netfilter-persistent")
        .await?;
    if check_result.exit_status != 0 {
        // Try to install iptables-persistent
        let install_result = utils::install(&session, "iptables-persistent").await?;
        if install_result.exit_status != 0 {
            return Err(anyhow!(
                "Failed to install iptables-persistent (exit code: {}) - {}",
                install_result.exit_status,
                truncate_error_message(&install_result.output.trim(), 3)
            ));
        }
    }

    let result = session
        .execute_with_sudo("netfilter-persistent save")
        .await?;

    if result.exit_status != 0 {
        return Err(anyhow!(
            "Failed to save iptables rules (exit code: {}) - {}",
            result.exit_status,
            truncate_error_message(&result.output.trim(), 3)
        ));
    }

    Ok(())
}

async fn save_rules_redhat(session: &Session) -> Result<()> {
    // check firewalld
    let check_result = session
        .execute_with_sudo("systemctl is-active firewalld")
        .await?;
    if check_result.exit_status == 0 {
        utils::stop_service(&session, "firewalld").await?;
    }

    // Try to enable iptables service
    utils::enable_service(&session, "iptables").await?;

    let save_result = session.execute_with_sudo("serivce iptables save").await?;
    if save_result.exit_status == 0 {
        return Ok(());
    }

    // Try multiple save locations
    let save_commands = [
        "iptables-save > /etc/sysconfig/iptables", // Traditional RedHat/CentOS
        "iptables-save > /etc/iptables/iptables.rules", // Modern systemd location
        "iptables-save > /etc/iptables/rules.v4",  // Alternative location
    ];

    let save_result = session
        .execute_with_sudo(&save_commands.join(" || "))
        .await?;
    if save_result.exit_status != 0 {
        return Err(anyhow!(
            "Failed to save iptables rules (exit code: {}) - {}",
            save_result.exit_status,
            truncate_error_message(&save_result.output.trim(), 3)
        ));
    }

    Ok(())
}

async fn save_rules_arch(session: &Session) -> Result<()> {
    // Arch Linux typically uses iptables-save/restore
    session
        .execute_with_sudo("iptables-save > /etc/iptables/iptables.rules")
        .await?;

    // Enable iptables service
    utils::enable_service(&session, "iptables").await?;

    Ok(())
}

/// Get iptables status
pub async fn status(session: &Session) -> Result<String> {
    let result = session.execute_with_sudo("iptables -L -n -v").await?;
    if result.exit_status != 0 {
        return Err(anyhow!(
            "Failed to get iptables status (exit code: {}) - {}",
            result.exit_status,
            truncate_error_message(&result.output.trim(), 3)
        ));
    }
    Ok(result.output)
}

/// Allow a port
pub async fn allow_port(session: &Session, port_spec: &str) -> Result<()> {
    let (port, protocol) = parse_port_spec(port_spec)?;

    // Check if rule already exists
    let check_cmd = format!(
        "iptables -C INPUT -p {} --dport {} -j ACCEPT",
        protocol, port
    );
    let check_result = session.execute_with_sudo(&check_cmd).await?;

    if check_result.exit_status == 0 {
        // Rule already exists
        return Ok(());
    }

    // Add the rule
    let cmd = format!(
        "iptables -A INPUT -p {} --dport {} -j ACCEPT",
        protocol, port
    );
    let result = session.execute_with_sudo(&cmd).await?;

    if result.exit_status != 0 {
        return Err(anyhow!(
            "Port {} was not allowed successfully (exit code: {}) - {}",
            port_spec,
            result.exit_status,
            truncate_error_message(&result.output.trim(), 3)
        ));
    }

    Ok(())
}

/// Allow multiple ports
pub async fn allow_ports<S: AsRef<str>>(session: &Session, port_specs: &[S]) -> Result<()> {
    for port_spec in port_specs {
        if let Err(e) = allow_port(session, port_spec.as_ref()).await {
            return Err(e);
        }
    }

    Ok(())
}

/// Deny a port
pub async fn deny_port(session: &Session, port_spec: &str) -> Result<()> {
    let (port, protocol) = parse_port_spec(port_spec)?;

    // Check if rule already exists
    let check_cmd = format!("iptables -C INPUT -p {} --dport {} -j DROP", protocol, port);
    let check_result = session.execute_with_sudo(&check_cmd).await?;

    if check_result.exit_status == 0 {
        // Rule already exists
        return Ok(());
    }

    // Add the rule
    let cmd = format!("iptables -A INPUT -p {} --dport {} -j DROP", protocol, port);
    let result = session.execute_with_sudo(&cmd).await?;

    if result.exit_status != 0 {
        return Err(anyhow!(
            "Port {} was not denied successfully (exit code: {}) - {}",
            port_spec,
            result.exit_status,
            truncate_error_message(&result.output.trim(), 3)
        ));
    }

    Ok(())
}

/// Deny multiple ports
pub async fn deny_ports<S: AsRef<str>>(session: &Session, port_specs: &[S]) -> Result<()> {
    for port_spec in port_specs {
        if let Err(e) = deny_port(session, port_spec.as_ref()).await {
            return Err(e);
        }
    }

    Ok(())
}

/// Delete a port
pub async fn delete_port(session: &Session, allow: bool, port_spec: &str) -> Result<()> {
    let (port, protocol) = parse_port_spec(port_spec)?;
    let action = if allow { "ACCEPT" } else { "DROP" };

    let check_cmd = format!(
        "iptables -C INPUT -p {} --dport {} -j {}",
        protocol, port, action
    );
    let check_result = session.execute_with_sudo(&check_cmd).await?;
    if check_result.exit_status != 0 {
        return Ok(());
    }

    let delete_cmd = format!(
        "iptables -D INPUT -p {} --dport {} -j {}",
        protocol, port, action
    );
    let delete_result = session.execute_with_sudo(&delete_cmd).await?;
    if delete_result.exit_status != 0 {
        return Err(anyhow!(
            "Port {} was not deleted successfully (exit code: {}) - {}",
            port_spec,
            delete_result.exit_status,
            truncate_error_message(&delete_result.output.trim(), 3)
        ));
    }

    Ok(())
}

/// Delete multiple ports
pub async fn delete_ports<S: AsRef<str>>(
    session: &Session,
    allow: bool,
    port_specs: &[S],
) -> Result<()> {
    for port_spec in port_specs {
        if let Err(e) = delete_port(session, allow, port_spec.as_ref()).await {
            return Err(e);
        }
    }

    Ok(())
}
