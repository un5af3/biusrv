use std::sync::{Arc, OnceLock};

use anyhow::{anyhow, Result};
use clap::{Args, Subcommand};

use crate::{
    cli::executor::{self, Task},
    script::ScriptConfig,
};

static SCRIPT_CONFIG: OnceLock<ScriptConfig> = OnceLock::new();

/// Script action for script execution
#[derive(Args, Clone, Debug)]
pub struct ScriptAction {
    /// Script action to perform
    #[command(subcommand)]
    pub action: ScriptSubAction,
}

#[derive(Subcommand, Clone, Debug)]
pub enum ScriptSubAction {
    /// Execute a script file or directory
    Run(RunAction),
    /// List available actions in a script
    List(ListAction),
}

#[derive(Args, Clone, Debug)]
pub struct RunAction {
    /// Path to script file or directory
    #[arg(required = true)]
    pub path: String,

    /// Specific actions to execute (comma-separated, if not provided, lists available actions)
    #[arg(long, value_delimiter = ',')]
    pub action: Vec<String>,
}

#[derive(Args, Clone, Debug)]
pub struct ListAction {
    /// Path to script file or directory
    #[arg(required = true)]
    pub path: String,
}

impl ScriptAction {
    /// Execute local operations (validation, listing)
    pub fn local_execute(&self) -> Result<bool> {
        match &self.action {
            ScriptSubAction::List(list_action) => {
                list_actions(&list_action.path)?;
                Ok(true)
            }
            ScriptSubAction::Run(run_action) => {
                if run_action.action.is_empty() {
                    return Err(anyhow!("No actions specified"));
                }

                let config = ScriptConfig::load(&run_action.path)?;
                for name in run_action.action.iter() {
                    if !config.script.contains_key(name) {
                        return Err(anyhow!("Action '{}' not found", name));
                    }
                }
                SCRIPT_CONFIG.set(config).unwrap();

                Ok(false)
            }
        }
    }

    /// Execute remote operations
    pub async fn remote_execute(
        &self,
        thread_num: usize,
        max_retry: u32,
        tasks: Vec<Task>,
    ) -> Result<()> {
        let config = SCRIPT_CONFIG.get().unwrap();
        let action = Arc::new(self.clone());
        // Execute tasks using the standard executor pattern
        executor::execute_tasks(thread_num, max_retry, tasks, move |_, task| {
            let action = Arc::clone(&action);
            handle_script_execute(action, task, config)
        })
        .await
    }
}

/// List actions in a script
pub fn list_actions(path: &str) -> Result<()> {
    let config = ScriptConfig::load(path)?;

    println!("📋 Script: {}", config.info.name);
    println!("📝 Description: {}", config.info.desc);
    println!("\n🎯 Available actions:");
    if config.script.is_empty() {
        println!("  • No actions found");
    } else {
        for (action_name, action) in config.script.iter() {
            let desc = action.desc.as_deref().unwrap_or("No description");
            println!("  • {} - {}", action_name, desc);
        }
    }

    Ok(())
}

pub async fn handle_script_execute(
    action: Arc<ScriptAction>,
    task: Arc<Task>,
    config: &ScriptConfig,
) -> Result<()> {
    let result = match &action.action {
        ScriptSubAction::Run(run_action) => {
            handle_run_action(&task, config, &run_action.action).await
        }
        ScriptSubAction::List(list_action) => {
            list_actions(&list_action.path)?;
            return Ok(());
        }
    };

    if let Err(e) = result {
        println!("❌ {} ({}) - Failed: {}", task.srv_name, task.ssh_client, e);
        return Err(e);
    }

    println!("✅ {} ({}) - Success", task.srv_name, task.ssh_client);
    Ok(())
}

/// Run script actions
pub async fn handle_run_action(
    task: &Task,
    config: &ScriptConfig,
    actions: &Vec<String>,
) -> Result<()> {
    let session = match task.ssh_client.connect().await {
        Ok(session) => session,
        Err(e) => {
            log::error!(
                "Failed to connect to {} - {}",
                task.srv_name,
                task.ssh_client
            );
            return Err(e);
        }
    };

    for action_name in actions.iter() {
        let action = config.script.get(action_name).unwrap();
        println!(
            "🔍 [{} - {}] Executing action: {} - {}",
            task.srv_name,
            task.ssh_client,
            action_name,
            action.desc.as_deref().unwrap_or("No description"),
        );
        for (index, step) in action.step.iter().enumerate() {
            println!(
                "🔍 [{} - {}] Executing step {} - {}",
                task.srv_name,
                task.ssh_client,
                index + 1,
                step,
            );
            if let Err(e) = step.execute(&session).await {
                return Err(anyhow!("Failed to execute step {} - {}", step, e));
            }
        }
    }

    Ok(())
}
