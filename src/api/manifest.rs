use std::io;

use anyhow::{Context, Result};
use axum::{extract::Path, http::StatusCode, response::IntoResponse, Json};
use cached::proc_macro::cached;
use reqwest::Client;
use tokio::{fs::{File, OpenOptions}, io::{AsyncReadExt, AsyncWriteExt}};

use super::versions::{fetch_versions, Version};
use crate::manifest::ManifestKind;


pub async fn manifest(Path(id): Path<String>) -> impl IntoResponse {
    match fetch_manifest(id).await {
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
        Ok(data) => match data {
            Some(data) => Json(data.into_latest()).into_response(),
            None => (StatusCode::NOT_FOUND).into_response(),
        },
    }
}

#[cached(time = 600, result = true)]
pub async fn fetch_manifest(version: String) -> Result<Option<ManifestKind>> {
    let versions = fetch_versions().await.context("Failed to fetch versions")?;
    match versions.into_iter().find(|entry| entry.version == version) {
        None => Ok(None),
        Some(version) => {
            if let Ok(manifest) = fetch_manifest_from_github(&version).await {
                Ok(Some(manifest))
            } else {
                fetch_manifest_from_disk(&version).await.map(Some)
            }

        }
    }
}

async fn fetch_manifest_from_disk(version: &Version) -> Result<ManifestKind> {
    let mut file = File::open(format!("data/{}/manifest.json", version.version)).await
        .context("Failed to load manifest.json")?;

    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer).await
        .context("Failed to read manifest.json")?;
    let manifest: ManifestKind = serde_json::from_slice(&buffer)
        .context("Failed to parse manifest.json")?;

    Ok(manifest)
}

async fn fetch_manifest_from_github(version: &Version) -> Result<ManifestKind> {
    let client = Client::new();
    let response = client.get(&version.manifest).send().await
        .context("Failed to send request to Github")?;
    
    let manifest: ManifestKind = response.json().await
        .context("Failed to parse response from Github")?;

    write_manifest_to_disk(&manifest, version).await?;

    Ok(manifest)
}

async fn write_manifest_to_disk(manifest: &ManifestKind, version: &Version) -> io::Result<()> {
    let data = serde_json::to_string(manifest)?;
    let mut file = OpenOptions::new()
        .write(true)
        .truncate(true)
        .create(true)
        .open(format!("data/{}/manifest.json", version.version))
        .await?;

    file.write_all(data.as_bytes()).await
}
