use std::process::{Command, Child, Stdio};
use std::thread;
use std::time::Duration;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use reqwest::blocking::Client;
use reqwest::StatusCode as ReqwestStatusCode;
use portpicker::pick_unused_port;
use serde_json::json;
use uuid::Uuid; // For unique temporary file names

// Paths to binaries, assuming `cargo test` is run from the workspace root
const PROXY_BIN: &str = "./target/debug/reverse_proxy";
const BACKEND_BIN: &str = "./target/debug/integration-test-backend";

struct TestContext {
    proxy_http_port: u16,
    proxy_https_port: Option<u16>,
    backend_ports: Vec<u16>, // Can hold multiple for load balancing
    proxy_process: Option<Child>,
    backend_processes: Vec<Child>,
    proxy_config_path: PathBuf,
    // For capturing logs
    proxy_stdout_path: PathBuf,
    proxy_stderr_path: PathBuf,
    backend_stdout_paths: Vec<PathBuf>,
    backend_stderr_paths: Vec<PathBuf>,
    test_uuid: Uuid, // To keep filenames unique per context
}

impl TestContext {
    // config_modifier allows tests to provide custom config sections
    // num_backends specifies how many backend servers to start
    fn new(
        num_backends: usize,
        tls_enabled: bool,
        config_modifier: Option<fn(&mut String, u16, Option<u16>, &Vec<u16>)>,
    ) -> Self {
        let proxy_http_port = pick_unused_port().expect("No free ports for proxy HTTP");
        let mut proxy_https_port = None;
        if tls_enabled {
            proxy_https_port = Some(pick_unused_port().expect("No free ports for proxy HTTPS"));
        }

        let mut backend_ports = Vec::new();
        for _ in 0..num_backends {
            backend_ports.push(pick_unused_port().expect("No free ports for backend"));
        }
        
        let test_uuid = Uuid::new_v4();
        let temp_dir = std::env::temp_dir();

        // Default destination is the first backend for simplicity in basic configs
        let default_destination = format!("127.0.0.1:{}", backend_ports[0]);

        let mut config_content = format!(r#"
            [proxy]
            listen_address = "127.0.0.1"
            http_host = {}
            https_host = {} 
            destination = "{}"

            [tls]
            enabled = {}
            # cert = "/path/to/cert.pem" # Required if tls.enabled = true
            # key = "/path/to/key.pem"   # Required if tls.enabled = true

            [balancer]
            enabled = false # Default, can be overridden by config_modifier
            # strategy = "round-robin"
            # hosts = []

            [reroute]
            enabled = false
            # paths = [] 
        "#, 
        proxy_http_port, 
        proxy_https_port.unwrap_or(0), // Use 0 if None, won't be used by proxy if TLS disabled
        default_destination,
        tls_enabled
    );

        if tls_enabled {
            // These are placeholders. Real certs need to be generated/provided for TLS tests.
            config_content = config_content.replace("# cert =", "cert =");
            config_content = config_content.replace("# key =", "key =");
            config_content = config_content.replace("/path/to/cert.pem", "test_certs/cert.pem");
            config_content = config_content.replace("/path/to/key.pem", "test_certs/key.pem");
            // Ensure test_certs directory and dummy files exist for config validation if needed
            // For actual TLS tests, these files must be valid.
        }


        if let Some(modifier) = config_modifier {
            modifier(&mut config_content, proxy_http_port, proxy_https_port, &backend_ports);
        }
        
        let proxy_config_path = temp_dir.join(format!("proxy_config_{}.toml", test_uuid));
        let proxy_stdout_path = temp_dir.join(format!("proxy_stdout_{}.log", test_uuid));
        let proxy_stderr_path = temp_dir.join(format!("proxy_stderr_{}.log", test_uuid));
        
        let mut backend_stdout_paths = Vec::new();
        let mut backend_stderr_paths = Vec::new();

        let mut file = File::create(&proxy_config_path).expect("Failed to create temp proxy config");
        file.write_all(config_content.as_bytes()).expect("Failed to write proxy config");
        
        println!("[SETUP][{}] Proxy config: {}", test_uuid, proxy_config_path.display());
        println!("[SETUP][{}] Proxy HTTP: {}, HTTPS: {:?}, Backends: {:?}", test_uuid, proxy_http_port, proxy_https_port, backend_ports);

        let proxy_stdout_file = File::create(&proxy_stdout_path).expect("Failed to create proxy stdout log");
        let proxy_stderr_file = File::create(&proxy_stderr_path).expect("Failed to create proxy stderr log");

        let mut backend_processes = Vec::new();
        for (i, &port) in backend_ports.iter().enumerate() {
            let backend_stdout_path = temp_dir.join(format!("backend_{}_stdout_{}.log", i, test_uuid));
            let backend_stderr_path = temp_dir.join(format!("backend_{}_stderr_{}.log", i, test_uuid));
            let backend_stdout_file = File::create(&backend_stdout_path).expect("Failed to create backend stdout log");
            let backend_stderr_file = File::create(&backend_stderr_path).expect("Failed to create backend stderr log");

            let backend_process = Command::new(Path::new(BACKEND_BIN).canonicalize().expect("Backend binary not found"))
                .env("BACKEND_PORT", port.to_string())
                .env("RUST_LOG", "info")
                .stdout(Stdio::from(backend_stdout_file))
                .stderr(Stdio::from(backend_stderr_file))
                .spawn()
                .expect(&format!("Failed to start backend server #{}", i));
            println!("[SETUP][{}] Started backend server #{} (PID: {}, Port: {})", test_uuid, i, backend_process.id(), port);
            backend_processes.push(backend_process);
            backend_stdout_paths.push(backend_stdout_path);
            backend_stderr_paths.push(backend_stderr_path);
        }

        // Start reverse proxy
        let proxy_process = Command::new(Path::new(PROXY_BIN).canonicalize().expect("Proxy binary not found"))
            .arg("--config")
            .arg(&proxy_config_path)
            .env("RUST_LOG", "info")
            .stdout(Stdio::from(proxy_stdout_file))
            .stderr(Stdio::from(proxy_stderr_file))
            .spawn()
            .expect("Failed to start reverse proxy");
        println!("[SETUP][{}] Started reverse proxy (PID: {})", test_uuid, proxy_process.id());

        thread::sleep(Duration::from_secs(3)); // Wait for servers

        TestContext {
            proxy_http_port,
            proxy_https_port,
            backend_ports,
            proxy_process: Some(proxy_process),
            backend_processes,
            proxy_config_path,
            proxy_stdout_path,
            proxy_stderr_path,
            backend_stdout_paths,
            backend_stderr_paths,
            test_uuid,
        }
    }

    fn proxy_url(&self, path: &str, use_https: bool) -> String {
        if use_https {
            format!("https://127.0.0.1:{}{}", self.proxy_https_port.expect("HTTPS port not set for TestContext"), path)
        } else {
            format!("http://127.0.0.1:{}{}", self.proxy_http_port, path)
        }
    }

    fn print_log_files(&self) {
        println!("[LOGS][{}] Proxy stdout ({}):\n{}", self.test_uuid, self.proxy_stdout_path.display(), fs::read_to_string(&self.proxy_stdout_path).unwrap_or_default());
        println!("[LOGS][{}] Proxy stderr ({}):\n{}", self.test_uuid, self.proxy_stderr_path.display(), fs::read_to_string(&self.proxy_stderr_path).unwrap_or_default());
        for i in 0..self.backend_stdout_paths.len() {
            println!("[LOGS][{}] Backend #{} stdout ({}):\n{}", self.test_uuid, i, self.backend_stdout_paths[i].display(), fs::read_to_string(&self.backend_stdout_paths[i]).unwrap_or_default());
            println!("[LOGS][{}] Backend #{} stderr ({}):\n{}", self.test_uuid, i, self.backend_stderr_paths[i].display(), fs::read_to_string(&self.backend_stderr_paths[i]).unwrap_or_default());
        }
    }
}

impl Drop for TestContext {
    fn drop(&mut self) {
        println!("[TEARDOWN][{}] Cleaning up TestContext...", self.test_uuid);
        self.print_log_files(); 

        if let Some(mut child) = self.proxy_process.take() {
            println!("[TEARDOWN][{}] Killing proxy process (PID: {})...", self.test_uuid, child.id());
            if let Err(e) = child.kill() { eprintln!("[TEARDOWN][{}] Failed to kill proxy process: {}", self.test_uuid, e); }
            match child.wait() {
                Ok(status) => println!("[TEARDOWN][{}] Proxy process exited with: {}", self.test_uuid, status),
                Err(e) => eprintln!("[TEARDOWN][{}] Failed to wait for proxy process: {}", self.test_uuid, e),
            }
        }
        for (i, mut child) in self.backend_processes.drain(..).enumerate() {
            println!("[TEARDOWN][{}] Killing backend process #{} (PID: {})...", self.test_uuid, i, child.id());
            if let Err(e) = child.kill() { eprintln!("[TEARDOWN][{}] Failed to kill backend process #{}: {}", self.test_uuid, i, e); }
            match child.wait() {
                Ok(status) => println!("[TEARDOWN][{}] Backend process #{} exited with: {}", self.test_uuid, i, status),
                Err(e) => eprintln!("[TEARDOWN][{}] Failed to wait for backend process #{}: {}", self.test_uuid, i, e),
            }
        }

        let paths_to_delete = [&self.proxy_config_path, &self.proxy_stdout_path, &self.proxy_stderr_path];
        for path in paths_to_delete.iter() {
            if path.exists() {
                println!("[TEARDOWN][{}] Deleting temp file: {}", self.test_uuid, path.display());
                if let Err(e) = fs::remove_file(path) { eprintln!("[TEARDOWN][{}] Failed to delete temp file {}: {}", self.test_uuid, path.display(), e); }
            }
        }
        for path in self.backend_stdout_paths.iter().chain(self.backend_stderr_paths.iter()) {
             if path.exists() {
                println!("[TEARDOWN][{}] Deleting temp backend log file: {}", self.test_uuid, path.display());
                if let Err(e) = fs::remove_file(path) { eprintln!("[TEARDOWN][{}] Failed to delete temp file {}: {}", self.test_uuid, path.display(), e); }
            }
        }
        println!("[TEARDOWN][{}] TestContext cleanup finished.", self.test_uuid);
    }
}

#[test]
fn test_basic_get_passthrough() {
    let ctx = TestContext::new(1, false, None); // 1 backend, TLS false, no custom config
    let client = Client::new();

    let url = ctx.proxy_url("/test_get_path", false);
    println!("[TEST][{}] Testing GET request to: {}", ctx.test_uuid, url);
    let response = client.get(&url).send().expect("Request failed");

    assert_eq!(response.status(), ReqwestStatusCode::OK);
    assert_eq!(response.headers().get("X-Backend-Echo-Path").unwrap().to_str().unwrap(), "/test_get_path");
    assert_eq!(response.text().unwrap(), "Backend server received request for: /test_get_path");
}

#[test]
fn test_basic_post_passthrough() {
    let ctx = TestContext::new(1, false, None);
    let client = Client::new();

    let post_body = json!({"message": "hello from integration test"});
    let body_str = post_body.to_string();
    let body_len = body_str.len();

    let url = ctx.proxy_url("/test_post_path", false);
    println!("[TEST][{}] Testing POST request to: {}",  ctx.test_uuid,url);
    let response = client.post(&url)
        .header("Content-Type", "application/json")
        .body(body_str)
        .send()
        .expect("Request failed");

    assert_eq!(response.status(), ReqwestStatusCode::OK);
    assert_eq!(response.headers().get("X-Backend-Echo-Path").unwrap().to_str().unwrap(), "/test_post_path");
    assert_eq!(response.headers().get("X-Backend-Echo-Body-Len").unwrap().to_str().unwrap(), body_len.to_string());
    
    let expected_response_body = format!("Backend server received POST on /test_post_path with body length: {}", body_len);
    assert_eq!(response.text().unwrap(), expected_response_body);
}

#[test]
fn test_rerouting() {
    let config_modifier = |config_string: &mut String, _proxy_http_port: u16, _proxy_https_port: Option<u16>, backend_ports: &Vec<u16>| {
        let reroute_section = format!(r#"
            [reroute]
            enabled = true
            paths = [
                {{ from = "/api/v1/data", to = "http://127.0.0.1:{}/internal/data" }}
            ]
        "#, backend_ports[0]); // Reroute to the first backend's specific path

        if let Some(idx) = config_string.find("[reroute]") {
            // Find end of [reroute] section or end of string.
            // This is a bit fragile; assumes sections are separated by double newlines or it's the last section.
            let end_idx = config_string[idx..].find("\n\n[").map_or_else(
                || config_string.len(), 
                |next_section_start| idx + next_section_start
            );
            config_string.replace_range(idx..end_idx, &reroute_section);
        } else {
            config_string.push_str(&reroute_section);
        }
    };

    let ctx = TestContext::new(1, false, Some(config_modifier));
    let client = Client::new();

    let url = ctx.proxy_url("/api/v1/data/item123?query=true", false);
    println!("[TEST][{}] Testing GET request with rerouting to: {}", ctx.test_uuid, url);
    let response = client.get(&url).send().expect("Request failed");

    assert_eq!(response.status(), ReqwestStatusCode::OK);
    assert_eq!(response.headers().get("X-Backend-Echo-Path").unwrap().to_str().unwrap(), "/internal/data/item123?query=true");
    assert_eq!(response.text().unwrap(), "Backend server received request for: /internal/data/item123?query=true");
}

#[test]
#[ignore = "TLS setup with self-signed certificates is required and not yet implemented for automated tests"]
fn test_tls_passthrough() {
    // 1. Generate self-signed certificate (cert.pem, key.pem) and place in `test_certs/` directory (or make paths configurable).
    //    This step is manual or requires scripting outside the test itself.
    //    Example: `mkdir -p test_certs && openssl req -x509 -newkey rsa:2048 -nodes -keyout test_certs/key.pem -out test_certs/cert.pem -sha256 -days 365 -subj "/CN=localhost"`
    //    Ensure PROXY_BIN can access these files.

    // Create dummy cert files if they don't exist to allow config parsing, but test will be ignored.
    if !Path::new("test_certs").exists() { std::fs::create_dir("test_certs").ok(); }
    if !Path::new("test_certs/cert.pem").exists() { File::create("test_certs/cert.pem").ok(); }
    if !Path::new("test_certs/key.pem").exists() { File::create("test_certs/key.pem").ok(); }
    
    let ctx = TestContext::new(1, true, None); // TLS true
    
    // reqwest client needs to trust the self-signed certificate or ignore validation
    let client = Client::builder()
        .danger_accept_invalid_certs(true) // For testing with self-signed certs ONLY
        .build()
        .unwrap();

    let url = ctx.proxy_url("/test_tls_get_path", true); // true for HTTPS
    println!("[TEST][{}] Testing GET request over TLS to: {}", ctx.test_uuid, url);
    
    match client.get(&url).send() {
        Ok(response) => {
            assert_eq!(response.status(), ReqwestStatusCode::OK);
            assert_eq!(response.headers().get("X-Backend-Echo-Path").unwrap().to_str().unwrap(), "/test_tls_get_path");
            assert_eq!(response.text().unwrap(), "Backend server received request for: /test_tls_get_path");
        }
        Err(e) => {
            // If the error is due to certs not being set up, this will fail.
            // The #[ignore] attribute should prevent this from running in CI unless certs are set up.
            panic!("TLS request failed: {}. Ensure test_certs/cert.pem and test_certs/key.pem are valid and accessible.", e);
        }
    }
}

#[test]
#[ignore = "Complex to verify round-robin behavior precisely without backend modification or specific headers from backend instances"]
fn test_load_balancing_round_robin() {
    let num_backends = 2;
    let config_modifier = |config_string: &mut String, _proxy_http_port: u16, _proxy_https_port: Option<u16>, backend_ports: &Vec<u16>| {
        let hosts_list = backend_ports.iter().map(|p| format!("\"127.0.0.1:{}\"", p)).collect::<Vec<_>>().join(", ");
        let balancer_section = format!(r#"
            [balancer]
            enabled = true
            strategy = "round-robin"
            hosts = [{}]
        "#, hosts_list);
        
        // Replace default [balancer] and [proxy].destination
        if let Some(idx) = config_string.find("[balancer]") {
            let end_idx = config_string[idx..].find("\n\n[").map_or_else(|| config_string.len(), |i| idx + i);
            config_string.replace_range(idx..end_idx, &balancer_section);
        } else {
            config_string.push_str(&balancer_section);
        }
        // Remove or comment out the main proxy.destination as balancer will be used
        if let Some(idx) = config_string.find("destination =") {
             let line_end_idx = config_string[idx..].find('\n').map_or_else(|| config_string.len(), |i| idx + i);
             config_string.replace_range(idx..line_end_idx, "# destination = \"..." ); // Comment it out
        }
    };

    let ctx = TestContext::new(num_backends, false, Some(config_modifier));
    let client = Client::new();

    let mut responses_from_ports = std::collections::HashSet::new();
    // It's hard to guarantee perfect round-robin distribution in a short number of requests
    // and without unique identifiers from backend instances.
    // This test will just check if requests are hitting more than one backend if possible.
    // For more robust testing, backends would need to return their specific port or an ID.
    println!("[TEST][{}] Testing round-robin load balancing. Sending {} requests...", ctx.test_uuid, num_backends * 2);
    for i in 0..(num_backends * 2) {
        let url = ctx.proxy_url(&format!("/lb_test_{}", i), false);
        match client.get(&url).send() {
            Ok(response) => {
                if response.status() == ReqwestStatusCode::OK {
                    // Ideally, we'd get the backend port from a response header.
                    // For now, we assume the X-Backend-Echo-Path reflects the backend it hit.
                    // This isn't a perfect test for LB as both backends behave identically.
                    // A real test would involve backends returning unique IDs or ports.
                    println!("[TEST][{}] LB Request {} got OK. Backend path: {}", ctx.test_uuid, i, response.headers().get("X-Backend-Echo-Path").unwrap().to_str().unwrap());
                    // This test currently doesn't effectively verify round-robin.
                    // To make it work, backend needs to provide its listening port.
                    // For now, just ensure requests pass.
                } else {
                     println!("[TEST][{}] LB Request {} failed: status {}",ctx.test_uuid, i, response.status());
                }
            }
            Err(e) => {
                 println!("[TEST][{}] LB Request {} error: {}",ctx.test_uuid, i, e);
            }
        }
    }
    // Assert that we hit more than one backend if possible (placeholder for real verification)
    // assertTrue(responses_from_ports.len() > 1 || num_backends <= 1); 
    println!("[TEST][{}] Load balancing test finished. Manual log inspection recommended for round-robin verification.", ctx.test_uuid);
}
