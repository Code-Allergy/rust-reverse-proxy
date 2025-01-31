mod config;
mod headers;
mod proxy;
mod server;
mod cli;
mod tls;
mod balancer;
mod reroute;

use std::sync::Once;
use config::Config;
use once_cell::sync::Lazy;
use crate::cli::{Args, build_cli};
use clap::Parser;
use crate::config::initialize_config;

static INIT: Once = Once::new();

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    pretty_env_logger::formatted_builder()
        .filter(None, log::LevelFilter::Info)
        .init();

    initialize_config().expect("Failed to load config.");

    if let Err(err) = server::run_server().await {
        log::error!("Server error: {:?}", err);
    }

    Ok(())
}
