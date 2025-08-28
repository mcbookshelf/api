use anyhow::{Context, Result};
use axum::extract::Path;
use axum::http::StatusCode;
use axum::Json;
use axum::response::IntoResponse;
use cached::proc_macro::cached;
use reqwest::Client;
use tokio::time::Duration;

use crate::manifest::ManifestKind;
use crate::manifest::v2::Manifest;
use crate::utils::{read_from_json_file, write_to_json_file};
use super::versions::{fetch_versions, Version};


#[utoipa::path(
    get,
    tag = "versions",
    summary = "Get version manifest",
    description = "Get the manifest associated with a specific version.",
    path = "/version/{version}",
    params(
        ("version" = String, Path, description = "Version number to get the manifest for", example = "2.2.2"),
    ),
    responses(
        (status = 200, description = "Manifest data for the specified version", body = Manifest),
        (status = 404, description = "Manifest not found"),
    )
)]
pub async fn manifest(Path(version): Path<String>) -> impl IntoResponse {
    match fetch_manifest(version.to_string()).await {
        Ok(Some(data)) => Json(data.into_latest()).into_response(),
        Ok(None) => (StatusCode::NOT_FOUND).into_response(),
        Err(err) => {
            eprintln!("{}", err);
            (StatusCode::INTERNAL_SERVER_ERROR, "Failed to fetch manifest").into_response()
        },
    }
}

#[cached(time = 86400, result = true, sync_writes = "by_key")]
pub async fn fetch_manifest(version: String) -> Result<Option<ManifestKind>> {
    let cache_path = format!("cache/{}/manifest.json", version);
    if let Ok(manifest) = read_from_json_file(&cache_path).await {
        return Ok(Some(manifest));
    }

    let versions = fetch_versions().await.context("Failed to fetch versions")?;

    if let Some(version) = versions.into_iter().find(|entry| entry.version == version) {
        let manifest = fetch_manifest_from_github(&version).await?;
        write_to_json_file(&cache_path, &manifest).await?;
        Ok(Some(manifest))
    } else {
        Ok(None)
    }
}

async fn fetch_manifest_from_github(version: &Version) -> Result<ManifestKind> {
    let client = Client::new();
    let response = client.get(&version.manifest).send().await?.error_for_status()?;
    let manifest: ManifestKind = response.json().await?;

    Ok(manifest)
}
