use anyhow::{anyhow, Context, Result};
use clap::Args;
use std::sync::Arc;

use crate::cli::executor::{self, Task};
use crate::cli::multishell::MultiShell;
use crate::ssh::Client;

#[derive(Args, Clone, Debug)]
pub struct ExecAction {
    /// Command to execute on remote servers
    #[arg(required = true, num_args = 1..)]
    pub command: Vec<String>,

    /// Use sudo to execute the command
    #[arg(long)]
    pub sudo: bool,

    /// Hide command output
    #[arg(long)]
    pub hide_output: bool,

    /// Start interactive shell instead of executing command
    #[arg(long)]
    pub shell: bool,
}

impl ExecAction {
    pub fn local_execute(&self) -> Result<bool> {
        if self.command.is_empty() {
            return Err(anyhow!("Command cannot be empty"));
        }
        Ok(false)
    }

    pub async fn remote_execute(
        &self,
        thread_num: usize,
        max_retry: u32,
        tasks: Vec<Task>,
    ) -> Result<()> {
        if self.shell {
            // Shell mode - start interactive shells
            let shell_cmd = self.command.join(" ");

            if tasks.len() == 1 {
                let task = tasks.first().unwrap();
                shell_session(&task.srv_name, &task.ssh_client, &shell_cmd).await
            } else {
                let mut multishell = MultiShell::new();
                multishell.start_shells(tasks, &shell_cmd).await
            }
        } else {
            // Command execution mode
            let action = Arc::new(self.clone());
            executor::execute_tasks(thread_num, max_retry, tasks, move |_, task| {
                let action = Arc::clone(&action);
                handle_exec_execute(action, task)
            })
            .await
        }
    }
}

pub async fn handle_exec_execute(action: Arc<ExecAction>, task: Arc<Task>) -> Result<()> {
    let session = match task.ssh_client.connect().await {
        Ok(session) => session,
        Err(e) => {
            log::error!(
                "Failed to connect to {}({})",
                task.srv_name,
                task.ssh_client
            );
            return Err(e);
        }
    };

    // Join command parts with spaces
    let full_command = action.command.join(" ");

    log::info!("Executing '{}' on server '{}'", full_command, task.srv_name);

    let result = if action.sudo {
        session.execute_with_sudo(&full_command).await?
    } else {
        session.execute_command(&full_command).await?
    };

    // Default to showing output unless explicitly hidden
    let show_output = !action.hide_output;

    if result.exit_status == 0 {
        println!("✅ {} ({}) - Success", task.srv_name, task.ssh_client);
        if show_output && !result.output.is_empty() {
            // Format output with server name prefix
            for line in result.output.lines() {
                if !line.trim().is_empty() {
                    println!("   {}", line);
                }
            }
        }
    } else {
        println!(
            "❌ {} ({}) - Failed (exit code: {})",
            task.srv_name, task.ssh_client, result.exit_status
        );
        if show_output && !result.output.is_empty() {
            for line in result.output.lines() {
                if !line.trim().is_empty() {
                    println!("   {}", line);
                }
            }
        }
        return Err(anyhow!("Command failed on {}", task.srv_name));
    }

    Ok(())
}

/// Start an interactive shell session on a server.
pub async fn shell_session(srv_name: &str, ssh_client: &Client, shell_cmd: &str) -> Result<()> {
    use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
    use log::info;

    info!("Connecting to server '{}' ({})", srv_name, ssh_client);

    let session = ssh_client
        .connect()
        .await
        .with_context(|| format!("Failed to connect to {}", ssh_client))?;

    info!("SSH connection successful!");
    info!("Starting interactive shell...");

    enable_raw_mode().context("Failed to enable terminal raw mode")?;

    let exit_code = session
        .interactive(shell_cmd)
        .await
        .map_err(|e| {
            let _ = disable_raw_mode();
            e
        })
        .context("Interactive shell session failed")?;

    disable_raw_mode().context("Failed to disable terminal raw mode")?;

    info!("Interactive session ended with exit code: {}", exit_code);

    Ok(())
}
