/// SFTP related functionality.
use std::{collections::VecDeque, io::SeekFrom, time::Instant};

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
            max_retry: 0,
            chunk_size: 64 * 1024,
            progress_interval: 1.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TransferProgress {
    // local path
    pub local_path: String,
    // remote path
    pub remote_path: String,
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
    pub fn new(total_bytes: u64, done_bytes: u64, local_path: String, remote_path: String) -> Self {
        Self {
            local_path,
            remote_path,
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

    pub fn inner_session(&self) -> &SftpSession {
        &self.session
    }

    pub async fn upload(&self, local_path: &str, remote_path: &str) -> Result<u64> {
        self.upload_with_callback(local_path, remote_path, no_callback)
            .await
    }

    pub async fn download(&self, remote_path: &str, local_path: &str) -> Result<u64> {
        self.download_with_callback(remote_path, local_path, no_callback)
            .await
    }

    pub async fn upload_file(&self, local_path: &str, remote_path: &str) -> Result<u64> {
        self.upload_file_with_callback(local_path, remote_path, no_callback)
            .await
    }

    pub async fn download_file(&self, remote_path: &str, local_path: &str) -> Result<u64> {
        self.download_file_with_callback(remote_path, local_path, no_callback)
            .await
    }

    pub async fn upload_dir(&self, local_dir: &str, remote_dir: &str) -> Result<u64> {
        self.upload_dir_with_callback(local_dir, remote_dir, no_callback)
            .await
    }

    pub async fn download_dir(&self, remote_dir: &str, local_dir: &str) -> Result<u64> {
        self.download_dir_with_callback(remote_dir, local_dir, no_callback)
            .await
    }

    pub async fn upload_with_callback<C>(
        &self,
        local_path: &str,
        remote_path: &str,
        callback: C,
    ) -> Result<u64>
    where
        C: Fn(&TransferProgress),
    {
        let metadata = tokio::fs::metadata(local_path).await?;
        if metadata.is_dir() {
            self.upload_dir_with_callback(local_path, remote_path, callback)
                .await
        } else if metadata.is_file() {
            self.upload_file_with_callback(local_path, remote_path, callback)
                .await
        } else {
            return Err(anyhow!("Invalid local path: {}", local_path));
        }
    }

    pub async fn download_with_callback<C>(
        &self,
        remote_path: &str,
        local_path: &str,
        callback: C,
    ) -> Result<u64>
    where
        C: Fn(&TransferProgress),
    {
        let metadata = self.session.metadata(remote_path).await?;
        if metadata.is_dir() {
            self.download_dir_with_callback(remote_path, local_path, callback)
                .await
        } else if metadata.is_regular() {
            self.download_file_with_callback(remote_path, local_path, callback)
                .await
        } else {
            return Err(anyhow!("Invalid remote path: {}", remote_path));
        }
    }

    pub async fn upload_file_with_callback<C>(
        &self,
        local_path: &str,
        remote_path: &str,
        callback: C,
    ) -> Result<u64>
    where
        C: Fn(&TransferProgress),
    {
        let mut local_file = tokio::fs::File::open(local_path).await?;
        let local_size = local_file.metadata().await?.len();

        let metadata = if let Ok(meta) = self.session.metadata(remote_path).await {
            if !meta.is_regular() {
                return Err(anyhow!("Remote path '{remote_path}' exists but not file"));
            }
            Some(meta)
        } else {
            None
        };

        let (mut remote_file, remote_size) = match (self.config.force, self.config.resume) {
            (true, _) => (self.session.create(remote_path).await?, 0),
            (false, false) => {
                if metadata.is_some() {
                    return Err(anyhow!("Remote file already exists"));
                }
                let remote_file = self.session.create(remote_path).await?;
                (remote_file, 0)
            }
            (false, true) => {
                let mut remote_file = self
                    .session
                    .open_with_flags(remote_path, OpenFlags::WRITE | OpenFlags::CREATE)
                    .await?;
                let remote_size = if let Some(meta) = metadata {
                    meta.len()
                } else {
                    0
                };

                if remote_size == local_size {
                    return Ok(0);
                } else if remote_size > local_size {
                    return Err(anyhow!("Remote file is larger than local file"));
                } else {
                    local_file.seek(SeekFrom::Start(remote_size)).await?;
                    remote_file.seek(SeekFrom::Start(remote_size)).await?;
                    (remote_file, remote_size)
                }
            }
        };

        let progress = TransferProgress::new(
            local_size,
            remote_size,
            local_path.to_string(),
            remote_path.to_string(),
        );

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
        C: Fn(&TransferProgress),
    {
        let mut remote_file = self.session.open(remote_path).await?;
        let metadata = remote_file.metadata().await?;
        if !metadata.is_regular() {
            return Err(anyhow!("Remote path '{remote_path}' exists but not file"));
        }
        let remote_size = metadata.len();

        let (mut local_file, local_size) = match (self.config.force, self.config.resume) {
            (true, _) => {
                let local_file = tokio::fs::File::create(local_path).await?;
                (local_file, 0)
            }
            (false, false) => {
                if tokio::fs::metadata(local_path).await.is_ok() {
                    return Err(anyhow!("Local file already exists"));
                }

                let local_file = tokio::fs::File::create(local_path).await?;
                (local_file, 0)
            }
            (false, true) => {
                let mut local_file = tokio::fs::OpenOptions::new()
                    .create(true)
                    .write(true)
                    .open(local_path)
                    .await?;

                let local_size = local_file.metadata().await?.len();

                if local_size == remote_size {
                    return Ok(0);
                } else if local_size > remote_size {
                    return Err(anyhow!("Local file is larger than remote file"));
                } else {
                    remote_file.seek(SeekFrom::Start(local_size)).await?;
                    local_file.seek(SeekFrom::Start(local_size)).await?;
                    (local_file, local_size)
                }
            }
        };

        let progress = TransferProgress::new(
            remote_size,
            local_size,
            local_path.to_string(),
            remote_path.to_string(),
        );

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
        C: Fn(&TransferProgress),
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
                callback(&progress);
                last_time = now;
            }
        }

        progress.update(done_bytes, Instant::now());
        callback(&progress);

        write_file.flush().await?;

        Ok(done_bytes)
    }

    pub async fn upload_dir_with_callback<C>(
        &self,
        local_dir: &str,
        remote_dir: &str,
        callback: C,
    ) -> Result<u64>
    where
        C: Fn(&TransferProgress),
    {
        let local_dir = tokio::fs::canonicalize(local_dir).await?;
        let local_dir = local_dir
            .into_os_string()
            .into_string()
            .map_err(|e| anyhow!("Failed to convert path to string: {}", e.display()))?;
        let local_dir = if cfg!(target_os = "windows") && local_dir.starts_with(r"\\?\") {
            &local_dir[4..]
        } else {
            &local_dir
        };

        let remote_dir = if remote_dir.ends_with("/") {
            &remote_dir[..remote_dir.len() - 1]
        } else {
            remote_dir
        };

        let dir_files = read_local_dir(local_dir).await?;

        // create remote dir first
        for dir_file in dir_files.iter() {
            let remote_path = replace_to_remote_path(&dir_file.path, local_dir, remote_dir);

            if self.session.create_dir(&remote_path).await.is_err() {
                // check if remote path exists
                let metadata = self.session.metadata(&remote_path).await?;
                if !(self.config.force || self.config.resume) {
                    return Err(anyhow!("Remote path already exists: {}", remote_path));
                }

                if metadata.is_dir() {
                    continue;
                }
                return Err(anyhow!("Failed to create remote directory"));
            }
        }

        let mut bytes_transfered = 0;

        // handle upload file logic
        for dir_file in dir_files.iter() {
            for local_file in dir_file.files.iter() {
                let remote_file = replace_to_remote_path(local_file, local_dir, remote_dir);
                let bytes = self
                    .upload_file_with_callback(local_file, &remote_file, |progress| {
                        callback(progress);
                    })
                    .await?;
                bytes_transfered += bytes;
            }

            for local_file in dir_file.symlinks.iter() {
                let remote_file = replace_to_remote_path(local_file, local_dir, remote_dir);
                let link = tokio::fs::read_link(local_file).await?;
                let link = link
                    .into_os_string()
                    .into_string()
                    .map_err(|e| anyhow!("Failed to convert path to string: {}", e.display()))?;
                let link_to = replace_to_remote_path(&link, local_dir, remote_dir);
                self.session.symlink(link_to, remote_file).await?;
            }
        }

        Ok(bytes_transfered)
    }

    pub async fn download_dir_with_callback<C>(
        &self,
        remote_dir: &str,
        local_dir: &str,
        callback: C,
    ) -> Result<u64>
    where
        C: Fn(&TransferProgress),
    {
        let remote_dir = &self.session.canonicalize(remote_dir).await?;

        let local_dir = if local_dir.ends_with("/") {
            &local_dir[..local_dir.len() - 1]
        } else {
            local_dir
        };

        let dir_files = read_remote_dir(&self.session, remote_dir).await?;

        // create local dir first
        for dir_file in dir_files.iter() {
            let local_path = replace_to_local_path(&dir_file.path, local_dir, remote_dir);

            if let Err(e) = tokio::fs::create_dir(&local_path).await {
                if e.kind() == std::io::ErrorKind::AlreadyExists {
                    let metadata = tokio::fs::metadata(&local_path).await?;
                    if !(self.config.force || self.config.resume) {
                        return Err(anyhow!("Local path already exists: {}", local_path));
                    }

                    if metadata.is_dir() {
                        continue;
                    }
                    return Err(anyhow!("Local path already exists but not directory"));
                }
                return Err(anyhow!("Failed to create local directory: {}", e));
            }
        }

        let mut bytes_transfered = 0;

        // handle download file logic
        for dir_file in dir_files.iter() {
            for remote_file in dir_file.files.iter() {
                let local_file = replace_to_local_path(remote_file, local_dir, remote_dir);
                let bytes = self
                    .download_file_with_callback(remote_file, &local_file, |progress| {
                        callback(progress);
                    })
                    .await?;
                bytes_transfered += bytes;
            }

            for remote_file in dir_file.symlinks.iter() {
                let local_file = replace_to_local_path(remote_file, local_dir, remote_dir);
                let link_to = self.session.read_link(remote_file).await?;
                let link_to = replace_to_local_path(&link_to, local_dir, remote_dir);
                tokio::fs::symlink_file(link_to, local_file).await?;
            }
        }

        Ok(bytes_transfered)
    }
}

/// No callback function.
pub fn no_callback(_: &TransferProgress) {}

// Read local directory
pub async fn read_local_dir(path: &str) -> Result<Vec<DirFile>> {
    let mut dir_files = vec![];
    let mut queue = VecDeque::new();

    queue.push_back(DirFile::new(path.to_string()));
    while let Some(mut cur_dir_file) = queue.pop_front() {
        let mut read_dir = tokio::fs::read_dir(&cur_dir_file.path).await?;

        while let Some(entry) = read_dir.next_entry().await? {
            let entry_path = entry
                .path()
                .to_str()
                .ok_or_else(|| anyhow!("Invalid UTF-8 path: {:?}", &entry.path()))?
                .to_string();

            let file_type = entry.file_type().await?;

            if file_type.is_dir() {
                queue.push_back(DirFile::new(entry_path));
            } else if file_type.is_file() {
                cur_dir_file.add_file(entry_path);
            } else if file_type.is_symlink() {
                cur_dir_file.add_symlink(entry_path);
            }
        }

        dir_files.push(cur_dir_file);
    }

    Ok(dir_files)
}

/// Read remote directory.
pub async fn read_remote_dir(session: &SftpSession, path: &str) -> Result<Vec<DirFile>> {
    let mut dir_files = vec![];
    let mut queue = VecDeque::new();

    queue.push_back(DirFile::new(path.to_string()));
    while let Some(mut cur_dir_file) = queue.pop_front() {
        let read_dir = session.read_dir(&cur_dir_file.path).await?;

        for entry in read_dir {
            let entry_path = format!("{}/{}", cur_dir_file.path, entry.file_name());

            let file_type = entry.file_type();
            if file_type.is_dir() {
                queue.push_back(DirFile::new(entry_path));
            } else if file_type.is_file() {
                cur_dir_file.add_file(entry_path);
            } else if file_type.is_symlink() {
                cur_dir_file.add_symlink(entry_path);
            }
        }

        dir_files.push(cur_dir_file);
    }

    Ok(dir_files)
}

fn to_remote_path(path: String) -> String {
    if cfg!(target_os = "windows") {
        path.replace("\\", "/")
    } else {
        path
    }
}

fn replace_to_remote_path(path: &str, local_dir: &str, remote_dir: &str) -> String {
    let path = if path.starts_with(local_dir) {
        path.replacen(local_dir, remote_dir, 1)
    } else {
        path.to_string()
    };
    to_remote_path(path)
}

fn to_local_path(path: String) -> String {
    if cfg!(target_os = "windows") {
        path.replace("/", "\\")
    } else {
        path
    }
}

fn replace_to_local_path(path: &str, local_dir: &str, remote_dir: &str) -> String {
    let path = if path.starts_with(remote_dir) {
        path.replacen(remote_dir, local_dir, 1)
    } else {
        path.to_string()
    };
    to_local_path(path)
}

#[derive(Debug, Clone)]
pub struct DirFile {
    pub path: String,
    pub files: Vec<String>,
    pub symlinks: Vec<String>,
}

impl DirFile {
    pub fn new(path: String) -> Self {
        Self {
            path,
            files: vec![],
            symlinks: vec![],
        }
    }

    pub fn add_file(&mut self, file: String) {
        self.files.push(file);
    }

    pub fn add_symlink(&mut self, symlink: String) {
        self.symlinks.push(symlink);
    }
}
