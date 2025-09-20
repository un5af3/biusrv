use std::sync::Arc;

use anyhow::{anyhow, Result};
use clap::Args;

use crate::{
    cli::{
        common,
        executor::{self, Task},
    },
    config::InitConfig,
    init::InitServer,
};

#[derive(Args)]
pub struct InitCommand {
    /// List all servers
    #[arg(long)]
    pub list_servers: bool,
    /// Initialize all servers
    #[arg(long)]
    pub all_servers: bool,
    /// Specify server names to initialize
    #[arg(short, long, value_delimiter = ',')]
    pub server: Vec<String>,
    /// Threads to use for initialization, default is cpu cores
    #[arg(short, long)]
    pub threads: Option<usize>,
    /// Maximum retry attempts for failed operations
    #[arg(long, default_value = "0")]
    pub max_retry: u32,
}

impl InitCommand {
    pub async fn execute(&self, config: &InitConfig) -> Result<()> {
        let srv_config = config
            .server
            .as_ref()
            .ok_or_else(|| anyhow!("No servers configured"))?;

        if srv_config.is_empty() {
            return Err(anyhow!("No servers configured"));
        }

        if self.list_servers {
            println!("Available servers for initialization:");
            common::list_servers(srv_config);
            return Ok(());
        }

        // Handle all servers case
        let tasks = if self.all_servers {
            executor::build_tasks(srv_config)?
        } else if !self.server.is_empty() {
            let mut tasks = vec![];
            for server_name in self.server.iter() {
                let cfg = srv_config
                    .get(server_name)
                    .ok_or_else(|| anyhow!("Server '{}' not found in init config", server_name))?;
                tasks.push(Task {
                    srv_name: server_name.clone(),
                    ssh_client: cfg.build_client()?,
                });
            }
            tasks
        } else {
            return Err(anyhow!("No servers specified. Use --server to specify servers or --all-servers to initialize all servers."));
        };

        let init_server = Arc::new(InitServer::new(config));

        // Handle multiple servers or all servers
        let thread_num = self.threads.unwrap_or(
            std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4),
        );

        println!("\n🚀 Server Initialization");
        println!("{}", "═".repeat(50));
        executor::list_tasks(&tasks);

        executor::execute_tasks(thread_num, self.max_retry, tasks, move |_, task| {
            let init_server = Arc::clone(&init_server);
            handle_server(init_server, task)
        })
        .await
    }
}

// Handle single server initialization
async fn handle_server(init_server: Arc<InitServer>, task: Arc<Task>) -> Result<()> {
    println!("🔧 Initializing: {}", task.srv_name);

    if let Err(e) = run_init(&init_server, &task).await {
        println!("❌ {} ({}) - Failed: {}", task.srv_name, task.ssh_client, e);
    } else {
        println!("✅ {} ({}) - Success", task.srv_name, task.ssh_client);
    }

    Ok(())
}

pub async fn run_init(init_server: &InitServer, task: &Task) -> Result<()> {
    let session = task.ssh_client.connect().await?;

    println!(
        "  📦 {} ({}) → Updating system packages",
        task.srv_name, task.ssh_client
    );
    init_server.update_system(&session).await?;

    println!(
        "  📥 {} ({}) → Installing required packages",
        task.srv_name, task.ssh_client
    );
    init_server.install_required(&session).await?;

    println!(
        "  👤 {} ({}) → Creating user account",
        task.srv_name, task.ssh_client
    );
    init_server.create_user(&session).await?;

    println!(
        "  🔐 {} ({}) → Setting up sudo permissions",
        task.srv_name, task.ssh_client
    );
    init_server.setup_sudo(&session).await?;

    let mut ssh_port = 22;
    if let Some(ref sshd_config) = init_server.sshd_config {
        println!(
            "  🔑 {} ({}) → Configuring SSH daemon",
            task.srv_name, task.ssh_client
        );
        init_server.configure_sshd(&session, sshd_config).await?;
        if let Some(port) = sshd_config.new_port {
            ssh_port = port;
        }
    }

    if let Some(ref fail2ban_config) = init_server.fail2ban_config {
        println!(
            "  🛡️ {} ({}) → Setting up Fail2ban protection",
            task.srv_name, task.ssh_client
        );
        init_server
            .setup_fail2ban(&session, fail2ban_config)
            .await?;
    }

    if let Some(ref commands) = init_server.commands {
        println!(
            "  ⚡ {} ({}) → Executing custom commands",
            task.srv_name, task.ssh_client
        );
        init_server
            .execute_custom_commands(&session, commands)
            .await?;
    }

    if let Some(ref firewall_config) = init_server.firewall_config {
        println!(
            "  🔥 {} ({}) → Configuring firewall",
            task.srv_name, task.ssh_client
        );
        init_server
            .setup_firewall(&session, ssh_port, firewall_config)
            .await?;
    }

    println!(
        "  🔄 {} ({}) → Reloading SSH daemon",
        task.srv_name, task.ssh_client
    );
    init_server.reload_sshd(&session).await?;

    Ok(())
}
