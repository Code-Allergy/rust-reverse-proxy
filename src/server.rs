use hyper::server::conn::http1::Builder;
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;
use std::net::SocketAddr;
use std::sync::Arc;
use crate::{proxy, config::CONFIG};
use log::{error, info};
use std::fs::File;
use std::io::BufReader;
use crate::config::config;
use rustls::{ServerConfig};
use rustls_pemfile::{certs, pkcs8_private_keys, private_key};
use tokio_rustls::TlsAcceptor;
use crate::tls::{init_tls, redirect_to_https};

pub async fn run_server() -> Result<(), Box<dyn std::error::Error>> {
    let tls_acceptor = if config().tls.enabled {
        info!("TLS enabled");
        Some(init_tls())
    } else {
        info!("TLS disabled");
        None
    };

    // TODO handle disabling TLS
    let http_addr = SocketAddr::from(([127, 0, 0, 1], config().proxy.http_host));
    let https_addr = SocketAddr::from(([127, 0, 0, 1], config().proxy.https_host));
    info!("Starting HTTP proxy on 127.0.0.1:{}", config().proxy.http_host);
    info!("Starting HTTPS proxy on 127.0.0.1:{}", config().proxy.https_host);
    info!("Sending traffic to {}", config().proxy.destination);

    let http_listener = TcpListener::bind(http_addr).await?;
    let https_listener = TcpListener::bind(https_addr).await?;

    // Run both servers concurrently
    tokio::select! {
        result = run_http_server(http_listener) => {
            if let Err(e) = result {
                error!("HTTP server error: {}", e);
            }
        },
        result = run_https_server(https_listener, tls_acceptor.unwrap()) => {
            if let Err(e) = result {
                error!("HTTPS server error: {}", e);
            }
        },
    }

    Ok(())
}

async fn run_http_server(listener: TcpListener) -> Result<(), Box<dyn std::error::Error>> {
    loop {
        let (stream, client_addr) = listener.accept().await?;
        let io = TokioIo::new(stream);

        tokio::spawn(async move {
            let service = hyper::service::service_fn(move |req| {
                redirect_to_https(req, client_addr)
            });

            if let Err(err) = Builder::new().serve_connection(io, service).await {
                error!("Failed to serve HTTP connection: {:?}", err);
            }
        });
    }
}

async fn run_https_server(listener: TcpListener, tls_acceptor: TlsAcceptor) -> Result<(), Box<dyn std::error::Error>> {
    loop {
        let (stream, client_addr) = listener.accept().await?;
        let tls_acceptor = tls_acceptor.clone();

        tokio::spawn(async move {
            match tls_acceptor.accept(stream).await {
                Ok(tls_stream) => {
                    let io = TokioIo::new(tls_stream);
                    let service = hyper::service::service_fn(move |req| {
                        proxy::handle_request(req, client_addr)
                    });

                    if let Err(err) = Builder::new().serve_connection(io, service).await {
                        error!("Failed to serve HTTPS connection: {:?}", err);
                    }
                }
                Err(err) => {
                    error!("Failed to establish TLS connection: {:?}", err);
                }
            }
        });
    }
}