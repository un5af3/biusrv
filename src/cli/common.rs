/// Common functions for CLI.
use std::collections::HashMap;

use crate::config::ServerConfig;

/// List all servers.
pub fn list_servers(servers: &HashMap<String, ServerConfig>) {
    if servers.is_empty() {
        println!("📝 No servers configured");
        return;
    }

    println!("\n🖥️  Configured Servers ({})", servers.len());
    println!("{}", "─".repeat(50));

    for (name, srv_cfg) in servers.iter() {
        let auth_type = if srv_cfg.use_password.unwrap_or(false) {
            "🔐 Password"
        } else {
            "🔑 Key"
        };

        println!(
            "  {} - {}@{}:{} ({})",
            name,
            srv_cfg.username,
            srv_cfg.host,
            srv_cfg.port.unwrap_or(22),
            auth_type
        );
    }

    println!("{}", "─".repeat(50));
}
