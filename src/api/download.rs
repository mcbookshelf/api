use std::collections::HashSet;
use std::io::{Cursor, Write};
use std::ops::Deref;

use anyhow::{Context, Result};
use axum::body::Bytes;
use axum::extract::Query;
use axum::http::{header, StatusCode};
use axum::response::IntoResponse;
use cached::proc_macro::cached;
use reqwest::Client;
use serde::Deserialize;
use zip::CompressionMethod;
use zip::write::SimpleFileOptions;
use zip::ZipArchive;
use zip::ZipWriter;

use crate::manifest::ManifestKind;
use crate::manifest::v2::{Module, ModuleKind};
use crate::utils::{read_from_file, write_to_new_file};
use super::manifest::fetch_manifest;


#[derive(Deserialize)]
pub struct QueryParams {
    version: String,
    modules: String,
}

#[derive(Deserialize, Clone, Debug, Hash, Eq, PartialEq)]
struct GithubRelease {
    assets: Vec<GithubAsset>,
}

#[derive(Deserialize, Clone, Debug, Hash, Eq, PartialEq)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
}

#[derive(Clone, Debug, Hash, Eq, PartialEq)]
struct VersionedModule {
    module: Module,
    version: String,
}

impl Deref for VersionedModule {
    type Target = Module;

    fn deref(&self) -> &Self::Target {
        &self.module
    }
}


pub async fn download(Query(params): Query<QueryParams>) -> impl IntoResponse {
    let manifest = match fetch_manifest(params.version.to_owned()).await {
        Ok(Some(manifest)) => manifest,
        Ok(None) => return (StatusCode::UNPROCESSABLE_ENTITY).into_response(),
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR).into_response(),
    };

    let modules = get_versioned_modules(manifest, params).await;
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
                    "attachment; filename=\"bookshelf.zip\"",
                ),
            ];
            (StatusCode::OK, headers, Bytes::from(data)).into_response()
        }
    }
}

#[cached(size = 10, time = 60, result = true)]
async fn create_bundle(modules: Vec<VersionedModule>) -> Result<Vec<u8>> {
    let (datapacks, resourcepacks): (Vec<VersionedModule>, Vec<VersionedModule>) = modules
        .into_iter()
        .partition(|module| matches!(module.kind, ModuleKind::DataPack));

    if datapacks.is_empty() || resourcepacks.is_empty() {
        create_pack([datapacks, resourcepacks].concat()).await
    } else {
        create_packs(datapacks, resourcepacks).await
    }
}

async fn create_packs(
    datapacks: Vec<VersionedModule>,
    resourcepacks: Vec<VersionedModule>,
) -> Result<Vec<u8>> {
    let mut buffer = Vec::new();
    let mut zip_writer = ZipWriter::new(Cursor::new(&mut buffer));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);

    zip_writer.start_file("resource_packs.zip", options)?;
    zip_writer.write_all(&create_pack(datapacks).await?)?;
    zip_writer.start_file("data_packs.zip", options)?;
    zip_writer.write_all(&create_pack(resourcepacks).await?)?;
    zip_writer.finish()?;

    Ok(buffer)
}

async fn create_pack(modules: Vec<VersionedModule>) -> Result<Vec<u8>> {
    let mut buffer = Vec::new();
    let mut writer = ZipWriter::new(Cursor::new(&mut buffer));

    let mut duplicates = HashSet::new();
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .compression_level(Some(9));

    for module in modules {
        let data = fetch_module(module.module, module.version).await?;
        let mut archive = ZipArchive::new(Cursor::new(data))?;

        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            let name = file.name().to_string();

            if duplicates.insert(name.clone()) {
                writer.start_file(&name, options)?;
                std::io::copy(&mut file, &mut writer)?;
            }
        }
    }

    writer.finish()?;
    Ok(buffer)
}

#[cached(size = 50, time = 60, result = true)]
async fn fetch_module(
    module: Module,
    version: String,
) -> Result<Vec<u8>> {
    let disk_path = format!("data/{}/{}.zip", version, module.id);

    if let Ok(bytes) = fetch_module_from_modrinth(&module, &version).await {
        return write_to_new_file(&disk_path, bytes).await;
    }

    if let Ok(bytes) = read_from_file(&disk_path).await {
        return Ok(bytes);
    }

    write_to_new_file(
        &disk_path,
        fetch_module_from_github(&module, &version)
            .await
            .context(format!("Failed to fetch module: {}", module.id))?
    ).await
}

async fn fetch_module_from_modrinth(
    module: &Module,
    version: &str,
) -> Result<Vec<u8>> {
    let client = Client::new();
    let url = format!("https://api.modrinth.com/v3/project/{}/version/{}", module.slug, version);

    let response = client.get(url).send().await?;
    let data: serde_json::Value = response.json().await?;
    let url = data["files"][0]["url"].as_str().context("Failed to find file URL in response")?;

    let response = client.get(url).send().await?;
    Ok(response.bytes().await?.to_vec())
}

async fn fetch_module_from_github(
    module: &Module,
    version: &str,
) -> Result<Vec<u8>> {
    let release = fetch_release_from_github(version.to_owned()).await?;
    let asset = release.assets
        .iter()
        .find(|asset| asset.name.starts_with(&module.id))
        .context("Failed to find module asset")?;

    let client = Client::new();
    let response = client.get(&asset.browser_download_url).send().await?;
    Ok(response.bytes().await?.to_vec())
}

#[cached(result = true)]
async fn fetch_release_from_github(
    version: String,
) -> Result<GithubRelease> {
    let client = Client::new();
    let url = format!("https://api.github.com/repos/mcbookshelf/Bookshelf/releases/tags/v{}", version);

    let response = client.get(url).header("User-Agent", "Bookshelf-API").send().await?;
    Ok(response.json().await?)
}

async fn get_versioned_modules(
    manifest: ManifestKind,
    params: QueryParams,
) -> Vec<VersionedModule> {
    let ids: Vec<&str> = params.modules.split(',').collect();

    manifest.into_latest().modules.into_iter().filter_map(|module| {
        ids.iter().find(|&id| id.starts_with(&module.id)).map(|id| {
            let version = id.split(':').nth(1).unwrap_or(&params.version).to_owned();
            VersionedModule {module, version}
        })
    }).collect()
}
