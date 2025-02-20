use anyhow::Result;
use axum::http::StatusCode;
use axum::Json;
use axum::response::IntoResponse;
use cached::proc_macro::cached;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::utils::{read_from_json_file, write_to_json_file};


#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Version {
    pub version: String,
    pub minecraft_versions: Vec<String>,
    pub manifest: String,
}

pub async fn versions() -> impl IntoResponse {
    match fetch_versions().await {
        Ok(data) => Json(data).into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Failed to fetch versions").into_response(),
    }
}

#[cached(time = 600, result = true, sync_writes = true)]
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
    let url = "https://raw.githubusercontent.com/mcbookshelf/Bookshelf/refs/heads/master/meta/versions.json";
    let client = Client::new();
    let response = client.get(url).send().await?.error_for_status()?;
    let versions: Vec<Version> = response.json().await?;

    Ok(versions)
}
