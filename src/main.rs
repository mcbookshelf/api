use std::env;

use api::download::download;
use api::manifest::manifest;
use api::versions::versions;
use axum::{http::{HeaderValue, Method}, routing::get, Router};
use tower_http::cors::{Any, CorsLayer};

mod api;
mod manifest;
mod utils;


#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/versions", get(versions))
        .route("/version/{id}", get(manifest))
        .route("/download", get(download))
        .layer(create_cors_layer().await);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .unwrap_or_else(|err| {
            eprintln!("Failed to bind listener: {}", err);
            std::process::exit(1);
        });

    axum::serve(listener, app).await.unwrap();
}

async fn create_cors_layer() -> CorsLayer {
    match env::var("BS_CORS_ALLOW_LIST") {
        Ok(origins) => CorsLayer::new().allow_origin(origins
            .split(',')
            .filter_map(|origin| origin.trim().parse().ok())
            .collect::<Vec<HeaderValue>>()
        ),
        Err(_) => CorsLayer::new().allow_origin(Any),
    }.allow_methods([Method::GET])
}
