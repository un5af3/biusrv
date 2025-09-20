use crate::ssh::{CommandResult, OsType, Session};
use anyhow::Result;
use base64::{engine::general_purpose, Engine as _};

pub async fn create_file(
    session: &Session,
    path: &str,
    content: &str,
    mode: Option<&str>,
) -> Result<CommandResult> {
    let encoded = general_purpose::STANDARD.encode(content.as_bytes());
    let command = if let Some(mode) = mode {
        format!(
            "echo '{}' | base64 -d > {} && chmod {} {}",
            encoded, path, mode, path
        )
    } else {
        format!("echo '{}' | base64 -d > {}", encoded, path)
    };
    session.execute_with_sudo(&command).await
}

pub async fn create_dir(
    session: &Session,
    path: &str,
    mode: Option<&str>,
) -> Result<CommandResult> {
    let command = if let Some(mode) = mode {
        format!("mkdir -p {} && chmod {} {}", path, mode, path)
    } else {
        format!("mkdir -p {}", path)
    };
    session.execute_with_sudo(&command).await
}

pub async fn install(session: &Session, package: &str) -> Result<CommandResult> {
    let command = match session.os_type() {
        OsType::Debian => format!("DEBIAN_FRONTEND=noninteractive apt install -y -o Dpkg::Options::=\"--force-confdef\" -o Dpkg::Options::=\"--force-confold\" {}", package),
        OsType::RedHat => format!("yum install -y {}", package),
        OsType::Arch => format!("pacman -S --noconfirm {}", package),
    };
    session.execute_with_sudo(&command).await
}

pub async fn install_packages(session: &Session, packages: &[&str]) -> Result<CommandResult> {
    let command = match session.os_type() {
        OsType::Debian => format!("DEBIAN_FRONTEND=noninteractive apt install -y -o Dpkg::Options::=\"--force-confdef\" -o Dpkg::Options::=\"--force-confold\" {}", packages.join(" ")),
        OsType::RedHat => format!("yum install -y {}", packages.join(" ")),
        OsType::Arch => format!("pacman -S --noconfirm {}", packages.join(" ")),
    };
    session.execute_with_sudo(&command).await
}

pub async fn uninstall(session: &Session, package: &str) -> Result<CommandResult> {
    let command = match session.os_type() {
        OsType::Debian => format!("apt remove -y {}", package),
        OsType::RedHat => format!("yum remove -y {}", package),
        OsType::Arch => format!("pacman -R --noconfirm {}", package),
    };
    session.execute_with_sudo(&command).await
}

pub async fn uninstall_packages(session: &Session, packages: &[&str]) -> Result<CommandResult> {
    let command = match session.os_type() {
        OsType::Debian => format!("apt remove -y {}", packages.join(" ")),
        OsType::RedHat => format!("yum remove -y {}", packages.join(" ")),
        OsType::Arch => format!("pacman -R --noconfirm {}", packages.join(" ")),
    };
    session.execute_with_sudo(&command).await
}

pub async fn update_system(session: &Session) -> Result<CommandResult> {
    let command = match session.os_type() {
        OsType::Debian => {
            r#"DEBIAN_FRONTEND=noninteractive apt update && apt upgrade -y -o Dpkg::Options::="--force-confdef" -o Dpkg::Options::="--force-confold""#
        }
        OsType::RedHat => "yum update -y",
        OsType::Arch => "pacman -Syu --noconfirm",
    };
    session
        .execute_with_sudo(&format!("{} > /tmp/update_system.log", command))
        .await
}

pub async fn enable_service(session: &Session, service: &str) -> Result<CommandResult> {
    let result = session
        .execute_with_sudo(&format!("systemctl enable {}", service))
        .await?;

    if result.exit_status != 0 {
        let next_result = match session.os_type() {
            OsType::Debian => {
                session
                    .execute_with_sudo(&format!("update-rc.d {} defaults", service))
                    .await?
            }
            OsType::RedHat => {
                session
                    .execute_with_sudo(&format!("chkconfig {} on", service))
                    .await?
            }
            OsType::Arch => {
                session
                    .execute_with_sudo(&format!("systemctl enable {}", service))
                    .await?
            }
        };

        if next_result.exit_status == 0 {
            return Ok(next_result);
        }
    }

    Ok(result)
}

pub async fn disable_service(session: &Session, service: &str) -> Result<CommandResult> {
    let result = session
        .execute_with_sudo(&format!("systemctl disable {}", service))
        .await?;

    if result.exit_status != 0 {
        let next_result = match session.os_type() {
            OsType::Debian => {
                session
                    .execute_with_sudo(&format!("update-rc.d -f {} remove", service))
                    .await?
            }
            OsType::RedHat => {
                session
                    .execute_with_sudo(&format!("chkconfig {} off", service))
                    .await?
            }
            OsType::Arch => {
                session
                    .execute_with_sudo(&format!("systemctl disable {}", service))
                    .await?
            }
        };

        if next_result.exit_status == 0 {
            return Ok(next_result);
        }
    }
    Ok(result)
}

pub async fn start_service(session: &Session, service: &str) -> Result<CommandResult> {
    let result = session
        .execute_with_sudo(&format!("systemctl start {}", service))
        .await?;

    if result.exit_status != 0 {
        let next_result = session
            .execute_with_sudo(&format!("service {} start", service))
            .await?;
        if next_result.exit_status == 0 {
            return Ok(next_result);
        }
    }
    Ok(result)
}

pub async fn stop_service(session: &Session, service: &str) -> Result<CommandResult> {
    let result = session
        .execute_with_sudo(&format!("systemctl stop {}", service))
        .await?;

    if result.exit_status != 0 {
        let next_result = session
            .execute_with_sudo(&format!("service {} stop", service))
            .await?;
        if next_result.exit_status == 0 {
            return Ok(next_result);
        }
    }

    Ok(result)
}

pub async fn restart_service(session: &Session, service: &str) -> Result<CommandResult> {
    let result = session
        .execute_with_sudo(&format!("systemctl restart {}", service))
        .await?;

    if result.exit_status != 0 {
        let next_result = session
            .execute_with_sudo(&format!("service {} restart", service))
            .await?;
        if next_result.exit_status == 0 {
            return Ok(next_result);
        }
    }
    Ok(result)
}

pub async fn reload_service(session: &Session, service: &str) -> Result<CommandResult> {
    let result = session
        .execute_with_sudo(&format!("systemctl reload {}", service))
        .await?;

    if result.exit_status != 0 {
        let next_result = session
            .execute_with_sudo(&format!("service {} reload", service))
            .await?;
        if next_result.exit_status == 0 {
            return Ok(next_result);
        }
    }
    Ok(result)
}

pub async fn service_status(session: &Session, service: &str) -> Result<CommandResult> {
    let result = session
        .execute_with_sudo(&format!("systemctl status {}", service))
        .await?;

    if result.exit_status != 0 {
        let next_result = session
            .execute_with_sudo(&format!("service {} status", service))
            .await?;
        if next_result.exit_status == 0 {
            return Ok(next_result);
        }
    }
    Ok(result)
}

/// Truncate error message to a reasonable number of lines for display
pub fn truncate_error_message(message: &str, max_lines: usize) -> String {
    let lines: Vec<&str> = message.lines().collect();
    if lines.len() <= max_lines {
        message.to_string()
    } else {
        let truncated_lines = &lines[..max_lines];
        format!(
            "{}\n... (truncated {} more lines)",
            truncated_lines.join("\n"),
            lines.len() - max_lines
        )
    }
}
