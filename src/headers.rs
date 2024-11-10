use std::net::SocketAddr;
use hyper::header::HeaderValue;
use hyper::HeaderMap;

pub fn add_forwarded_headers(headers: &mut HeaderMap, client_addr: SocketAddr) {
    headers.insert(
        "X-Forwarded-For",
        HeaderValue::from_str(&client_addr.ip().to_string()).unwrap(),
    );
    headers.insert("X-Forwarded-Proto", HeaderValue::from_static("http"));
}