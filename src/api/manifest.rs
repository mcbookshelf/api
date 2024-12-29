use std::path::Path as StdPath;

use anyhow::{Context, Result};
use axum::extract::Path;
use axum::http::StatusCode;
use axum::Json;
use axum::response::IntoResponse;
use cached::proc_macro::cached;
use reqwest::Client;

use crate::manifest::ManifestKind;
use crate::utils::{read_from_json_file, write_to_json_file};
use super::versions::{fetch_versions, Version};


pub async fn manifest(Path(version): Path<String>) -> impl IntoResponse {
    match fetch_manifest(version).await {
        Ok(Some(data)) => Json(data.into_latest()).into_response(),
        Ok(None) => (StatusCode::NOT_FOUND).into_response(),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
    }
}

#[cached(time = 86400, result = true)]
pub async fn fetch_manifest(version: String) -> Result<Option<ManifestKind>> {
    let disk_path = format!("data/{}/manifest.json", version);
    if StdPath::new(&disk_path).exists() {
        return read_from_json_file(&disk_path).await.map(Some);
    }

    let versions = fetch_versions().await.context("Failed to fetch versions")?;

    if let Some(version) = versions.into_iter().find(|entry| entry.version == version) {
        let manifest = fetch_manifest_from_github(&version).await?;
        write_to_json_file(&disk_path, &manifest).await?;
        Ok(Some(manifest))
    } else {
        Ok(None)
    }
}

async fn fetch_manifest_from_github(version: &Version) -> Result<ManifestKind> {
    let client = Client::new();
    let response = client.get(&version.manifest).send().await?;
    let manifest: ManifestKind = response.json().await?;

    Ok(manifest)
}
