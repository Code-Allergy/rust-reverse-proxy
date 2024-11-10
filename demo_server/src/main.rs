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

async fn handle_request(req: Request<Body>) -> impl IntoResponse {
    println!("Received request: {:?}", req);
    // Return a "Hello, World!" response
    (StatusCode::OK, "Hello, World! (rust)").into_response()
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();
    let port = args.get(1).unwrap_or(&"3000".to_string()).parse::<u16>().unwrap();
    // Build the Axum app
    let app = Router::new()
        .route("/", get(handle_request))
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