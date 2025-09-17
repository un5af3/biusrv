use std::{collections::HashMap, fs, path::Path};

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Script configuration structure
#[derive(Debug, Deserialize, Serialize)]
pub struct ScriptConfig {
    pub info: ScriptInfo,
    pub script: HashMap<String, ScriptAction>,
}

impl ScriptConfig {
    /// Load script config from a single file
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let cfg = fs::read_to_string(path)?;
        let config = toml::from_str(&cfg)?;
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
    pub sudo: bool,
    pub desc: Option<String>,
    pub commands: Vec<String>,
}
