use std::collections::HashMap;
use std::fs;
use std::path::Path;

use anyhow::{anyhow, Result};

use crate::component::config::{
    ComponentConfig, ComponentInfo, InstallConfig, ServiceConfig, UninstallConfig,
};
use crate::ssh::Session;
use crate::utils;

/// Component manager for handling component
#[derive(Debug)]
pub struct ComponentManager {
    components: HashMap<String, ComponentConfig>,
}

impl ComponentManager {
    /// load components from path
    pub fn build<P: AsRef<Path>>(path: P) -> Result<Self> {
        let cfg = fs::read_dir(path.as_ref())?;
        let mut components = HashMap::new();

        for entry in cfg {
            let entry = entry?;
            if entry.path().extension().and_then(|s| s.to_str()) == Some("toml") {
                let component = ComponentConfig::load(entry.path())?;
                components.insert(component.info.name.clone(), component);
            }
        }

        Ok(Self { components })
    }

    /// List all components
    pub fn list_info(&self) -> Vec<&ComponentInfo> {
        self.components.values().map(|c| &c.info).collect()
    }

    /// List all components name
    pub fn list_names(&self) -> Vec<&String> {
        self.components.keys().collect()
    }

    /// Contains component name
    pub fn contains(&self, name: &str) -> bool {
        self.components.contains_key(name)
    }

    /// Get component by name
    pub fn get_component(&self, name: &str) -> Result<&ComponentConfig> {
        self.components
            .get(name)
            .ok_or_else(|| anyhow!("Component '{}' not found", name))
    }

    /// Check if a component is installed
    pub async fn is_installed(&self, name: &str, session: &Session) -> Result<bool> {
        let config = self.get_component(name)?;

        if let Some(check_config) = &config.check {
            let result = session.execute_with_sudo(&check_config.command).await?;
            Ok(result.exit_status == 0)
        } else {
            // If no check command is provided, assume it's not installed
            Ok(false)
        }
    }

    /// Install a component
    pub async fn install(&self, name: &str, session: &Session) -> Result<()> {
        let config = self.get_component(name)?;

        // Execute install configuration
        self.execute_install_config(session, &config.install)
            .await?;

        // Handle service management
        self.handle_service(session, name, &config.service, true)
            .await?;

        Ok(())
    }

    /// Uninstall a component
    pub async fn uninstall(&self, name: &str, session: &Session) -> Result<()> {
        let config = self.get_component(name)?;

        // Handle service management first
        self.handle_service(session, name, &config.service, false)
            .await?;

        // Execute uninstall configuration
        self.execute_uninstall_config(session, &config.uninstall)
            .await?;

        Ok(())
    }

    /// Execute install configuration
    async fn execute_install_config(
        &self,
        session: &Session,
        config: &InstallConfig,
    ) -> Result<()> {
        match config {
            InstallConfig::Package {
                packages,
                before,
                after,
            } => {
                // Execute before commands
                if let Some(before_commands) = before {
                    for cmd in before_commands {
                        if session.execute_with_sudo(cmd).await?.exit_status != 0 {
                            return Err(anyhow!("Failed to execute command: {}", cmd));
                        }
                    }
                }

                // Install packages
                let result = utils::install_packages(
                    session,
                    &packages.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
                )
                .await?;
                if result.exit_status != 0 {
                    return Err(anyhow!("Failed to install packages"));
                }

                // Execute after commands
                if let Some(after_commands) = after {
                    for cmd in after_commands {
                        if session.execute_with_sudo(cmd).await?.exit_status != 0 {
                            return Err(anyhow!("Failed to execute command: {}", cmd));
                        }
                    }
                }
            }
            InstallConfig::Command { commands } => {
                for cmd in commands {
                    if session.execute_with_sudo(cmd).await?.exit_status != 0 {
                        return Err(anyhow!("Failed to execute command: {}", cmd));
                    }
                }
            }
        }
        Ok(())
    }

    /// Execute uninstall configuration
    async fn execute_uninstall_config(
        &self,
        session: &Session,
        config: &UninstallConfig,
    ) -> Result<()> {
        match config {
            UninstallConfig::Package {
                packages,
                before,
                after,
            } => {
                // Execute before commands
                if let Some(before_commands) = before {
                    for cmd in before_commands {
                        if session.execute_with_sudo(cmd).await?.exit_status != 0 {
                            return Err(anyhow!("Failed to execute command: {}", cmd));
                        }
                    }
                }

                // Uninstall packages
                let result = utils::uninstall_packages(
                    session,
                    &packages.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
                )
                .await?;
                if result.exit_status != 0 {
                    return Err(anyhow!("Failed to uninstall packages"));
                }

                // Execute after commands
                if let Some(after_commands) = after {
                    for cmd in after_commands {
                        if session.execute_with_sudo(cmd).await?.exit_status != 0 {
                            return Err(anyhow!("Failed to execute command: {}", cmd));
                        }
                    }
                }
            }
            UninstallConfig::Command { commands } => {
                for cmd in commands {
                    if session.execute_with_sudo(cmd).await?.exit_status != 0 {
                        return Err(anyhow!("Failed to execute command: {}", cmd));
                    }
                }
            }
        }
        Ok(())
    }

    /// Handle service management
    async fn handle_service(
        &self,
        session: &Session,
        name: &str,
        config: &ServiceConfig,
        is_install: bool,
    ) -> Result<()> {
        let start = config.start.unwrap_or(false);
        let enable = config.enable.unwrap_or(false);

        if is_install {
            if enable {
                utils::enable_service(session, name).await?;
            }
            if start {
                utils::start_service(session, name).await?;
            }
        } else {
            if start {
                utils::stop_service(session, name).await?;
            }
            if enable {
                utils::disable_service(session, name).await?;
            }
        }

        Ok(())
    }
}
