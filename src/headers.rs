use std::net::SocketAddr;
use hyper::header::HeaderValue;
use hyper::HeaderMap;
use crate::config::config;

pub fn add_forwarded_headers(headers: &mut HeaderMap, client_addr: SocketAddr) {
    if let Some(existing) = headers.get("X-Forwarded-For") {
        let mut new_value = existing.to_str().unwrap().to_string();
        new_value.push_str(&format!(", {}", client_addr.ip().to_string()));
        headers.insert("X-Forwarded-For", HeaderValue::from_str(&new_value).unwrap());
    } else {
        headers.insert(
            "X-Forwarded-For",
            HeaderValue::from_str(&client_addr.ip().to_string()).unwrap(),
        );
    }

    headers.insert("X-Forwarded-Proto", HeaderValue::from_static("https"));
    // Add X-Forwarded-Host
    headers.insert(
        "X-Forwarded-Host",
        HeaderValue::from_str(&headers["Host"].to_str().unwrap()).unwrap(),
    );

    // Add X-Forwarded-Port
    headers.insert(
        "X-Forwarded-Port",
        HeaderValue::from_str(&client_addr.port().to_string()).unwrap(),
    );

    // modify host header
    if headers.contains_key("Host") {
        let new_value = &format!("{}", config().proxy.destination);
        headers.insert("Host", HeaderValue::from_str(&new_value).unwrap());
    }

    // Add X-Real-IP
    headers.insert(
        "X-Real-IP",
        HeaderValue::from_str(&client_addr.ip().to_string()).unwrap(),
    );

    // Optionally, add X-Forwarded-By if known
    headers.insert(
        "X-Forwarded-By",
        HeaderValue::from_str("proxy-server-01").unwrap(),
    );

    // Add X-Forwarded-Scheme (for extra clarity on protocol)
    headers.insert(
        "X-Forwarded-Scheme",
        HeaderValue::from_static("https"),
    );
}