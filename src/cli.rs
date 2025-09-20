/// CLI interface and commands.

/// Common functions for CLI.
pub mod common;

/// Executor for parallel tasks.
pub mod executor;

/// Initialize server.
pub mod init;

/// Manage server.
pub mod manage;

/// Handle multiple shell sessions.
pub mod multishell;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "biusrv")]
#[command(
    about = "🚀 SSH Server Management Tool - Initialize, manage, and control multiple servers"
)]
pub struct Cli {
    /// Config file
    #[arg(short, long, default_value = "config.yaml")]
    pub config: String,

    /// Log level (trace, debug, info, warn, error)
    #[arg(long, default_value = "warn")]
    pub log_level: String,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// 🚀 Initialize server (users, SSH, firewall, fail2ban)
    Init(init::InitCommand),
    /// ⚙️  Manage server (components, ports, services)
    Manage(manage::ManageCommand),
}
