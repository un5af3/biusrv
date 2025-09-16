use std::{collections::HashMap, sync::Arc};

use anyhow::Result;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    sync::{mpsc, Mutex},
};

use crate::cli::executor::Task;

#[derive(Debug)]
pub struct MultiShell {
    /// shells with input channel
    shells: HashMap<String, mpsc::Sender<Vec<u8>>>,
    /// save outputs from each shell
    outputs: Arc<Mutex<HashMap<String, Vec<String>>>>,
}

impl MultiShell {
    pub fn new() -> Self {
        Self {
            shells: HashMap::new(),
            outputs: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn start_shells(&mut self, tasks: Vec<Task>, shell_cmd: &str) -> Result<()> {
        self.distribute_tasks(tasks, shell_cmd)?;

        let stdin = tokio::io::stdin();
        let mut reader = BufReader::new(stdin);
        let mut line = String::new();

        loop {
            line.clear();
            match reader.read_line(&mut line).await {
                Ok(0) => break, // EOF
                Ok(_) => {
                    let input = line.trim();
                    if input.starts_with("/history") {
                        let srv_name = input.split_whitespace().nth(1).unwrap_or("--all");
                        self.show_outputs(srv_name).await?;
                    } else if !input.is_empty() {
                        // send command + newline
                        let command = format!("{}\n", input);
                        self.distribute_input(command.as_bytes()).await?;
                        if input == "exit" {
                            break;
                        }
                    }
                }
                Err(e) => {
                    println!("Input error: {}", e);
                    break;
                }
            }
        }

        Ok(())
    }

    pub fn distribute_tasks(&mut self, tasks: Vec<Task>, shell_cmd: &str) -> Result<()> {
        for task in tasks {
            let (input_tx, input_rx) = mpsc::channel(100);
            let (output_tx, mut output_rx) = mpsc::channel(100);

            self.shells.insert(task.srv_name.clone(), input_tx);

            let srv_name = task.srv_name.clone();
            let shell_cmd = shell_cmd.to_string();
            tokio::spawn(async move {
                let session = match task.ssh_client.connect().await {
                    Ok(session) => session,
                    Err(e) => {
                        log::error!("Failed to connect to '{}': {}", task.srv_name, e);
                        return;
                    }
                };
                log::info!("Connected to '{} ({})", task.srv_name, task.ssh_client);

                let _ = session
                    .interactive_with_channels(&shell_cmd, output_tx, input_rx)
                    .await;

                log::info!("Channel '{}' closed", task.srv_name);
            });

            let outputs = Arc::clone(&self.outputs);
            tokio::spawn(async move {
                let mut buffer = String::new();
                let colors = ["31", "32", "33", "34", "35", "36"];
                let color = colors[srv_name.len() % colors.len()];

                while let Some(output) = output_rx.recv().await {
                    buffer.push_str(&String::from_utf8_lossy(&output));

                    // process output line by line
                    while let Some(newline_pos) = buffer.find('\n') {
                        let line = buffer[..newline_pos].to_string();
                        buffer = buffer[newline_pos + 1..].to_string();

                        if !line.is_empty() {
                            println!("\x1b[{}m[{}]\x1b[0m {}", color, srv_name, line);

                            // save to history
                            outputs
                                .lock()
                                .await
                                .entry(srv_name.clone())
                                .or_default()
                                .push(line);
                        }
                    }
                }
            });
        }
        Ok(())
    }

    pub async fn distribute_input(&self, input: &[u8]) -> Result<()> {
        for (_, tx) in self.shells.iter() {
            let _ = tx.send(input.to_vec()).await;
        }
        Ok(())
    }

    pub async fn show_outputs(&self, srv_name: &str) -> Result<()> {
        let outputs = self.outputs.lock().await;

        if srv_name == "--all" {
            if outputs.is_empty() {
                println!("📝 No command history available");
                return Ok(());
            }

            println!("\n📋 Command History Summary");
            println!("{}", "═".repeat(50));

            for (srv_name, outputs) in outputs.iter() {
                self.print_server_history(srv_name, outputs);
            }
        } else {
            match outputs.get(srv_name) {
                Some(outputs) => {
                    println!("\n📋 Command History for {}", srv_name);
                    println!("{}", "═".repeat(50));
                    self.print_server_history(srv_name, outputs);
                }
                None => {
                    println!("❌ Server '{}' not found or no history available", srv_name);
                }
            }
        }

        Ok(())
    }

    fn print_server_history(&self, srv_name: &str, outputs: &[String]) {
        if outputs.is_empty() {
            println!("📝 {}: No commands executed yet", srv_name);
            return;
        }

        println!("\n🖥️  Server: {}", srv_name);
        println!("{}", "─".repeat(30));

        for (i, output) in outputs.iter().enumerate() {
            let line_num = i + 1;
            println!("{:2} │ {}", line_num, output);
        }

        println!("{}", "─".repeat(30));
        println!("📊 Total: {} commands", outputs.len());
    }
}
