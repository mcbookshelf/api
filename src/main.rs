use std::env;

use api::download::download;
use api::manifest::manifest;
use api::versions::versions;
use axum::{http::{HeaderValue, Method}, routing::get, Router};
use tower_http::compression::CompressionLayer;
use tower_http::cors::{Any, CorsLayer};
use utoipa::OpenApi;
use utoipa_rapidoc::RapiDoc;

mod api;
mod bundle;
mod manifest;
mod utils;

#[derive(OpenApi)]
#[openapi(
     info(
        title = "Bookshelf API",
        description = "Public API to retrieve Bookshelf modules, versions, and manifests.",
    ),
    paths(
        crate::api::download::download,
        crate::api::versions::versions,
        crate::api::manifest::manifest
    ),
    tags(
        (name = "modules", description = "Download and manage modules."),
        (name = "versions", description = "Get available versions and their manifests."),
    )
)]
pub struct ApiDoc;

const TEMPLATE: &str = r##"
<!doctype html> <!-- Important: must specify -->
<html>
  <head>
    <meta charset="utf-8"> <!-- Important: rapi-doc uses utf8 characters -->
    <script type="module" src="https://unpkg.com/rapidoc/dist/rapidoc-min.js"></script>
  </head>
  <body>
    <rapi-doc
        spec-url = "$specUrl"
        render-style = "focused"
        show-header = false
        show-method-in-nav-barv="as-colored-text"
        allow-authentication = false
        allow-server-selection = false
        bg-color="#171D24"
        nav-bg-color="#222832"
        primary-color="#3578c8"
        font-size="large"
    ></rapi-doc>
  </body>
</html>
"##;


#[tokio::main]
async fn main() {
    let app = Router::new()
        .merge(RapiDoc::with_openapi("/openapi", ApiDoc::openapi()).custom_html(TEMPLATE).path("/"))
        .route("/versions", get(versions))
        .route("/version/{id}", get(manifest))
        .route("/download", get(download))
        .layer(create_cors_layer().await)
        .layer(CompressionLayer::new());

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
