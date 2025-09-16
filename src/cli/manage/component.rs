use anyhow::{anyhow, Result};
use clap::Args;
use std::sync::{Arc, OnceLock};

use crate::{
    cli::executor::{self, Task},
    component::manager::ComponentManager,
    ssh::Client,
};

static COMPONENT_MANAGER: OnceLock<ComponentManager> = OnceLock::new();

#[derive(Args, Clone, Debug)]
pub struct ComponentAction {
    /// Component directory path
    #[arg(short = 'D', long, default_value = "components")]
    pub dir: String,
    /// List all components
    #[arg(short, long)]
    pub list: bool,
    /// Install components
    #[arg(long, value_delimiter = ',')]
    pub install: Vec<String>,
    /// Uninstall components
    #[arg(long, value_delimiter = ',')]
    pub uninstall: Vec<String>,
}

impl ComponentAction {
    pub fn local_execute(&self) -> Result<bool> {
        let component_manager = ComponentManager::build(&self.dir)?;
        COMPONENT_MANAGER.set(component_manager).unwrap();
        let manager = COMPONENT_MANAGER.get().unwrap();

        if self.list {
            list_components(manager);
            return Ok(true);
        } else if !self.install.is_empty() {
            if !check_components(&self.install, manager) {
                return Err(anyhow!(
                    "Supported component '{}' not found",
                    self.install.join(", ")
                ));
            }
        } else if !self.uninstall.is_empty() {
            if !check_components(&self.uninstall, manager) {
                return Err(anyhow!(
                    "Supported component '{}' not found",
                    self.uninstall.join(", ")
                ));
            }
        } else {
            return Err(anyhow!(
                "No component action specified. Use --list, --install, or --uninstall"
            ));
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
        let manager = COMPONENT_MANAGER.get().unwrap();

        executor::execute_tasks(thread_num, max_retry, tasks, move |_, task| {
            let action = Arc::clone(&action);
            //let manager = Arc::clone(&manager);
            handle_component_execute(action, task, manager)
        })
        .await
    }
}

pub async fn handle_component_execute(
    action: Arc<ComponentAction>,
    task: Arc<Task>,
    manager: &ComponentManager,
) -> Result<()> {
    let result = if !action.install.is_empty() {
        install_component(&task.srv_name, &task.ssh_client, &action.install, manager).await
    } else if !action.uninstall.is_empty() {
        uninstall_component(&task.srv_name, &task.ssh_client, &action.uninstall, manager).await
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

/// List all components.
pub fn list_components(component_manager: &ComponentManager) {
    let components = component_manager.list_info();

    if components.is_empty() {
        println!("📝 No components available");
        return;
    }

    println!("\n📦 Available Components ({})", components.len());
    println!("{}", "─".repeat(60));

    for component in components {
        println!("  📦 {} - {}", component.name, component.description);
    }

    println!("{}", "─".repeat(60));
}

/// Check if all components are available.
pub fn check_components<S: AsRef<str>>(comp_names: &[S], comp_manager: &ComponentManager) -> bool {
    comp_names
        .iter()
        .all(|comp_name| comp_manager.contains(comp_name.as_ref()))
}

/// Install components.
pub async fn install_component<S: AsRef<str>>(
    srv_name: &str,
    ssh_client: &Client,
    comp_names: &[S],
    comp_manager: &ComponentManager,
) -> Result<()> {
    let session = match ssh_client.connect().await {
        Ok(session) => session,
        Err(e) => {
            log::error!("Failed to connect to {}({})", srv_name, ssh_client);
            return Err(e);
        }
    };

    for comp_name in comp_names {
        let comp_name = comp_name.as_ref();
        log::info!(
            "Installing component '{}' on server '{}'",
            comp_name,
            srv_name
        );
        comp_manager.install(comp_name, &session).await?;
    }

    Ok(())
}

/// Uninstall components.
pub async fn uninstall_component<S: AsRef<str>>(
    srv_name: &str,
    ssh_client: &Client,
    comp_names: &[S],
    comp_manager: &ComponentManager,
) -> Result<()> {
    let session = match ssh_client.connect().await {
        Ok(session) => session,
        Err(e) => {
            log::error!("Failed to connect to {}({})", srv_name, ssh_client);
            return Err(e);
        }
    };

    for comp_name in comp_names {
        let comp_name = comp_name.as_ref();
        log::info!(
            "Uninstalling component '{}' from server '{}'",
            comp_name,
            srv_name
        );
        comp_manager.uninstall(comp_name, &session).await?;
    }

    Ok(())
}
