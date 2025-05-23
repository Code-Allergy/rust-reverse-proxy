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
    pub listen_address: String,
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

// Helper function to deserialize Vec<PathMapping> into HashMap<String, String>
mod path_mapping_deserializer {
    use serde::{Deserialize, Deserializer};
    use std::collections::HashMap;
    use super::PathMapping; // Keep PathMapping for deserialization

    pub fn deserialize<'de, D>(deserializer: D) -> Result<HashMap<String, String>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let mappings = Vec::<PathMapping>::deserialize(deserializer)?;
        let mut map = HashMap::new();
        for mapping in mappings {
            map.insert(mapping.from, mapping.to);
        }
        Ok(map)
    }
}

#[derive(Deserialize)]
pub struct RerouteConfig {
    pub enabled: bool,
    #[serde(deserialize_with = "path_mapping_deserializer::deserialize", default)]
    pub paths: HashMap<String, String>
}

// PathMapping is still needed for the custom deserializer
#[derive(Deserialize, Debug)]
struct PathMapping {
    from: String,
    to: String,
    description: Option<String>, 
}


impl Default for ProxyConfig {
    fn default() -> Self {
        ProxyConfig {
            listen_address: "127.0.0.1".to_string(),
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
            paths: HashMap::new() // Initialize with an empty HashMap
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
            // Return an error instead of panicking
            return Err(format!("Config file not found at: {}", config_path.display()).into());
        }
    } else {
        info!("No config file specified, using default configuration");
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use std::io::Write;
    use std::path::PathBuf;

    // Helper function to create a temporary config file
    fn create_temp_config_file(content: &str) -> PathBuf {
        let mut dir = std::env::temp_dir();
        dir.push(format!("test_config_{}.toml", uuid::Uuid::new_v4()));
        let path = dir;
        let mut file = File::create(&path).expect("Failed to create temp config file");
        writeln!(file, "{}", content).expect("Failed to write to temp config file");
        path
    }

    #[test]
    fn test_proxy_config_defaults() {
        let proxy_config = ProxyConfig::default();
        assert_eq!(proxy_config.listen_address, "127.0.0.1");
        assert_eq!(proxy_config.http_host, 8080);
        assert_eq!(proxy_config.https_host, 8443);
        assert_eq!(proxy_config.destination, "127.0.0.1:3000");
    }

    #[test]
    fn test_tls_config_defaults() {
        let tls_config = TlsConfig::default();
        assert_eq!(tls_config.enabled, false);
        assert!(tls_config.cert.is_none());
        assert!(tls_config.key.is_none());
    }

    #[test]
    fn test_balancer_config_defaults() {
        let balancer_config = BalancerConfig::default();
        assert_eq!(balancer_config.enabled, false);
        assert_eq!(balancer_config.strategy, "round-robin");
        assert!(balancer_config.hosts.is_empty());
    }

    #[test]
    fn test_reroute_config_defaults() {
        let reroute_config = RerouteConfig::default();
        assert_eq!(reroute_config.enabled, false);
        assert!(reroute_config.paths.is_empty());
    }

    #[test]
    fn test_config_default_creation() {
        let config = Config::default();
        assert_eq!(config.proxy.listen_address, "127.0.0.1");
        assert_eq!(config.reroute.paths.len(), 0);
        assert_eq!(config.tls.enabled, false);
        assert_eq!(config.balancer.enabled, false);
    }
    
    #[test]
    fn test_config_load_valid_toml_string() {
        let toml_content = r#"
            [proxy]
            listen_address = "0.0.0.0"
            http_host = 8000
            https_host = 8440
            destination = "example.com:80"

            [tls]
            enabled = true
            cert = "/path/to/cert.pem"
            key = "/path/to/key.pem"

            [balancer]
            enabled = true
            strategy = "random"
            hosts = ["host1:80", "host2:80"]

            [reroute] # Added [reroute] table explicitly
            enabled = true # Added enabled field
            [[reroute.paths]]
            from = "/service_a"
            to = "http://localhost:8081"
            description = "Service A route"

            [[reroute.paths]]
            from = "/service_b"
            to = "http://localhost:8082"
        "#;
        
        let temp_file_path = create_temp_config_file(toml_content);
        let loaded_config = Config::load(&temp_file_path).expect("Failed to load config from string");

        assert_eq!(loaded_config.proxy.listen_address, "0.0.0.0");
        assert_eq!(loaded_config.proxy.http_host, 8000);
        assert_eq!(loaded_config.tls.enabled, true);
        assert_eq!(loaded_config.tls.cert.unwrap(), PathBuf::from("/path/to/cert.pem"));
        assert_eq!(loaded_config.balancer.strategy, "random");
        assert_eq!(loaded_config.balancer.hosts, vec!["host1:80", "host2:80"]);
        
        assert_eq!(loaded_config.reroute.paths.len(), 2);
        assert_eq!(loaded_config.reroute.paths.get("/service_a").unwrap(), "http://localhost:8081");
        assert_eq!(loaded_config.reroute.paths.get("/service_b").unwrap(), "http://localhost:8082");

        fs::remove_file(temp_file_path).expect("Failed to delete temp config file");
    }

    #[test]
    fn test_config_load_non_existent_file() {
        let result = Config::load("non_existent_config.toml");
        assert!(result.is_err());
        // Check for a specific IO error kind if possible/needed, e.g., NotFound
        // For now, is_err() is sufficient as per requirements
    }

    #[test]
    fn test_config_load_invalid_toml_syntax() {
        let invalid_toml_content = "this is not valid toml";
        let temp_file_path = create_temp_config_file(invalid_toml_content);
        let result = Config::load(&temp_file_path);
        assert!(result.is_err());
        // We could check the specific error type from toml::de::Error if needed
        fs::remove_file(temp_file_path).expect("Failed to delete temp config file");
    }

    #[test]
    fn test_config_load_reroute_paths_deserialization() {
        let toml_content = r#"
            [proxy]
            listen_address = "127.0.0.1"
            http_host = 8080
            https_host = 8443
            destination = "127.0.0.1:3000"

            [tls]
            enabled = false

            [balancer]
            enabled = false
            strategy = "round-robin"
            hosts = []

            [reroute]
            enabled = true # This field was missing
            [[reroute.paths]]
            from = "/api/v1"
            to = "http://service1/api"

            [[reroute.paths]]
            from = "/api/v2"
            to = "http://service2/api"
        "#;
        let temp_file_path = create_temp_config_file(toml_content);
        let config = Config::load(&temp_file_path).expect("Should load successfully");
        
        assert!(config.reroute.enabled);
        assert_eq!(config.reroute.paths.len(), 2);
        assert_eq!(config.reroute.paths.get("/api/v1").unwrap(), "http://service1/api");
        assert_eq!(config.reroute.paths.get("/api/v2").unwrap(), "http://service2/api");

        fs::remove_file(temp_file_path).expect("Failed to delete temp config file");
    }

    #[test]
    fn test_proxy_listen_address_deserialization() {
         let toml_content = r#"
            [proxy]
            listen_address = "192.168.1.100"
            http_host = 80
            https_host = 443
            destination = "dest"

            # Add other required sections for Config::load to work
            [tls]
            enabled = false

            [balancer]
            enabled = false
            strategy = "round-robin"
            hosts = []

            [reroute]
            enabled = false
            # paths can be empty if not being tested, due to #[serde(default)] on paths field
            # or provide empty array: paths = [] 
        "#;
        let temp_file_path = create_temp_config_file(toml_content);
        let config = Config::load(&temp_file_path).expect("Should load successfully");
        assert_eq!(config.proxy.listen_address, "192.168.1.100");
        fs::remove_file(temp_file_path).expect("Failed to delete temp config file");
    }

    // Note on initialize_config:
    // Fully unit testing `initialize_config` is challenging due to its direct use of
    // `Args::parse()` (from clap, which parses command-line arguments) and its
    // side effect of setting the global static `CONFIG`.
    //
    // - Mocking `Args::parse()` would require a mocking library or feature flags
    //   to inject test arguments.
    // - Testing the global `CONFIG` set can lead to test interdependencies if not
    //   handled carefully (e.g., ensuring `CONFIG` is reset or only set once).
    //
    // The core logic of loading from a path or using defaults is covered by
    // `Config::load` tests and default value tests.
    // For `initialize_config` specifically:
    //   - The "config file not found" error path is testable if we could mock `Args`
    //     to provide a non-existent path. (This is implicitly tested by Config::load with non_existent_file)
    //   - The "default config" path is testable if `Args::parse()` returns no config path.
    // These scenarios are better suited for integration tests where command-line
    // arguments can be simulated or for tests that can manage the global state of `CONFIG`.
}
