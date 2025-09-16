use std::str::FromStr;

use clap::Parser;
use log::{error, LevelFilter};

use biusrv::cli::{Cli, Commands};
use biusrv::component::manager::ComponentManager;
use biusrv::config::Config;

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    // init logger
    env_logger::Builder::from_default_env()
        .filter_level(LevelFilter::from_str(&cli.log_level).unwrap_or(LevelFilter::Info))
        .init();

    let config = match Config::load(cli.config) {
        Ok(cfg) => cfg,
        Err(e) => {
            error!("Failed to load config: {}", e);
            std::process::exit(1);
        }
    };

    // Parse component manager
    let component_manager = match ComponentManager::build(&cli.component_path) {
        Ok(manager) => manager,
        Err(e) => {
            error!(
                "Failed to load components from '{}': {}",
                cli.component_path, e
            );
            std::process::exit(1);
        }
    };

    match cli.command {
        Commands::Init(init_cmd) => {
            if let Some(init_config) = &config.init {
                if let Err(e) = init_cmd.execute(init_config).await {
                    error!("Init command failed: {}", e);
                    std::process::exit(1);
                }
            } else {
                error!("Init configuration not found");
                std::process::exit(1);
            }
        }
        Commands::Manage(manage_cmd) => {
            if let Some(manage_config) = &config.manage {
                if let Err(e) = manage_cmd.execute(manage_config, component_manager).await {
                    error!("Manage command failed: {}", e);
                    std::process::exit(1);
                }
            } else {
                error!("Manage configuration not found");
                std::process::exit(1);
            }
        }
    }
}
