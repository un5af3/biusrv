/// Manage server.
use anyhow::{anyhow, Result};
use clap::{Args, Subcommand};

pub mod exec;
pub mod firewall;
/// Manage action modules
pub mod script;
pub mod transfer;

use crate::{
    cli::{
        common,
        executor::{self, Task},
    },
    config::ManageConfig,
};

#[derive(Args)]
pub struct ManageCommand {
    /// list all servers
    #[arg(long, global = true)]
    pub list_servers: bool,
    /// Manage all servers
    #[arg(long, global = true)]
    pub all_servers: bool,
    /// Specify server names to manage
    #[arg(short, long, value_delimiter = ',', global = true)]
    pub server: Vec<String>,
    /// Threads to use for parallel operations, default is cpu cores
    #[arg(short, long, global = true)]
    pub threads: Option<usize>,
    /// Maximum retry attempts for failed operations
    #[arg(long, default_value = "0", global = true)]
    pub max_retry: u32,
    /// Manage action to perform
    #[command(subcommand)]
    pub action: Option<ManageAction>,
}

#[derive(Subcommand, Debug, Clone)]
pub enum ManageAction {
    /// Execute scripts
    Script(script::ScriptAction),
    /// Execute commands on remote servers
    Exec(exec::ExecAction),
    /// Manage firewall (allow, deny, status, delete)
    Firewall(firewall::FirewallAction),
    /// Transfer files (upload, download)
    Transfer(transfer::TransferAction),
}

impl ManageCommand {
    pub async fn execute(&self, config: &ManageConfig) -> Result<()> {
        let srv_config = config
            .server
            .as_ref()
            .ok_or_else(|| anyhow!("No servers configured"))?;

        if self.list_servers {
            println!("Listing all servers");
            common::list_servers(srv_config);
            return Ok(());
        }

        let action = self.action.as_ref().ok_or_else(|| {
            anyhow!("Please specify an action: use subcommands (script, exec, firewall, transfer)")
        })?;

        // execute actions that don't need server
        if match action {
            ManageAction::Script(action) => action.local_execute()?,
            ManageAction::Exec(action) => action.local_execute()?,
            ManageAction::Firewall(action) => action.local_execute()?,
            ManageAction::Transfer(action) => action.local_execute()?,
        } {
            return Ok(());
        }

        // build tasks
        let tasks = if self.all_servers {
            executor::build_tasks(srv_config)?
        } else if !self.server.is_empty() {
            let mut tasks = vec![];
            for srv_name in self.server.iter() {
                let cfg = srv_config
                    .get(srv_name)
                    .ok_or_else(|| anyhow!("Server '{}' not found in manage config", srv_name))?;
                tasks.push(Task {
                    srv_name: srv_name.clone(),
                    ssh_client: cfg.build_client()?,
                });
            }
            tasks
        } else {
            return Err(anyhow!("No servers specified. Use --server to specify servers or --all-servers to manage all servers."));
        };

        println!("\n⚙️  Server Management");
        println!("{}", "═".repeat(50));
        executor::list_tasks(&tasks);

        // get thread number
        let thread_num = self.threads.unwrap_or(
            std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4),
        );

        match action {
            ManageAction::Script(script_action) => {
                script_action
                    .remote_execute(thread_num, self.max_retry, tasks)
                    .await
            }
            ManageAction::Exec(exec_action) => {
                exec_action
                    .remote_execute(thread_num, self.max_retry, tasks)
                    .await
            }
            ManageAction::Firewall(firewall_action) => {
                firewall_action
                    .remote_execute(thread_num, self.max_retry, tasks)
                    .await
            }
            ManageAction::Transfer(transfer_action) => {
                transfer_action
                    .remote_execute(thread_num, self.max_retry, tasks)
                    .await
            }
        }
    }
}
