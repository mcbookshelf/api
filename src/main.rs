use api::download::download;
use api::manifest::manifest;
use api::versions::versions;
use axum::{routing::get, Router};

mod api;
mod manifest;

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/versions", get(versions))
        .route("/version/:id", get(manifest))
        .route("/download", get(download));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
