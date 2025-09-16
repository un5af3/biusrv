use anyhow::{anyhow, Result};
use clap::Args;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::sync::Arc;

use crate::{
    cli::executor::{self, Task},
    ssh::Client,
    transfer::{TransferConfig, TransferProgress},
};

#[derive(Args, Clone, Debug)]
pub struct TransferAction {
    /// Upload local file to remote server
    #[arg(long)]
    pub upload: bool,
    /// Download remote file to local
    #[arg(long)]
    pub download: bool,
    /// Remote file path (required for upload/download)
    #[arg(long)]
    pub remote_path: Option<String>,
    /// Local file path (required for upload/download)
    #[arg(long)]
    pub local_path: Option<String>,
    /// Force overwrite existing files
    #[arg(long)]
    pub force: bool,
    /// Enable resume for interrupted transfers
    #[arg(long)]
    pub resume: bool,
    /// Hide progress display
    #[arg(long)]
    pub hide_progress: bool,
}

impl TransferAction {
    pub fn local_execute(&self) -> Result<bool> {
        if self.upload {
            if self.remote_path.is_none() {
                return Err(anyhow!("--remote-path is required for upload"));
            }
            if self.local_path.is_none() {
                return Err(anyhow!("--local-path is required for upload"));
            }
        } else if self.download {
            if self.remote_path.is_none() {
                return Err(anyhow!("--remote-path is required for download"));
            }
            if self.local_path.is_none() {
                return Err(anyhow!("--local-path is required for download"));
            }
        } else {
            return Err(anyhow!(
                "No transfer action specified. Use --upload or --download"
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
        let add_name = tasks.len() > 1;
        let progress = Arc::new(MultiProgress::new());
        executor::execute_tasks(thread_num, max_retry, tasks, move |_, task| {
            let action = Arc::clone(&action);
            let pb = if action.hide_progress {
                None
            } else {
                let pb = Arc::new(progress.add(ProgressBar::new_spinner()));
                Some(pb)
            };
            handle_transfer_execute(pb, action, task, add_name, max_retry)
        })
        .await
    }
}

pub async fn handle_transfer_execute(
    pb: Option<Arc<ProgressBar>>,
    action: Arc<TransferAction>,
    task: Arc<Task>,
    add_name: bool,
    max_retry: u32,
) -> Result<()> {
    let transfer_config = TransferConfig {
        max_retry,
        force: action.force,
        resume: action.resume,
        ..Default::default()
    };

    let result = if action.upload {
        upload_file(
            pb,
            &task.srv_name,
            &task.ssh_client,
            action.local_path.as_ref().unwrap(),
            action.remote_path.as_ref().unwrap(),
            transfer_config,
        )
        .await
    } else if action.download {
        // For download, append server name to avoid file conflicts
        let local_path = if add_name {
            add_server_name(action.local_path.as_ref().unwrap(), &task.srv_name)
        } else {
            action.local_path.as_ref().unwrap().clone()
        };

        download_file(
            pb,
            &task.srv_name,
            &task.ssh_client,
            action.remote_path.as_ref().unwrap(),
            &local_path,
            transfer_config,
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

/// Add server name to file path to avoid conflicts when downloading from multiple servers
fn add_server_name(local_path: &str, server_name: &str) -> String {
    if let Some((name, ext)) = local_path.rsplit_once('.') {
        format!("{}_{}.{}", name, server_name, ext)
    } else {
        format!("{}_{}", local_path, server_name)
    }
}

/// Upload file to server.
pub async fn upload_file(
    pb: Option<Arc<ProgressBar>>,
    srv_name: &str,
    ssh_client: &Client,
    local_path: &str,
    remote_path: &str,
    config: TransferConfig,
) -> Result<()> {
    let session = match ssh_client.connect().await {
        Ok(session) => session,
        Err(e) => {
            log::error!("Failed to connect to {}({})", srv_name, ssh_client);
            return Err(e);
        }
    };

    let transfer_session = session.open_sftp_session(Some(config)).await?;

    log::info!(
        "Uploading file '{}' to '{}' on server '{}'",
        local_path,
        remote_path,
        srv_name
    );

    let bytes_transferred = if let Some(ref pb) = pb {
        transfer_session
            .upload_file_with_callback(local_path, remote_path, |progress| {
                progress_callback(pb.clone(), srv_name, "📤", progress)
            })
            .await?
    } else {
        transfer_session
            .upload_file(local_path, remote_path)
            .await?
    };

    if let Some(ref pb) = pb {
        pb.finish_and_clear();
    }
    println!("📤 Uploaded {} bytes", bytes_transferred);

    Ok(())
}

/// Download file from server.
pub async fn download_file(
    pb: Option<Arc<ProgressBar>>,
    srv_name: &str,
    ssh_client: &Client,
    remote_path: &str,
    local_path: &str,
    config: TransferConfig,
) -> Result<()> {
    let session = match ssh_client.connect().await {
        Ok(session) => session,
        Err(e) => {
            log::error!("Failed to connect to {}({})", srv_name, ssh_client);
            return Err(e);
        }
    };

    let transfer_session = session.open_sftp_session(Some(config)).await?;

    log::info!(
        "Downloading file '{}' from '{}' on server '{}'",
        local_path,
        remote_path,
        srv_name
    );

    let bytes_transferred = if let Some(ref pb) = pb {
        transfer_session
            .download_file_with_callback(remote_path, local_path, |progress| {
                progress_callback(pb.clone(), srv_name, "📥", progress)
            })
            .await?
    } else {
        transfer_session
            .download_file(remote_path, local_path)
            .await?
    };

    if let Some(ref pb) = pb {
        pb.finish_and_clear();
    }
    println!("📥 Downloaded {} bytes", bytes_transferred);

    Ok(())
}

/// Progress callback for transfer operations with server name
fn progress_callback(
    pb: Arc<ProgressBar>,
    srv_name: &str,
    operation: &str,
    transfer_progress: TransferProgress,
) {
    // Set the total length if not already set
    if pb.length().is_none() {
        pb.set_length(transfer_progress.total_bytes);

        // Set a nice style for the progress bar
        let style = ProgressStyle::default_bar()
            .template("{msg} [{elapsed_precise}] {spinner:.green} [{bar:40.cyan/blue}] {percent:>3}% {bytes}/{total_bytes} ({bytes_per_sec}) ETA: {eta}")
            .unwrap()
            .progress_chars("#>-");
        pb.set_style(style);
    }

    // Update the current position
    pb.set_position(transfer_progress.done_bytes);

    // Set the message with server name, operation and speed
    pb.set_message(format!("{} [{}]", operation, srv_name));
}
