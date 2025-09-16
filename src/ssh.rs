#![allow(dead_code)]
/// SSH related functionality.
use std::future::Future;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use crossterm::terminal;
use russh::{
    client::{self, Config, Handle},
    keys::{load_secret_key, ssh_key, PrivateKeyWithHashAlg},
};
use russh_sftp::client::SftpSession;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::mpsc,
};

use crate::transfer::{TransferConfig, TransferSession};

#[derive(Debug)]
pub struct Client {
    host: String,
    port: u16,
    username: String,
    password: Option<String>,
    keypath: Option<String>,
}

impl Client {
    pub fn new(host: String, username: String) -> Self {
        Self {
            host,
            port: 22,
            username,
            password: None,
            keypath: None,
        }
    }

    pub fn host(&self) -> &str {
        &self.host
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn user(&self) -> &str {
        &self.username
    }

    pub fn with_password(&mut self, password: String) {
        self.password = Some(password);
    }

    pub fn with_private_key(&mut self, keypath: String) {
        self.keypath = Some(keypath);
    }

    pub fn with_port(&mut self, port: u16) {
        self.port = port;
    }

    pub async fn connect(&self) -> Result<Session> {
        let config = Config::default();
        let config = Arc::new(config);

        let handler = Handler {};
        let mut session = client::connect(config, (&self.host[..], self.port), handler).await?;

        let auth_result = if let Some(password) = &self.password {
            session
                .authenticate_password(&self.username, password)
                .await?
        } else if let Some(ref keypath) = self.keypath {
            let key_pair = load_secret_key(keypath, None)
                .with_context(|| format!("Failed to load private key from: {}", keypath))?;
            session
                .authenticate_publickey(
                    &self.username,
                    PrivateKeyWithHashAlg::new(
                        Arc::new(key_pair),
                        session.best_supported_rsa_hash().await?.flatten(),
                    ),
                )
                .await?
        } else {
            return Err(anyhow!(
                "No authentication method provided (need password or private key)"
            ));
        };

        if !auth_result.success() {
            return Err(anyhow!(
                "SSH authentication failed for user: {}",
                self.username
            ));
        }

        Ok(Session {
            user: self.username.clone(),
            handler: session,
        })
    }
}

impl std::fmt::Display for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}@{}:{}", self.username, self.host, self.port)
    }
}

#[derive(Debug)]
pub struct CommandResult {
    pub output: String,
    pub exit_status: u32,
}

pub struct Session {
    user: String,
    handler: Handle<Handler>,
}

impl Session {
    pub fn current_user(&self) -> &str {
        &self.user
    }

    pub async fn open_sftp_session(
        &self,
        config: Option<TransferConfig>,
    ) -> Result<TransferSession> {
        let channel = self.handler.channel_open_session().await?;
        channel.request_subsystem(true, "sftp").await?;
        let session = SftpSession::new(channel.into_stream()).await?;

        Ok(TransferSession::new(session, config.unwrap_or_default()))
    }

    pub async fn execute_command_no_reply<S: AsRef<str>>(&self, command: S) -> Result<()> {
        let channel = self.handler.channel_open_session().await?;
        channel.exec(false, command.as_ref()).await?;
        Ok(())
    }

    pub async fn execute_command<S: AsRef<str>>(&self, command: S) -> Result<CommandResult> {
        let mut channel = self.handler.channel_open_session().await?;
        channel.exec(true, command.as_ref()).await?;

        let mut result = CommandResult {
            output: String::new(),
            exit_status: 0,
        };

        while let Some(data) = channel.wait().await {
            match data {
                russh::ChannelMsg::Data { data } => {
                    result.output.push_str(&String::from_utf8_lossy(&data));
                }
                russh::ChannelMsg::ExitStatus { exit_status } => {
                    result.exit_status = exit_status;
                    if exit_status != 0 {
                        return Ok(result);
                    }
                }
                russh::ChannelMsg::Close => break,
                _ => {}
            }
        }

        // Remove trailing newlines before returning
        if result.output.ends_with("\n") {
            result.output.pop();
        }
        Ok(result)
    }

    pub async fn execute_commands<S: AsRef<str>>(
        &self,
        commands: &[S],
    ) -> Result<Vec<Result<CommandResult>>> {
        let mut results = Vec::new();

        for command in commands {
            results.push(self.execute_command(command.as_ref()).await);
        }

        Ok(results)
    }

    pub async fn execute_commands_one_by_one<S: AsRef<str>>(
        &self,
        commands: &[S],
    ) -> Result<Vec<CommandResult>> {
        let mut results = Vec::new();

        for command in commands {
            let result = self.execute_command(command.as_ref()).await?;
            results.push(result);
        }

        Ok(results)
    }

    pub async fn execute_with_sudo(&self, command: &str) -> Result<CommandResult> {
        // check if current user is root
        if self.current_user() == "root" {
            self.execute_command(command).await
        } else {
            let quoted_command = shell_words::quote(command);
            let sudo_command = format!("sudo sh -c {}", quoted_command);
            self.execute_command(&sudo_command).await
        }
    }

    pub async fn interactive(&self, command: &str) -> Result<u32> {
        let mut stdin = tokio::io::stdin();
        let mut stdout = tokio::io::stdout();
        self.interactive_with_streams(command, &mut stdin, &mut stdout)
            .await
    }

    pub async fn interactive_with_streams<
        R: tokio::io::AsyncRead + Unpin,
        W: tokio::io::AsyncWrite + Unpin,
    >(
        &self,
        command: &str,
        input: &mut R,
        output: &mut W,
    ) -> Result<u32> {
        let mut channel = self.handler.channel_open_session().await?;

        let (cols, rows) = terminal::size()?;

        channel
            .request_pty(
                true,
                &std::env::var("TERM").unwrap_or("xterm".into()),
                cols as u32,
                rows as u32,
                0,
                0,
                &[],
            )
            .await?;
        channel.exec(true, command).await?;

        let code;
        let mut buf = [0u8; 1024];
        let mut stdin_closed = false;

        loop {
            tokio::select! {
                r = input.read(&mut buf), if !stdin_closed => {
                    match r {
                        Ok(0) => {
                            stdin_closed = true;
                            channel.eof().await?;
                        }
                        Ok(n) => channel.data(&buf[..n]).await?,
                        Err(e) => return Err(e.into()),
                    }
                }
                Some(msg) = channel.wait() => {
                    match msg {
                        russh::ChannelMsg::Data { data } => {
                            output.write_all(&data).await?;
                            output.flush().await?;
                        }
                        russh::ChannelMsg::ExitStatus { exit_status } => {
                            code = exit_status;
                            if !stdin_closed {
                                channel.eof().await?;
                            }
                            break;
                        }
                        _ => {}
                    }
                }
            }
        }

        Ok(code)
    }

    pub async fn interactive_with_channels(
        &self,
        command: &str,
        tx: mpsc::Sender<Vec<u8>>,
        mut rx: mpsc::Receiver<Vec<u8>>,
    ) -> Result<u32> {
        let mut channel = self.handler.channel_open_session().await?;

        let (cols, rows) = terminal::size()?;

        channel
            .request_pty(
                true,
                &std::env::var("TERM").unwrap_or("xterm".into()),
                cols as u32,
                rows as u32,
                0,
                0,
                &[],
            )
            .await?;
        channel.exec(true, command).await?;

        let code;
        let mut input_closed = false;

        loop {
            tokio::select! {
                input_data = rx.recv(), if !input_closed => {
                    match input_data {
                        Some(data) => {
                            channel.data(&data[..]).await?;
                        }
                        None => {
                            input_closed = true;
                            channel.eof().await?;
                        }
                    }
                }
                Some(msg) = channel.wait() => {
                    match msg {
                        russh::ChannelMsg::Data { data } => {
                            tx.send(data.to_vec()).await?;
                        }
                        russh::ChannelMsg::ExitStatus { exit_status } => {
                            code = exit_status;
                            if !input_closed {
                                channel.eof().await?;
                            }
                            break;
                        }
                        _ => {}
                    }
                }
            }
        }

        Ok(code)
    }
}

#[derive(Debug)]
struct Handler {}

impl client::Handler for Handler {
    type Error = russh::Error;

    fn check_server_key(
        &mut self,
        _server_public_key: &ssh_key::PublicKey,
    ) -> impl Future<Output = Result<bool, Self::Error>> + Send {
        async { Ok(true) }
    }
}
