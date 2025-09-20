use std::{collections::HashMap, fs, path::Path};

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

use crate::{ssh::Session, transfer::TransferConfig, utils::truncate_error_message};

/// Script configuration structure
#[derive(Debug, Deserialize, Serialize)]
pub struct ScriptConfig {
    pub info: ScriptInfo,
    pub script: HashMap<String, ScriptAction>,
}

impl ScriptConfig {
    /// Load script config from a single file
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let contents = fs::read_to_string(path)?;

        if let Some(ext) = path.extension() {
            if ext == "toml" {
                let config = toml::from_str(&contents)?;
                return Ok(config);
            } else if ext == "yaml" {
                let config = serde_yaml::from_str(&contents)?;
                return Ok(config);
            }
        }

        let config = if let Ok(config) = toml::from_str(&contents) {
            config
        } else {
            serde_yaml::from_str(&contents)?
        };

        Ok(config)
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ScriptInfo {
    pub name: String,
    pub desc: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ScriptAction {
    pub desc: Option<String>,
    pub step: Vec<ScriptActionType>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ScriptActionType {
    Command(CommandAction),
    Upload(TransferAction),
    Download(TransferAction),
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CommandAction {
    pub sudo: Option<bool>,
    pub cmds: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TransferAction {
    pub local: String,
    pub remote: String,
    pub force: Option<bool>,
    pub resume: Option<bool>,
    pub max_retry: Option<u32>,
}

impl ScriptActionType {
    pub async fn execute(&self, session: &Session) -> Result<()> {
        match self {
            ScriptActionType::Command(action) => action.execute(session).await,
            ScriptActionType::Upload(action) => action.execute(session, true).await,
            ScriptActionType::Download(action) => action.execute(session, false).await,
        }
    }
}

impl std::fmt::Display for ScriptActionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScriptActionType::Command(action) => {
                write!(f, "command (sudo: {})", action.sudo.unwrap_or(false))
            }
            ScriptActionType::Upload(action) => {
                write!(f, "upload ({} -> {})", action.local, action.remote)
            }
            ScriptActionType::Download(action) => {
                write!(f, "download ({} -> {})", action.remote, action.local)
            }
        }
    }
}

impl CommandAction {
    pub async fn execute(&self, session: &Session) -> Result<()> {
        for cmd in self.cmds.iter() {
            let result = if self.sudo.unwrap_or(false) {
                session.execute_with_sudo(cmd).await?
            } else {
                session.execute_command(cmd).await?
            };

            if result.exit_status != 0 {
                return Err(anyhow!(
                    "Failed to execute command: {} (exit code: {}) - {}",
                    cmd,
                    result.exit_status,
                    truncate_error_message(&result.output.trim(), 3)
                ));
            }
        }

        Ok(())
    }
}

impl TransferAction {
    pub async fn execute(&self, session: &Session, is_upload: bool) -> Result<()> {
        let transfer_config = TransferConfig {
            force: self.force.unwrap_or(false),
            resume: self.resume.unwrap_or(false),
            max_retry: self.max_retry.unwrap_or(0),
            ..Default::default()
        };

        let transfer_session = session.open_sftp_session(Some(transfer_config)).await?;

        if is_upload {
            transfer_session.upload(&self.local, &self.remote).await?;
        } else {
            transfer_session.download(&self.remote, &self.local).await?;
        }

        Ok(())
    }
}
