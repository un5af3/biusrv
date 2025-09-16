use anyhow::{anyhow, Result};

use crate::ssh::Session;
use crate::utils;

/// Install and setup ufw
pub async fn setup(session: &Session) -> Result<()> {
    // Check if ufw is installed
    let check_result = session.execute_with_sudo("which ufw").await?;
    if check_result.exit_status != 0 {
        utils::install(session, "ufw").await?;
    }

    // Enable ufw (this will also start it)
    session.execute_with_sudo("ufw --force enable").await?;

    // Verify ufw is active
    let verify_result = session.execute_with_sudo("ufw status").await?;
    if !verify_result.output.contains("Status: active") {
        return Err(anyhow!("UFW is not active"));
    }

    Ok(())
}

/// Allow a port
pub async fn allow_port(session: &Session, port_spec: &str) -> Result<()> {
    let cmd = format!("ufw allow {}", port_spec);
    session.execute_with_sudo(&cmd).await?;

    // Verify port was allowed
    let verify_result = session
        .execute_with_sudo(&format!("ufw status | grep '{}.*ALLOW'", port_spec))
        .await?;
    if verify_result.exit_status != 0 {
        return Err(anyhow!("Port {} was not allowed successfully", port_spec));
    }

    Ok(())
}

/// Deny a port
pub async fn deny_port(session: &Session, port_spec: &str) -> Result<()> {
    let cmd = format!("ufw deny {}", port_spec);
    session.execute_with_sudo(&cmd).await?;

    // Verify port was denied
    let verify_result = session
        .execute_with_sudo(&format!("ufw status | grep '{}.*DENY'", port_spec))
        .await?;
    if verify_result.exit_status != 0 {
        return Err(anyhow!("Port {} was not denied successfully", port_spec));
    }

    Ok(())
}

/// Get ufw status
pub async fn status(session: &Session) -> Result<String> {
    let result = session.execute_with_sudo("ufw status").await?;
    if result.exit_status != 0 {
        return Err(anyhow!("Failed to get ufw status"));
    }
    Ok(result.output)
}

/// Allow multiple ports
pub async fn allow_ports<S: AsRef<str>>(session: &Session, port_specs: &[S]) -> Result<()> {
    if port_specs.is_empty() {
        return Ok(());
    }

    // Build command with semicolon-separated ufw allow commands
    let cmd = port_specs
        .iter()
        .map(|port| format!("ufw allow {}", port.as_ref()))
        .collect::<Vec<_>>()
        .join("; ");

    session.execute_with_sudo(&cmd).await?;

    // Verify all ports were allowed
    let existing_ports = list_allowed_ports(session).await?;
    let port_strings: Vec<String> = port_specs.iter().map(|p| p.as_ref().to_string()).collect();
    let missing_ports: Vec<&String> = port_strings
        .iter()
        .filter(|port| !existing_ports.contains(port))
        .collect();

    if !missing_ports.is_empty() {
        return Err(anyhow!(
            "Ports {:?} were not allowed successfully",
            missing_ports
        ));
    }

    Ok(())
}

/// Deny multiple ports
pub async fn deny_ports<S: AsRef<str>>(session: &Session, port_specs: &[S]) -> Result<()> {
    if port_specs.is_empty() {
        return Ok(());
    }

    // Build command with semicolon-separated ufw deny commands
    let cmd = port_specs
        .iter()
        .map(|port| format!("ufw deny {}", port.as_ref()))
        .collect::<Vec<_>>()
        .join("; ");

    session.execute_with_sudo(&cmd).await?;

    // Verify all ports were denied
    let existing_ports = list_denied_ports(session).await?;
    let port_strings: Vec<String> = port_specs.iter().map(|p| p.as_ref().to_string()).collect();
    let missing_ports: Vec<&String> = port_strings
        .iter()
        .filter(|port| !existing_ports.contains(port))
        .collect();

    if !missing_ports.is_empty() {
        return Err(anyhow!(
            "Ports {:?} were not denied successfully",
            missing_ports
        ));
    }

    Ok(())
}

/// List only allowed ports
pub async fn list_allowed_ports(session: &Session) -> Result<Vec<String>> {
    let result = session.execute_with_sudo("ufw status").await?;

    if result.exit_status != 0 {
        return Err(anyhow!("Failed to list ports"));
    }

    // Parse ufw status output to extract only ALLOW port rules
    let mut ports = Vec::new();
    for line in result.output.lines() {
        if line.contains("ALLOW") {
            // Extract port from lines like "22/tcp                   ALLOW       Anywhere"
            if let Some(port_part) = line.split_whitespace().next() {
                if port_part.contains("/") || port_part.parse::<u16>().is_ok() {
                    ports.push(port_part.to_string());
                }
            }
        }
    }

    Ok(ports)
}

/// List only denied ports
pub async fn list_denied_ports(session: &Session) -> Result<Vec<String>> {
    let result = session.execute_with_sudo("ufw status").await?;

    if result.exit_status != 0 {
        return Err(anyhow!("Failed to list ports"));
    }

    // Parse ufw status output to extract only DENY port rules
    let mut ports = Vec::new();
    for line in result.output.lines() {
        if line.contains("DENY") {
            // Extract port from lines like "22/tcp                   DENY        Anywhere"
            if let Some(port_part) = line.split_whitespace().next() {
                if port_part.contains("/") || port_part.parse::<u16>().is_ok() {
                    ports.push(port_part.to_string());
                }
            }
        }
    }

    Ok(ports)
}

/// Delete a port
pub async fn delete_port(session: &Session, allow: bool, port_spec: &str) -> Result<()> {
    let cmd = format!(
        "ufw delete {} {}",
        if allow { "allow" } else { "deny" },
        port_spec
    );
    session.execute_with_sudo(&cmd).await?;

    Ok(())
}

/// Delete multiple ports
pub async fn delete_ports<S: AsRef<str>>(
    session: &Session,
    allow: bool,
    port_specs: &[S],
) -> Result<()> {
    if port_specs.is_empty() {
        return Ok(());
    }

    let cmd = port_specs
        .iter()
        .map(|port| {
            format!(
                "ufw delete {} {}",
                if allow { "allow" } else { "deny" },
                port.as_ref()
            )
        })
        .collect::<Vec<_>>()
        .join("; ");

    session.execute_with_sudo(&cmd).await?;

    let existing_ports = if allow {
        list_allowed_ports(session).await?
    } else {
        list_denied_ports(session).await?
    };

    let port_strings: Vec<String> = port_specs.iter().map(|p| p.as_ref().to_string()).collect();
    let remaining_ports: Vec<&String> = port_strings
        .iter()
        .filter(|port| existing_ports.contains(port))
        .collect();
    if !remaining_ports.is_empty() {
        return Err(anyhow!(
            "Ports {:?} were not deleted successfully",
            remaining_ports
        ));
    }

    Ok(())
}
