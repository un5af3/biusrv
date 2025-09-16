/// SFTP related functionality.
use std::{io::SeekFrom, time::Instant};

use anyhow::{anyhow, Result};
use russh_sftp::{client::SftpSession, protocol::OpenFlags};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncSeekExt, AsyncWrite, AsyncWriteExt};

use crate::retry_operation;

#[derive(Debug)]
pub struct TransferConfig {
    pub force: bool,
    pub resume: bool,
    pub max_retry: u32,
    pub chunk_size: usize,
    pub progress_interval: f64,
}

impl Default for TransferConfig {
    fn default() -> Self {
        Self {
            force: false,
            resume: false,
            max_retry: 3,
            chunk_size: 64 * 1024,
            progress_interval: 1.0,
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct TransferProgress {
    // start time
    pub start_time: Instant,
    // done bytes
    pub done_bytes: u64,
    // total bytes
    pub total_bytes: u64,
    // bytes per second
    pub speed_bytes: u64,
}

impl TransferProgress {
    pub fn new(total_bytes: u64, done_bytes: u64) -> Self {
        Self {
            start_time: Instant::now(),
            done_bytes,
            total_bytes,
            speed_bytes: 0,
        }
    }

    pub fn update(&mut self, done_bytes: u64, now: Instant) {
        let elapsed_secs = now.duration_since(self.start_time).as_secs_f64();
        self.speed_bytes = if elapsed_secs > 0.0 {
            (done_bytes as f64 / elapsed_secs) as u64
        } else {
            0
        };
        self.done_bytes = done_bytes;
    }
}

pub struct TransferSession {
    session: SftpSession,
    config: TransferConfig,
}

impl TransferSession {
    pub fn new(session: SftpSession, config: TransferConfig) -> Self {
        Self { session, config }
    }

    pub async fn upload_file(&self, local_path: &str, remote_path: &str) -> Result<u64> {
        self.upload_file_with_callback(local_path, remote_path, no_callback)
            .await
    }

    pub async fn download_file(&self, remote_path: &str, local_path: &str) -> Result<u64> {
        self.download_file_with_callback(remote_path, local_path, no_callback)
            .await
    }

    pub async fn upload_file_with_callback<C>(
        &self,
        local_path: &str,
        remote_path: &str,
        callback: C,
    ) -> Result<u64>
    where
        C: Fn(TransferProgress),
    {
        let mut local_file = tokio::fs::File::open(local_path).await?;
        let local_size = local_file.metadata().await?.len();

        let (mut remote_file, remote_size) = if self.config.resume {
            let mut remote_file = self
                .session
                .open_with_flags(remote_path, OpenFlags::WRITE | OpenFlags::CREATE)
                .await?;
            let remote_size = remote_file.metadata().await?.len();
            if remote_size == local_size {
                return Ok(0);
            } else if remote_size > local_size {
                if self.config.force {
                    (self.session.create(remote_path).await?, 0)
                } else {
                    return Err(anyhow!(
                        "File already exists and is larger than the local file"
                    ));
                }
            } else {
                local_file.seek(SeekFrom::Start(remote_size)).await?;
                remote_file.seek(SeekFrom::Start(remote_size)).await?;
                (remote_file, remote_size)
            }
        } else {
            // Check if remote file exists by trying to open it
            if let Ok(_) = self.session.metadata(remote_path).await {
                if self.config.force {
                    // Force overwrite: create new file
                    (self.session.create(remote_path).await?, 0)
                } else {
                    return Err(anyhow!(
                        "Remote file already exists. Use --force to overwrite or --resume to continue"
                    ));
                }
            } else {
                // File doesn't exist, create new
                (self.session.create(remote_path).await?, 0)
            }
        };

        let progress = TransferProgress::new(local_size, remote_size);

        self.copy_file_with_callback(&mut local_file, &mut remote_file, progress, callback)
            .await
    }

    pub async fn download_file_with_callback<C>(
        &self,
        remote_path: &str,
        local_path: &str,
        callback: C,
    ) -> Result<u64>
    where
        C: Fn(TransferProgress),
    {
        let mut remote_file = self.session.open(remote_path).await?;
        let remote_size = remote_file.metadata().await?.len();

        let (mut local_file, local_size) = if self.config.resume {
            let mut local_file = tokio::fs::OpenOptions::new()
                .create(true)
                .write(true)
                .open(local_path)
                .await?;
            let local_size = local_file.metadata().await?.len();
            if local_size == remote_size {
                return Ok(0);
            } else if local_size > remote_size {
                if self.config.force {
                    (tokio::fs::File::create(local_path).await?, 0)
                } else {
                    return Err(anyhow!(
                        "Local file is larger than remote file. Use --force to overwrite"
                    ));
                }
            } else {
                remote_file.seek(SeekFrom::Start(local_size)).await?;
                local_file.seek(SeekFrom::Start(local_size)).await?;
                (local_file, local_size)
            }
        } else {
            // Check if local file exists
            if let Ok(_) = tokio::fs::metadata(local_path).await {
                if self.config.force {
                    // Force overwrite: create new file
                    (tokio::fs::File::create(local_path).await?, 0)
                } else {
                    return Err(anyhow!(
                        "Local file already exists. Use --force to overwrite or --resume to continue"
                    ));
                }
            } else {
                // File doesn't exist, create new
                (tokio::fs::File::create(local_path).await?, 0)
            }
        };

        let progress = TransferProgress::new(remote_size, local_size);

        self.copy_file_with_callback(&mut remote_file, &mut local_file, progress, callback)
            .await
    }

    async fn copy_file_with_callback<R, W, C>(
        &self,
        read_file: &mut R,
        write_file: &mut W,
        mut progress: TransferProgress,
        callback: C,
    ) -> Result<u64>
    where
        R: AsyncRead + Unpin,
        W: AsyncWrite + Unpin,
        C: Fn(TransferProgress),
    {
        progress.start_time = Instant::now();

        let mut last_time = progress.start_time;
        let mut done_bytes = progress.done_bytes;

        let mut buffer = vec![0u8; self.config.chunk_size];
        loop {
            let bytes_read =
                retry_operation!(self.config.max_retry, read_file.read(&mut buffer).await)?;
            if bytes_read == 0 {
                break;
            }

            retry_operation!(
                self.config.max_retry,
                write_file.write_all(&buffer[..bytes_read]).await
            )?;
            done_bytes += bytes_read as u64;

            // Update progress periodically (at most once per second)
            let now = Instant::now();
            if now.duration_since(last_time).as_secs_f64() >= self.config.progress_interval {
                progress.update(done_bytes, now);
                callback(progress);
                last_time = now;
            }
        }

        progress.update(done_bytes, Instant::now());
        callback(progress);

        write_file.flush().await?;

        Ok(done_bytes)
    }
}

/// No callback function.
pub fn no_callback(_: TransferProgress) {}
