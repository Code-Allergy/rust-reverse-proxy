use axum::{
    body::Body,
    http::{Request, StatusCode},
    response::{IntoResponse, Response},
    Router,
};
use std::net::SocketAddr;
use axum::routing::get;
use tower::ServiceBuilder;
use std::env;

async fn handle_request(req: Request<Body>, port:u16) -> impl IntoResponse {
    println!("Received request: {:?}", req);
    // Return a "Hello, World!" response
    (StatusCode::OK, format!("Hello, World! (rust on port {})", port)).into_response()
}

async fn handle_hello(req: Request<Body>, port:u16) -> impl IntoResponse {
    (StatusCode::OK, format!("Hello, World API! (rust on port {})", port)).into_response()
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();
    let port = args.get(1).unwrap_or(&"3000".to_string()).parse::<u16>().unwrap();
    // Build the Axum app
    let app = Router::new()
        .route("/", get(move |req| handle_request(req, port.clone())))
        .route("/api/hello", get(move |req| handle_hello(req, port.clone())))
        .layer(ServiceBuilder::new());

    // Define the address to bind the server
    let addr = SocketAddr::from(([127, 0, 0, 1], port));

    // Start the server
    println!("Server is running on http://localhost:{}", port);
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
        axum::serve(listener, app)
        .await
        .unwrap();
}