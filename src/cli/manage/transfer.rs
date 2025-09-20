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
    pub remote: Option<String>,
    /// Local file path (required for upload/download)
    #[arg(long)]
    pub local: Option<String>,
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
            if self.remote.is_none() {
                return Err(anyhow!("--remote is required for upload"));
            }
            if self.local.is_none() {
                return Err(anyhow!("--local is required for upload"));
            }
        } else if self.download {
            if self.remote.is_none() {
                return Err(anyhow!("--remote is required for download"));
            }
            if self.local.is_none() {
                return Err(anyhow!("--local is required for download"));
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

                // Set a nice style for the progress bar
                let style = ProgressStyle::default_bar()
                    .template("{msg} [{elapsed_precise}] {spinner:.green} [{bar:40.cyan/blue}] {percent:>3}% {bytes}/{total_bytes} ({bytes_per_sec}) ETA: {eta}")
                    .unwrap()
                    .progress_chars("#>-");
                pb.set_style(style);

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
        upload(
            pb,
            &task.srv_name,
            &task.ssh_client,
            action.local.as_ref().unwrap(),
            action.remote.as_ref().unwrap(),
            transfer_config,
        )
        .await
    } else if action.download {
        // For download, append server name to avoid file conflicts
        let local_path = if add_name {
            add_server_name(action.local.as_ref().unwrap(), &task.srv_name)
        } else {
            action.local.as_ref().unwrap().clone()
        };

        download(
            pb,
            &task.srv_name,
            &task.ssh_client,
            action.remote.as_ref().unwrap(),
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

/// Upload to server.
pub async fn upload(
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
        "Uploading '{}' to '{}' on server '{}({})'",
        local_path,
        remote_path,
        srv_name,
        ssh_client,
    );

    let bytes_transferred = if let Some(ref pb) = pb {
        transfer_session
            .upload_with_callback(local_path, remote_path, |progress| {
                progress_callback(pb.clone(), srv_name, Operation::Upload, progress)
            })
            .await?
    } else {
        transfer_session.upload(local_path, remote_path).await?
    };

    if let Some(ref pb) = pb {
        pb.finish_and_clear();
    }
    println!(
        "📤 Uploaded Success {} Bytes on server '{}({})'",
        bytes_transferred, srv_name, ssh_client
    );

    Ok(())
}

/// Download file from server.
pub async fn download(
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
        "Downloading '{}' from '{}' on server '{}({})'",
        local_path,
        remote_path,
        srv_name,
        ssh_client
    );

    let bytes_transferred = if let Some(ref pb) = pb {
        transfer_session
            .download_with_callback(remote_path, local_path, |progress| {
                progress_callback(pb.clone(), srv_name, Operation::Download, progress)
            })
            .await?
    } else {
        transfer_session.download(remote_path, local_path).await?
    };

    if let Some(ref pb) = pb {
        pb.finish_and_clear();
    }
    println!(
        "📥 Downloaded {} Bytes on server '{}({})'",
        bytes_transferred, srv_name, ssh_client
    );

    Ok(())
}

#[derive(Debug, Clone, Copy)]
pub enum Operation {
    Upload,
    Download,
}

/// Progress callback for transfer operations with server name
fn progress_callback(
    pb: Arc<ProgressBar>,
    srv_name: &str,
    operation: Operation,
    transfer_progress: &TransferProgress,
) {
    // always set total bytes
    pb.set_length(transfer_progress.total_bytes);

    // Update the current position
    pb.set_position(transfer_progress.done_bytes);

    // Choose filename to display based on operation type
    let display_name = match operation {
        Operation::Upload => get_display_filename(&transfer_progress.local_path),
        Operation::Download => get_display_filename(&transfer_progress.remote_path),
    };

    // Set the message with server name, operation and filename
    pb.set_message(format!("📥 [{}] {}", srv_name, display_name));
}

/// Get display filename from path, truncating if too long
fn get_display_filename(path: &str) -> String {
    use std::path::Path;

    let path = Path::new(&path);
    let filename = path.file_name().unwrap_or_default().to_string_lossy();

    // Truncate filename if too long
    if filename.len() > 20 {
        format!("{}...", &filename[..17])
    } else {
        filename.to_string()
    }
}
