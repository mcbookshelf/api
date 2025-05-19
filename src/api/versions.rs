use anyhow::Result;
use axum::http::StatusCode;
use axum::Json;
use axum::response::IntoResponse;
use cached::proc_macro::cached;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::utils::{read_from_json_file, write_to_json_file};


#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct Version {
    pub version: String,
    pub minecraft_versions: Vec<String>,
    pub manifest: String,
}

#[utoipa::path(
    get,
    tag = "versions",
    summary = "List all versions",
    description = "Get a list of all available module versions.",
    path = "/versions",
    responses(
        (status = 200, description = "List of available versions", body = [Version]),
    )
)]
pub async fn versions() -> impl IntoResponse {
    match fetch_versions().await {
        Ok(data) => Json(data).into_response(),
        Err(err) => {
            eprintln!("{}", err);
            (StatusCode::INTERNAL_SERVER_ERROR, "Failed to fetch versions").into_response()
        }
    }
}

#[cached(time = 600, result = true, sync_writes = "by_key")]
pub async fn fetch_versions() -> Result<Vec<Version>> {
    let cache_path = "cache/versions.json";
    match fetch_versions_from_github().await {
        Ok(versions) => {
            write_to_json_file(cache_path, &versions).await?;
            Ok(versions)
        },
        Err(_) => read_from_json_file(cache_path).await,
    }
}

async fn fetch_versions_from_github() -> Result<Vec<Version>> {
    let urls = vec![
        "https://raw.githubusercontent.com/mcbookshelf/bookshelf/refs/heads/master/data/versions.json",
        "https://raw.githubusercontent.com/mcbookshelf/bookshelf/refs/heads/master/meta/versions.json",
    ];

    let client = Client::new();

    for url in urls {
        let response = client.get(url).send().await;

        match response {
            Ok(response) if response.status().is_success() => {
                let versions: Vec<Version> = response.json().await?;
                return Ok(versions);
            }
            Ok(response) => {
                eprintln!("Failed to fetch from {}: HTTP {}", url, response.status());
            }
            Err(err) => {
                eprintln!("Error fetching from {}: {}", url, err);
            }
        }
    }

    Err(anyhow::anyhow!("All URLs failed to fetch the versions"))
}
