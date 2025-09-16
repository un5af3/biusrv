/// Configuration serialization and deserialization.
use std::{
    collections::HashMap,
    fs::{self},
    path::Path,
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::ssh::Client;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: Option<u16>,
    pub username: String,
    pub keypath: Option<String>,
    pub password: Option<String>,
    pub use_password: Option<bool>,
}

impl ServerConfig {
    pub fn build_client(&self) -> Result<Client> {
        let mut client = Client::new(self.host.clone(), self.username.clone());

        client.with_port(self.port.unwrap_or(22));

        if let Some(ref keypath) = self.keypath {
            client.with_private_key(keypath.clone());
        } else if let Some(ref password) = self.password {
            client.with_password(password.clone());
        } else if self.use_password.unwrap_or(false) {
            let password = rpassword::read_password().context("Failed to read password")?;
            client.with_password(password);
        }

        Ok(client)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub init: Option<InitConfig>,
    pub manage: Option<ManageConfig>,
}

impl Config {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let contents = fs::read_to_string(path)?;
        let config = toml::from_str(&contents)?;

        Ok(config)
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            init: None,
            manage: None,
        }
    }
}

// config like:
// [init]
// new_username = "deploy"
// new_password = "123456"
//
// [init.server.myserver]
// host = "127.0.0.1"
// port = 22
// username = "root"
// password = "123456"
//
// [init.sshd]
// new_port = 2222
// public_key = "ssh-rsa ..."
//
// [init.sshd.option]
// PubkeyAuthentication = "yes"
// PasswordAuthentication = "no"
// PermitRootLogin = "no"
// PermitEmptyPasswords = "no"
//
// [init.firewall]
// allow_ports = ["2222/tcp", "80/tcp", "443/tcp"]
// deny_ports = ["22/tcp"]
//
// [init.fail2ban.jail.sshd]
// enabled = true
// port = "2222/tcp"
// filter = "sshd"
// maxretry = 3
// findtime = 600
// bantime = 3600
#[derive(Debug, Serialize, Deserialize)]
pub struct InitConfig {
    pub server: Option<HashMap<String, ServerConfig>>,

    // create a new user with the following username and password
    pub new_username: String,
    pub new_password: String,

    pub sshd: Option<SshdConfig>,
    pub firewall: Option<FirewallConfig>,
    pub fail2ban: Option<Fail2banConfig>,

    pub packages: Option<Vec<String>>,
    pub commands: Option<Vec<String>>,
}

// config like:
// [manage.server.myserver1]
// host = "127.0.0.1"
// port = 22
// username = "testuser"
// keypath = "~/.ssh/id_rsa"
// [manage.server.myserver2]
// host = "127.0.0.2"
// port = 2222
// username = "testuser"
// keypath = "~/.ssh/id_rsa"
#[derive(Debug, Serialize, Deserialize)]
pub struct ManageConfig {
    pub server: Option<HashMap<String, ServerConfig>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FirewallConfig {
    pub allow_ports: Vec<String>,
    pub deny_ports: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SshdConfig {
    pub new_port: Option<u16>,
    pub public_key: Option<String>,
    pub options: Option<HashMap<String, String>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Fail2banConfig {
    // if specified, ignore the jail config
    pub content: Option<String>,
    // backend, default is systemd
    pub backend: Option<String>,
    pub jail: Option<HashMap<String, Fail2banJailConfig>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Fail2banJailConfig {
    pub enabled: bool,
    pub port: String,
    pub filter: String,
    pub maxretry: u16,
    pub findtime: u16,
    pub bantime: u16,
    pub logpath: Option<String>,
    pub ignoreip: Option<Vec<String>>,
    pub options: Option<HashMap<String, String>>,
}
