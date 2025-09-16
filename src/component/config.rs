use std::{fs, path::Path};

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Component configuration structure
#[derive(Debug, Deserialize, Serialize)]
pub struct ComponentConfig {
    pub info: ComponentInfo,
    pub service: ServiceConfig,
    pub install: InstallConfig,
    pub uninstall: UninstallConfig,
    pub check: Option<CheckConfig>,
}

impl ComponentConfig {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let cfg = fs::read_to_string(path)?;
        let config = toml::from_str(&cfg)?;

        Ok(config)
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ComponentInfo {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ServiceConfig {
    pub start: Option<bool>,
    pub enable: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CheckConfig {
    pub command: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum InstallConfig {
    #[serde(rename = "package")]
    Package {
        packages: Vec<String>,
        before: Option<Vec<String>>,
        after: Option<Vec<String>>,
    },
    #[serde(rename = "command")]
    Command { commands: Vec<String> },
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum UninstallConfig {
    #[serde(rename = "package")]
    Package {
        packages: Vec<String>,
        before: Option<Vec<String>>,
        after: Option<Vec<String>>,
    },
    #[serde(rename = "command")]
    Command { commands: Vec<String> },
}
