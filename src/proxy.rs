use hyper::{Request, Response};
use hyper::body::Incoming;
use hyper_util::rt::TokioIo;
use tokio::net::TcpStream;
use crate::{headers, config::CONFIG};
use std::net::SocketAddr;
use crate::balancer::get_destination;
use crate::config::config;

type BoxError = Box<dyn std::error::Error + Send + Sync>;
pub async fn handle_request(
    mut inc_request: Request<Incoming>,
    client_addr: SocketAddr,
) -> Result<Response<Incoming>, BoxError> {
    let destination = get_destination().await;
    let stream = TcpStream::connect(&destination).await?;
    let io = TokioIo::new(stream);
    let addr = client_addr.clone();

    // Create hyper client
    let (mut sender, conn) = hyper::client::conn::http1::handshake::<_, Incoming>(io).await.unwrap();

    // Spawn a task to manage the connection
    tokio::task::spawn(async move {
        if let Err(err) = conn.await {
            log::error!("Connection failed: {:?}", err);
        }
    });

    // Add headers
    headers::add_forwarded_headers(inc_request.headers_mut(), addr, &destination);

    // Forward request
    let res = sender.send_request(inc_request).await?;
    Ok(res)
}