use std::path::Path;

use anyhow::{Context, Result};
use tokio::fs::{create_dir_all, File, OpenOptions};
use tokio::io::{AsyncReadExt, AsyncWriteExt};


pub async fn read_from_file(path: &str) -> Result<Vec<u8>> {
    let mut file = File::open(path).await.context("Failed to open file")?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer).await.context("Failed to read file")?;
    Ok(buffer)
}

pub async fn read_from_json_file<T>(path: &str) -> Result<T>
where
    T: serde::de::DeserializeOwned,
{
    let buffer = read_from_file(path).await?;
    serde_json::from_slice(&buffer).context("Failed to deserialize JSON")
}

pub async fn write_to_file(path: &str, bytes: &[u8]) -> Result<()> {
    if let Some(dir) = Path::new(path).parent() {
        create_dir_all(dir).await.context("Failed to create parent directory")?;
    }

    let mut file = OpenOptions::new()
        .write(true)
        .truncate(true)
        .create(true)
        .open(path)
        .await
        .context("Failed to open file for writing")?;

    file.write_all(bytes).await.context("Failed to write data to file")
}

pub async fn write_to_json_file<T>(path: &str, data: &T) -> Result<()>
where
    T: serde::Serialize,
{
    let data = serde_json::to_string(data).context("Failed to serialize data")?;
    write_to_file(path, data.as_bytes()).await
}

pub async fn write_to_new_file(path: &str, bytes: Vec<u8>) -> Result<Vec<u8>> {
    if !Path::new(path).exists() {
        write_to_file(path, &bytes).await?;
    }
    Ok(bytes)
}
