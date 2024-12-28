use std::collections::HashSet;
use std::io;
use std::io::Cursor;
use std::io::Write;
use std::path::Path;

use anyhow::anyhow;
use anyhow::Context;
use anyhow::Result;
use axum::body::Bytes;
use axum::extract::Query;
use axum::http::{header, StatusCode};
use axum::response::IntoResponse;
use serde::Deserialize;
use cached::proc_macro::cached;
use reqwest::Client;
use tokio::fs::File;
use tokio::fs::OpenOptions;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use zip::write::SimpleFileOptions;
use zip::CompressionMethod;
use zip::ZipArchive;
use zip::ZipWriter;

use crate::manifest::v2::Module;
use crate::manifest::v2::ModuleKind;

use super::manifest::fetch_manifest;


type VersionedModules = Vec<(Module, String)>;


#[derive(Deserialize)]
pub struct QueryParams {
    version: String,
    modules: String,
}

pub async fn download(Query(params): Query<QueryParams>) -> impl IntoResponse {
    let manifest = match fetch_manifest(params.version.clone()).await {
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR).into_response(),
        Ok(None) => return (StatusCode::UNPROCESSABLE_ENTITY).into_response(),
        Ok(Some(manifest)) => manifest,
    };

    let mut modules = Vec::new();
    let ids: Vec<&str> = params.modules.split(',').collect();

    for module in manifest.into_latest().modules {
        if let Some(id) = ids.iter().find(|&id| id.starts_with(&module.id)) {
            modules.push(
                if let Some(version) = id.split(':').nth(1) {
                    (module, version.to_string())
                } else {
                    (module, params.version.to_string())
                }
            );
        }
    }

    if modules.is_empty() {
        return (StatusCode::UNPROCESSABLE_ENTITY).into_response();
    }

    match create_bundle(modules).await {
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR).into_response(),
        Ok(data) => {
            let headers = [
                (header::CONTENT_TYPE, "application/zip"),
                (
                    header::CONTENT_DISPOSITION,
                    &format!("attachment; filename=\"bookshelf-{}.zip\"", params.version),
                ),
            ];
            (StatusCode::OK, headers, Bytes::from(data)).into_response()
        }
    }
}

#[cached(time = 60, result = true)]
async fn create_bundle(modules: VersionedModules) -> Result<Vec<u8>> {
    if modules.is_empty() {
        return Err(anyhow!("Cannot create an empty bundle"));
    } 

    let (data_packs, resource_packs): (VersionedModules, VersionedModules) = modules
        .clone()
        .into_iter()
        .partition(|(module, _)| matches!(module.kind, ModuleKind::DataPack));

    if data_packs.is_empty() || resource_packs.is_empty() {
        create_specialized_bundle(modules).await
    } else {
        let mut buffer = Vec::new();
        let mut zip_writer = ZipWriter::new(Cursor::new(&mut buffer));
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);

        zip_writer.start_file("resource_packs.zip", options)?;
        zip_writer.write_all(&create_specialized_bundle(data_packs).await?)?;
        zip_writer.start_file("data_packs.zip", options)?;
        zip_writer.write_all(&create_specialized_bundle(resource_packs).await?)?;
        zip_writer.finish()?;

        Ok(buffer)
    }
}

async fn create_specialized_bundle(modules: VersionedModules) -> Result<Vec<u8>> {
    let mut buffer = Vec::new();
    let mut zip_writer = ZipWriter::new(Cursor::new(&mut buffer));
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .compression_level(Some(9));

    let mut duplicates = HashSet::new();
    for module in modules {
        let mut archive = fetch_module(module.0, module.1).await?;

        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            let name = file.name().to_string();

            if !duplicates.contains(&name) {
                zip_writer.start_file(&name, options)?;
                std::io::copy(&mut file, &mut zip_writer)?;
                duplicates.insert(name);
            }
        }
    }

    zip_writer.finish()?;
    Ok(buffer)
}

#[cached(time = 60, result = true)]
async fn fetch_module(module: Module, version: String) -> Result<ZipArchive<Cursor<Vec<u8>>>> {
    if let Ok(bytes) = fetch_module_from_modrinth(&module, &version).await {
        Ok(ZipArchive::new(Cursor::new(bytes))?)
    } else if let Ok(bytes) = fetch_module_from_disk(&module, &version).await {
        Ok(ZipArchive::new(Cursor::new(bytes))?)
    } else {
        Err(anyhow!("Failed to fetch module from all sources"))
    }
}

async fn fetch_module_from_disk(module: &Module, version: &str) -> Result<Vec<u8>> {
    let mut buffer = Vec::new();
    let path = format!("data/{}/{}.zip", version, module.id);
    File::open(path).await?.read_to_end(&mut buffer).await?;
    Ok(buffer)
}

async fn fetch_module_from_modrinth(module: &Module, version: &str) -> Result<Vec<u8>> {
    let client = Client::new();
    let url = format!("https://api.modrinth.com/v3/project/{}/version/{}", module.slug, version);

    let response = client.get(url).send().await?;
    let data: serde_json::Value = response.json().await?;
    let url = data["files"][0]["url"].as_str().context("Failed to find file URL in response")?;

    let response = client.get(url).send().await?;
    if !response.status().is_success() {
        return Err(anyhow!("Failed to download the module from Modrinth: {}", response.status()));
    }

    let bytes = response.bytes().await?.to_vec();
    write_module_to_disk(module, version, &bytes).await?;

    Ok(bytes)
}

async fn write_module_to_disk(module: &Module, version: &str, bytes: &[u8]) -> io::Result<()> {
    let path = format!("data/{}/{}.zip", version, module.id);
    if !Path::new(&path).exists() {
        return Ok(())
    }

    let mut file = OpenOptions::new()
        .write(true)
        .truncate(true)
        .create(true)
        .open(path)
        .await?;

    file.write_all(bytes).await
}
