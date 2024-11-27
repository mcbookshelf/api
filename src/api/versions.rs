use std::fs::File;

use anyhow::{Context, Result};
use axum::{http::StatusCode, response::IntoResponse, Json};
use cached::proc_macro::cached;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::{
    fs::OpenOptions,
    io::{self, AsyncWriteExt},
};

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
    let file = File::open("data/versions.json").context("Failed to load versions.json")?;
    let versions: Vec<Version> =
        serde_json::from_reader(file).context("Failed to parse versions.json")?;
    Ok(versions)
}

async fn fetch_versions_from_github() -> Result<Vec<Version>> {
    let url =
        "https://raw.githubusercontent.com/mcbookshelf/Bookshelf/refs/heads/master/meta/versions.json";
    let client = Client::new();
    let response = client
        .get(url)
        .send()
        .await
        .context("Failed to send request to Github")?;
    let versions: Vec<Version> = response
        .json()
        .await
        .context("Failed to parse response from Github")?;
    if let Err(err) = write_versions_to_disk(&versions).await {
        eprintln!("Failed to overwrite versions file: {}", err);
    }
    Ok(versions)
}

async fn write_versions_to_disk(versions: &Vec<Version>) -> io::Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .truncate(true)
        .create(true)
        .open("data/versions.json")
        .await?;
    let data = serde_json::to_string(versions)?;
    file.write_all(data.as_bytes()).await
}
