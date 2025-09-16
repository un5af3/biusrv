use crate::ssh::{CommandResult, Session};
use anyhow::Result;
use base64::{engine::general_purpose, Engine as _};

pub async fn install(session: &Session, package: &str) -> Result<CommandResult> {
    let command = format!("apt install -y {}", package);
    session.execute_with_sudo(&command).await
}

pub async fn install_packages(session: &Session, packages: &[&str]) -> Result<CommandResult> {
    let command = format!("apt install -y {}", packages.join(" "));
    session.execute_with_sudo(&command).await
}

pub async fn uninstall(session: &Session, package: &str) -> Result<CommandResult> {
    let command = format!("apt remove -y {}", package);
    session.execute_with_sudo(&command).await
}

pub async fn uninstall_packages(session: &Session, packages: &[&str]) -> Result<CommandResult> {
    let command = format!("apt remove -y {}", packages.join(" "));
    session.execute_with_sudo(&command).await
}

pub async fn update_system(session: &Session) -> Result<CommandResult> {
    let command = r#"DEBIAN_FRONTEND=noninteractive apt update && apt upgrade -y -o Dpkg::Options::="--force-confdef" -o Dpkg::Options::="--force-confold""#;
    session.execute_with_sudo(command).await
}

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

pub async fn enable_service(session: &Session, service: &str) -> Result<CommandResult> {
    let mut result = session
        .execute_with_sudo(&format!("systemctl enable {}", service))
        .await?;
    if result.exit_status != 0 {
        // try update-rc.d
        result = session
            .execute_with_sudo(&format!("update-rc.d {} defaults", service))
            .await?;
    }
    Ok(result)
}

pub async fn disable_service(session: &Session, service: &str) -> Result<CommandResult> {
    let mut result = session
        .execute_with_sudo(&format!("systemctl disable {}", service))
        .await?;
    if result.exit_status != 0 {
        result = session
            .execute_with_sudo(&format!("update-rc.d -f {} remove", service))
            .await?;
    }
    Ok(result)
}

pub async fn start_service(session: &Session, service: &str) -> Result<CommandResult> {
    let mut result = session
        .execute_with_sudo(&format!("systemctl start {}", service))
        .await?;
    if result.exit_status != 0 {
        result = session
            .execute_with_sudo(&format!("service {} start", service))
            .await?;
    }
    Ok(result)
}

pub async fn stop_service(session: &Session, service: &str) -> Result<CommandResult> {
    let mut result = session
        .execute_with_sudo(&format!("systemctl stop {}", service))
        .await?;
    if result.exit_status != 0 {
        result = session
            .execute_with_sudo(&format!("service {} stop", service))
            .await?;
    }
    Ok(result)
}

pub async fn restart_service(session: &Session, service: &str) -> Result<CommandResult> {
    let mut result = session
        .execute_with_sudo(&format!("systemctl restart {}", service))
        .await?;
    if result.exit_status != 0 {
        result = session
            .execute_with_sudo(&format!("service {} restart", service))
            .await?;
    }
    Ok(result)
}

pub async fn reload_service(session: &Session, service: &str) -> Result<CommandResult> {
    let mut result = session
        .execute_with_sudo(&format!("systemctl reload {}", service))
        .await?;
    if result.exit_status != 0 {
        result = session
            .execute_with_sudo(&format!("service {} reload", service))
            .await?;
    }
    Ok(result)
}

pub async fn service_status(session: &Session, service: &str) -> Result<CommandResult> {
    let mut result = session
        .execute_with_sudo(&format!("systemctl status {}", service))
        .await?;
    if result.exit_status != 0 {
        result = session
            .execute_with_sudo(&format!("service {} status", service))
            .await?;
    }
    Ok(result)
}
