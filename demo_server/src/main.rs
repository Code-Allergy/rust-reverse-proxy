use axum::{
    body::Body,
    http::{Request, StatusCode},
    response::{IntoResponse, Response},
    Router,
};
use std::net::SocketAddr;
use axum::routing::get;
use tower::ServiceBuilder;

async fn handle_request(req: Request<Body>) -> impl IntoResponse {
    // Log all request headers
    // println!("Request Headers: {:?}", req.headers());

    // Return a "Hello, World!" response
    (StatusCode::OK, "Hello, World! (rust)").into_response()
}

#[tokio::main]
async fn main() {
    // Build the Axum app
    let app = Router::new()
        .route("/", get(handle_request))
        .layer(ServiceBuilder::new());

    // Define the address to bind the server
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));

    // Start the server
    println!("Server is running on http://localhost:3000");
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
        axum::serve(listener, app)
        .await
        .unwrap();
}