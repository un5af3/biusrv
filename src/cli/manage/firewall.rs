use std::sync::Arc;

use anyhow::Result;
use clap::Args;

use crate::{
    cli::executor::{self, Task},
    firewall,
    ssh::Client,
};

#[derive(Args, Clone, Debug)]
pub struct FirewallAction {
    /// Show firewall status and port information
    #[arg(long)]
    pub status: bool,
    /// Allow ports
    #[arg(long, value_delimiter = ',')]
    pub allow_port: Vec<String>,
    /// Deny ports
    #[arg(long, value_delimiter = ',')]
    pub deny_port: Vec<String>,
    /// Delete allowed ports
    #[arg(long, value_delimiter = ',')]
    pub delete_allow_port: Vec<String>,
    /// Delete denied ports
    #[arg(long, value_delimiter = ',')]
    pub delete_deny_port: Vec<String>,
    /// Save firewall rules permanently
    #[arg(long)]
    pub save: bool,
}

impl FirewallAction {
    pub fn local_execute(&self) -> Result<bool> {
        if !self.status
            && self.allow_port.is_empty()
            && self.deny_port.is_empty()
            && self.delete_allow_port.is_empty()
            && self.delete_deny_port.is_empty()
        {
            return Err(anyhow::anyhow!("No firewall action specified. Use --status, --allow-port, --deny-port, --delete-allow-port, or --delete-deny-port"));
        }

        Ok(false)
    }

    pub async fn remote_execute(
        &self,
        thread_num: usize,
        max_retry: u32,
        tasks: Vec<Task>,
    ) -> Result<()> {
        let action = Arc::new(self.clone());
        executor::execute_tasks(thread_num, max_retry, tasks, move |_, task| {
            let action = Arc::clone(&action);
            handle_firewall_execute(action, task)
        })
        .await
    }
}

pub async fn handle_firewall_execute(action: Arc<FirewallAction>, task: Arc<Task>) -> Result<()> {
    let result = if action.status {
        show_status(&task.srv_name, &task.ssh_client).await
    } else if !action.allow_port.is_empty() {
        allow_ports(
            &task.srv_name,
            &task.ssh_client,
            &action.allow_port,
            action.save,
        )
        .await
    } else if !action.deny_port.is_empty() {
        deny_ports(
            &task.srv_name,
            &task.ssh_client,
            &action.deny_port,
            action.save,
        )
        .await
    } else if !action.delete_allow_port.is_empty() {
        delete_allow_ports(
            &task.srv_name,
            &task.ssh_client,
            &action.delete_allow_port,
            action.save,
        )
        .await
    } else if !action.delete_deny_port.is_empty() {
        delete_deny_ports(
            &task.srv_name,
            &task.ssh_client,
            &action.delete_deny_port,
            action.save,
        )
        .await
    } else {
        unreachable!()
    };

    if let Err(e) = result {
        println!("❌ {} ({}) - Failed: {}", task.srv_name, task.ssh_client, e);
        return Err(e);
    }

    println!("✅ {} ({}) - Success", task.srv_name, task.ssh_client);
    Ok(())
}

/// Allow ports on a server.
pub async fn allow_ports<S: AsRef<str> + std::fmt::Debug>(
    srv_name: &str,
    ssh_client: &Client,
    ports: &[S],
    save: bool,
) -> Result<()> {
    let session = match ssh_client.connect().await {
        Ok(session) => session,
        Err(e) => {
            log::error!("Failed to connect to {}({})", srv_name, ssh_client);
            return Err(e);
        }
    };

    log::info!("Allowing ports {:?} on server '{}'", ports, srv_name);
    firewall::allow_ports(&session, ports).await?;

    if save {
        log::info!("Saving firewall rules permanently on server '{}'", srv_name);
        firewall::save_rules(&session).await?;
    }

    Ok(())
}

/// Deny ports on a server.
pub async fn deny_ports<S: AsRef<str> + std::fmt::Debug>(
    srv_name: &str,
    ssh_client: &Client,
    ports: &[S],
    save: bool,
) -> Result<()> {
    let session = match ssh_client.connect().await {
        Ok(session) => session,
        Err(e) => {
            log::error!("Failed to connect to {}({})", srv_name, ssh_client);
            return Err(e);
        }
    };

    log::info!("Denying ports {:?} on server '{}'", ports, srv_name);
    firewall::deny_ports(&session, ports).await?;

    if save {
        log::info!("Saving firewall rules permanently on server '{}'", srv_name);
        firewall::save_rules(&session).await?;
    }

    Ok(())
}

/// Show firewall status for a server.
pub async fn show_status(srv_name: &str, ssh_client: &Client) -> Result<()> {
    let session = match ssh_client.connect().await {
        Ok(session) => session,
        Err(e) => {
            log::error!("Failed to connect to {}({})", srv_name, ssh_client);
            return Err(e);
        }
    };

    let status = firewall::status(&session).await?;
    log::info!(
        "Checking firewall status for server '{} ({})'",
        srv_name,
        ssh_client
    );
    println!("{}", status);

    Ok(())
}

/// Delete allowed ports on a server.
pub async fn delete_allow_ports<S: AsRef<str> + std::fmt::Debug>(
    srv_name: &str,
    ssh_client: &Client,
    ports: &[S],
    save: bool,
) -> Result<()> {
    let session = match ssh_client.connect().await {
        Ok(session) => session,
        Err(e) => {
            log::error!("Failed to connect to {}({})", srv_name, ssh_client);
            return Err(e);
        }
    };

    log::info!(
        "Deleting allowed ports {:?} on server '{}'",
        ports,
        srv_name
    );
    firewall::delete_ports(&session, true, ports).await?;

    if save {
        log::info!("Saving firewall rules permanently on server '{}'", srv_name);
        firewall::save_rules(&session).await?;
    }

    Ok(())
}

/// Delete denied ports on a server.
pub async fn delete_deny_ports<S: AsRef<str> + std::fmt::Debug>(
    srv_name: &str,
    ssh_client: &Client,
    ports: &[S],
    save: bool,
) -> Result<()> {
    let session = match ssh_client.connect().await {
        Ok(session) => session,
        Err(e) => {
            log::error!("Failed to connect to {}({})", srv_name, ssh_client);
            return Err(e);
        }
    };

    log::info!("Deleting denied ports {:?} on server '{}'", ports, srv_name);
    firewall::delete_ports(&session, false, ports).await?;

    if save {
        log::info!("Saving firewall rules permanently on server '{}'", srv_name);
        firewall::save_rules(&session).await?;
    }

    Ok(())
}
