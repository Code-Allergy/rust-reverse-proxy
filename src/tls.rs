use std::convert::Infallible;
use std::fs::File;
use std::io::BufReader;
use std::sync::Arc;
use bytes::Bytes;
use http_body_util::combinators::BoxBody;
use http_body_util::{BodyExt, Empty};
use hyper::{Request, Response, StatusCode};
use hyper::header::LOCATION;
use rustls::ServerConfig;
use rustls_pemfile::{certs, private_key};
use tokio_rustls::TlsAcceptor;
use crate::config::config;

pub fn init_tls() -> Result<TlsAcceptor, Box<dyn std::error::Error>> {
    let cert_file = File::open(config().tls.cert.clone().unwrap())?;
    let key_file = File::open(config().tls.key.clone().unwrap())?;
    let cert_reader = &mut BufReader::new(cert_file);
    let key_reader = &mut BufReader::new(key_file);

    // Parse certificates and private key
    let certs = certs(cert_reader)
        .collect::<Result<Vec<_>, _>>()?;

    let key = private_key(key_reader)?
        .ok_or("no private key found")?;

    let tls_cfg = ServerConfig::builder()
        // .with_safe_default_protocol_versions()
        .with_no_client_auth()
        .with_single_cert(certs, key)?;

    Ok(TlsAcceptor::from(Arc::new(tls_cfg)))
}

// Handle HTTP to HTTPS redirect
pub async fn redirect_to_https(req: Request<hyper::body::Incoming>) -> Result<Response<BoxBody<Bytes, hyper::Error>>, hyper::Error> {
    let host = req.headers()
        .get("host")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("localhost");

    // Remove port if present and use HTTPS port
    let host = host.split(':').next().unwrap_or(host);
    let https_uri = format!("https://{}:{}{}",
                            host,
                            config().proxy.https_host,
                            req.uri().path_and_query().map(|x| x.as_str()).unwrap_or("")
    );

    Ok(Response::builder()
        .status(StatusCode::MOVED_PERMANENTLY)
        .header(LOCATION, https_uri)
        .body(empty_body())
        .unwrap())
}

fn empty_body() -> http_body_util::combinators::BoxBody<Bytes, hyper::Error> {
    Empty::<Bytes>::new()
        .map_err(|never: Infallible| match never {})
        .boxed()
}