use std::io;

use anyhow::Context;
use anyhow::Result;
use axum::{http::StatusCode, response::IntoResponse, Json};
use cached::proc_macro::cached;
use reqwest::Client;
use serde::Deserialize;
use serde::Serialize;
use tokio::fs::File;
use tokio::fs::OpenOptions;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;


#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Version {
    pub version: String,
    pub minecraft_versions: Vec<String>,
    pub manifest: String,
}

pub async fn versions() -> impl IntoResponse {
    match fetch_versions().await {
        Ok(data) => Json(data).into_response(),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
    }
}

#[cached(time = 600, result = true)]
pub async fn fetch_versions() -> Result<Vec<Version>> {
    match fetch_versions_from_github().await {
        Ok(versions) => Ok(versions),
        Err(_) => fetch_versions_from_disk().await,
    }
}

async fn fetch_versions_from_disk() -> Result<Vec<Version>> {
    let mut file = File::open("data/versions.json").await
        .context("Failed to load versions.json")?;

    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer).await
        .context("Failed to read versions.json")?;
    let versions: Vec<Version> = serde_json::from_slice(&buffer)
        .context("Failed to parse versions.json")?;

    Ok(versions)
}

async fn fetch_versions_from_github() -> Result<Vec<Version>> {
    let url = "https://raw.githubusercontent.com/mcbookshelf/Bookshelf/refs/heads/master/meta/versions.json";
    let client = Client::new();
    let response = client.get(url).send().await?;
    let versions: Vec<Version> = response.json().await?;

    if let Err(err) = write_versions_to_disk(&versions).await {
        eprintln!("Failed to overwrite versions file: {}", err);
    }

    Ok(versions)
}

async fn write_versions_to_disk(versions: &Vec<Version>) -> io::Result<()> {
    let data = serde_json::to_string(versions)?;
    let mut file = OpenOptions::new()
        .write(true)
        .truncate(true)
        .create(true)
        .open("data/versions.json")
        .await?;

    file.write_all(data.as_bytes()).await
}
