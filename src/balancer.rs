use std::sync::Arc;
use lazy_static::lazy_static;
use log::warn;
use tokio::sync::Mutex;
use crate::config::config;

lazy_static! {
    static ref INDEX: Arc<Mutex<usize>> = Arc::new(Mutex::new(0));
}

pub async fn get_destination() -> String {
    if !config().balancer.enabled {
        return config().proxy.destination.clone();
    }

    let strategy = config().balancer.strategy.clone();

    match strategy.as_str() {
        "round-robin" => round_robin().await,
        _ => {
            warn!("Unknown strategy: {}", strategy);
            warn!("Falling back to default destination");
            config().proxy.destination.clone()
        }
    }
}


async fn round_robin() -> String {
    let hosts = config().balancer.hosts.clone();
    let mut index = INDEX.lock().await;
    let host = hosts[*index].clone();
    *index = (*index + 1) % hosts.len();
    host
}