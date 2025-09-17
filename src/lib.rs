/// Configuration serialization and deserialization.
pub mod config;

/// SSH related functionality.
pub mod ssh;

/// Transfer related functionality.
pub mod transfer;

/// Server initialization functionality.
pub mod init;

/// Firewall management functionality.
pub mod firewall;

/// Utility functions for common operations.
pub mod utils;

/// Fail2ban management functionality.
pub mod fail2ban;

/// Script execution functionality.
pub mod script;

/// CLI interface and commands.
pub mod cli;

/// Macro for retrying operations with exponential backoff
#[macro_export]
macro_rules! retry_operation {
    // Simple version without logging
    ($max_retry:expr, $operation:expr) => {{
        let mut result = None;

        for attempt in 0..=$max_retry {
            match $operation {
                Ok(res) => {
                    result = Some(Ok(res));
                    break;
                }
                Err(e) => {
                    if attempt < $max_retry {
                        // Exponential backoff: 1s, 2s, 4s, 8s...
                        let delay = std::time::Duration::from_millis(1000 * (1 << attempt));
                        tokio::time::sleep(delay).await;
                    }
                    result = Some(Err(e));
                }
            }
        }

        result.unwrap()
    }};

    // Version with logging
    ($max_retry:expr, $operation:expr, $log_prefix:expr) => {{
        let mut result = None;

        for attempt in 0..=$max_retry {
            match $operation {
                Ok(res) => {
                    result = Some(Ok(res));
                    break;
                }
                Err(e) => {
                    if attempt < $max_retry {
                        log::warn!(
                            "{} failed (attempt {}/{}): {}, retrying in {}s...",
                            $log_prefix,
                            attempt + 1,
                            $max_retry + 1,
                            e,
                            1 << attempt
                        );

                        // Exponential backoff: 1s, 2s, 4s, 8s...
                        let delay = std::time::Duration::from_millis(1000 * (1 << attempt));
                        tokio::time::sleep(delay).await;
                    } else {
                        log::error!(
                            "{} failed after {} attempts: {}",
                            $log_prefix,
                            $max_retry + 1,
                            e
                        );
                    }
                    result = Some(Err(e));
                }
            }
        }

        result.unwrap()
    }};
}
