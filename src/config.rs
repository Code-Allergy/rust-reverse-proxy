use std::cell::OnceCell;
use std::{fs, process};
use std::path::Path;
use std::sync::OnceLock;
use clap::Parser;
use once_cell::sync::Lazy;
use lazy_static::lazy_static;
use log::info;
use serde::Deserialize;
use tokio::sync::RwLock;
use crate::cli::Args;

#[derive(Deserialize, Default)]
pub struct Config {
    pub proxy: ProxyConfig
}
#[derive(Deserialize)]
pub struct ProxyConfig {
    pub host_port: u16,
    pub dest_port: u16
}

impl Default for ProxyConfig {
    fn default() -> Self {
        ProxyConfig {
            host_port: 8080,
            dest_port: 3000,
        }
    }
}


// Create a global static CONFIG, initially uninitialized
pub static CONFIG: OnceLock<Config> = OnceLock::new();
impl Config {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let config_content = fs::read_to_string(path)?;
        let config = toml::from_str(&config_content)?;
        Ok(config)
    }
}
pub fn initialize_config() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let config = if let Some(config_path) = args.config {
        if config_path.exists() {
            info!("Loading config from: {}", config_path.display());
            Config::load(&config_path)?
        } else {
            panic!("Config file not found at: {}", config_path.display())
        }
    } else {
        println!("No config file specified, using default configuration");
        Config::default()
    };

    // This will only fail if another thread somehow initialized CONFIG first,
    // which shouldn't happen in this use case
    CONFIG.set(config)
        .map_err(|_| "Config was already initialized")?;

    Ok(())
}

pub fn config() -> &'static Config {
    CONFIG.get().expect("Config not initialized")
}
