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
use crate::tls; // Changed from init_tls, redirect_to_https
use std::future::Future;
use std::pin::Pin;
use hyper::{Request, Response, body::Incoming};
use http_body_util::combinators::BoxBody;
use http_body_util::BodyExt; // For resp.map(|b| b.boxed())
use bytes::Bytes;

// Define a type alias for the service response future
type ServiceFuture = Pin<Box<dyn Future<Output = Result<Response<BoxBody<Bytes, hyper::Error>>, hyper::Error>> + Send>>;


pub async fn run_server() -> Result<(), Box<dyn std::error::Error>> {
    let tls_enabled = config().tls.enabled;
    let tls_acceptor = if tls_enabled {
        info!("TLS enabled");
        Some(tls::init_tls()?)
    } else {
        info!("TLS disabled");
        None
    };

    // Parse the listen address from config
    let listen_address_str = &config().proxy.listen_address;
    let ip_addr: std::net::IpAddr = listen_address_str.parse().unwrap_or_else(|e| {
        log::error!("Invalid listen_address \"{}\": {}. Defaulting to 127.0.0.1.", listen_address_str, e);
        std::net::IpAddr::from([127, 0, 0, 1])
    });

    let http_addr = SocketAddr::from((ip_addr, config().proxy.http_host));
    info!("Starting HTTP proxy on {}:{}", ip_addr, config().proxy.http_host);

    if config().balancer.enabled {
        info!("Using balancer with strategy: {}", config().balancer.strategy);
        info!("Available hosts: {:?}", config().balancer.hosts);
    } else {
        info!("Sending traffic to {}", config().proxy.destination);
    }

    let http_listener = TcpListener::bind(http_addr).await?;

    if let Some(tls_acceptor_inner) = tls_acceptor {
        let https_addr = SocketAddr::from((ip_addr, config().proxy.https_host)); // Use parsed ip_addr
        info!("Starting HTTPS proxy on {}:{}", ip_addr, config().proxy.https_host);
        let https_listener = TcpListener::bind(https_addr).await?;

        tokio::select! {
            res_http = run_http_server(http_listener, true) => { // Pass true for tls_enabled
                if let Err(e) = res_http {
                    error!("HTTP server error: {}", e);
                }
            },
            res_https = run_https_server(https_listener, tls_acceptor_inner) => {
                if let Err(e) = res_https {
                    error!("HTTPS server error: {}", e);
                }
            },
        }
    } else {
        // Run only HTTP server
        if let Err(e) = run_http_server(http_listener, false).await { // Pass false for tls_enabled
            error!("HTTP server error: {}", e);
        }
    }

    Ok(())
}

async fn run_http_server(listener: TcpListener, tls_is_active: bool) -> Result<(), Box<dyn std::error::Error>> {
    loop {
        let (stream, client_addr) = listener.accept().await?; // Get client_addr
        let io = TokioIo::new(stream);

        tokio::spawn(async move {
            let service = hyper::service::service_fn(move |req: Request<Incoming>| -> ServiceFuture {
                if tls_is_active {
                    Box::pin(tls::redirect_to_https(req))
                } else {
                    Box::pin(async move {
                        let response = proxy::handle_request(req, client_addr)
                            .await
                            .unwrap() // Infallible, so unwrap is safe
                            .map(|b| b.boxed()); // Convert body
                        Ok(response)
                    })
                }
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