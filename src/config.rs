use std::cell::OnceCell;
use std::{fs, process};
use std::collections::HashMap;
use std::iter::Map;
use std::path::{Path, PathBuf};
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
    pub proxy: ProxyConfig,
    pub tls: TlsConfig,
    pub balancer: BalancerConfig,
    pub reroute: RerouteConfig,
}
#[derive(Deserialize)]
pub struct ProxyConfig {
    pub http_host: u16,
    pub https_host: u16,
    pub destination: String
}

#[derive(Deserialize)]
pub struct TlsConfig {
    pub enabled: bool,
    pub cert: Option<PathBuf>,
    pub key: Option<PathBuf>,
}

#[derive(Deserialize)]
pub struct BalancerConfig {
    pub enabled: bool,
    pub strategy: String,
    pub hosts: Vec<String>,
}

#[derive(Deserialize)]
pub struct RerouteConfig {
    pub enabled: bool,
    // TODO map from path to destination
    pub paths: Vec<PathMapping>
}

#[derive(Deserialize, Debug)]
pub struct PathMapping {
    pub(crate) from: String,
    pub(crate) to: String,
    pub description: Option<String>, // You can store an optional description
}


impl Default for ProxyConfig {
    fn default() -> Self {
        ProxyConfig {
            http_host: 8080,
            https_host: 8443,
            destination: "127.0.0.1:3000".into(),
        }
    }
}

impl Default for TlsConfig {
    fn default() -> Self {
        TlsConfig {
            enabled: false,
            cert: None,
            key: None,
        }
    }
}

impl Default for BalancerConfig {
    fn default() -> Self {
        BalancerConfig {
            enabled: false,
            strategy: "round-robin".into(),
            hosts: vec![]
        }
    }
}

impl Default for RerouteConfig {
    fn default() -> Self {
        RerouteConfig {
            enabled: false,
            paths: vec![]
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
