use std::sync::Arc;
use std::{collections::HashMap, future::Future};

use anyhow::Result;
use tokio::sync::{mpsc, Mutex};

use crate::config::ServerConfig;
use crate::ssh::Client;

use crate::retry_operation;

/// A task containing server name and client for execution
#[derive(Debug)]
pub struct Task {
    pub srv_name: String,
    pub ssh_client: Client,
}

impl std::fmt::Display for Task {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} - {}", self.srv_name, self.ssh_client)
    }
}

/// List all server tasks
pub fn list_tasks(tasks: &Vec<Task>) {
    if tasks.is_empty() {
        println!("📝 No servers to process");
        return;
    }

    println!("\n🎯 Target Servers ({})", tasks.len());
    println!("{}", "─".repeat(40));

    for (i, task) in tasks.iter().enumerate() {
        let server_num = i + 1;
        println!("{:2} - {}", server_num, task);
    }

    println!("{}", "─".repeat(40));
}

/// Build server tasks from server configs
pub fn build_tasks(server_config: &HashMap<String, ServerConfig>) -> Result<Vec<Task>> {
    let mut tasks = vec![];

    for (srv_name, srv_config) in server_config.iter() {
        if srv_config.use_password.unwrap_or(false) {
            println!(
                "🔐 {} ({}@{}:{}) requires password authentication",
                srv_name,
                srv_config.username,
                srv_config.host,
                srv_config.port.unwrap_or(22)
            );
        }

        tasks.push(Task {
            srv_name: srv_name.clone(),
            ssh_client: srv_config.build_client()?,
        });
    }

    Ok(tasks)
}

/// Generic concurrent task executor using producer-consumer pattern
pub async fn execute_tasks<F, Fut>(
    thread_num: usize,
    max_retry: u32,
    tasks: Vec<Task>,
    executor: F,
) -> Result<()>
where
    F: Fn(usize, Arc<Task>) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<()>> + Send + 'static,
{
    if tasks.is_empty() {
        return Ok(());
    }

    let thread_num = std::cmp::min(thread_num, tasks.len());

    let (sender, receiver) = mpsc::channel(tasks.len());
    let receiver = Arc::new(Mutex::new(receiver));
    let executor = Arc::new(executor);

    log::info!(
        "Starting execution with {} threads for {} tasks",
        thread_num,
        tasks.len()
    );

    // Spawn worker threads
    let mut handles = vec![];
    for _ in 0..thread_num {
        let receiver = Arc::clone(&receiver);
        let executor = Arc::clone(&executor);

        handles.push(tokio::spawn(async move {
            task_worker(max_retry, executor, receiver).await;
        }));
    }

    // Send all tasks to the channel
    for (idx, task) in tasks.into_iter().enumerate() {
        let _ = sender.send((idx, task)).await?;
    }
    drop(sender);

    // Wait for all workers to complete
    for handle in handles {
        let _ = handle.await;
    }

    Ok(())
}

/// Worker function that processes tasks from the channel
async fn task_worker<F, Fut>(
    max_retry: u32,
    executor: Arc<F>,
    receiver: Arc<Mutex<mpsc::Receiver<(usize, Task)>>>,
) where
    F: Fn(usize, Arc<Task>) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<()>> + Send + 'static,
{
    loop {
        let (idx, task) = match receiver.lock().await.recv().await {
            Some((idx, task)) => (idx, task),
            None => break,
        };

        let task = Arc::new(task);
        let log_prefix = format!("Server '{} ({})'", task.srv_name, task.ssh_client);

        // Use macro with logging
        let _ = retry_operation!(max_retry, executor(idx, task.clone()).await, log_prefix);
    }
}
