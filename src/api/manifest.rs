use anyhow::{Context, Result};
use axum::{extract::Path, http::StatusCode, response::IntoResponse, Json};
use cached::proc_macro::cached;
use reqwest::Client;

use super::versions::fetch_versions;
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
pub async fn fetch_manifest(id: String) -> Result<Option<ManifestKind>> {
    let versions = fetch_versions().await.context("Failed to fetch versions")?;
    match versions.into_iter().find(|version| version.version == id) {
        None => Ok(None),
        Some(version) => {
            let client = Client::new();
            let response = client
                .get(&version.manifest)
                .send()
                .await
                .context("Failed to send request to Github")?;
            let manifest: ManifestKind = response
                .json()
                .await
                .context("Failed to parse response from Github")?;
            Ok(Some(manifest))
        }
    }
}
