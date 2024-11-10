use hyper::server::conn::http1::Builder;
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;
use std::net::SocketAddr;
use crate::{proxy, config::CONFIG};
use log::{error, info};
use crate::config::config;

pub async fn run_server() -> Result<(), Box<dyn std::error::Error>> {
    let addr = SocketAddr::from(([127, 0, 0, 1], config().proxy.host_port));
    info!("Starting proxy on 127.0.0.1:{}", config().proxy.host_port);

    let listener = TcpListener::bind(addr).await?;
    loop {
        let (stream, client_addr) = listener.accept().await?;
        let io = TokioIo::new(stream);

        // Spawn a new task for each connection
        tokio::spawn(async move {
            let service = hyper::service::service_fn(move |req| {
                proxy::handle_request(req, client_addr)
            });

            if let Err(err) = Builder::new().serve_connection(io, service).await {
                error!("Failed to serve connection: {:?}", err);
            }
        });
    }
}