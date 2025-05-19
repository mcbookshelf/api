use std::sync::OnceLock;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use cached::proc_macro::cached;
use dashmap::DashMap;
use reqwest::Client;
use serde::Deserialize;

use crate::bundle::VersionedModule;
use crate::manifest::v2::ModuleKind;
use crate::utils::{read_from_file, write_to_file};


const FETCH_MODULE_COOLDOWN: Duration = Duration::from_secs(600);
static FETCH_MODULE_LAST: OnceLock<DashMap<String, Instant>> = OnceLock::new();


#[derive(Clone, Debug, Deserialize)]
struct ModrinthVersion {
    files: Vec<ModrinthFile>,
}

#[derive(Clone, Debug, Deserialize)]
struct ModrinthFile {
    url: String,
    primary: bool,
    file_type: String,
}

#[derive(Clone, Debug, Deserialize)]
struct GithubRelease {
    assets: Vec<GithubAsset>,
}

#[derive(Clone, Debug, Deserialize)]
struct GithubAsset {
    name: String,
    #[serde(rename = "browser_download_url")]
    url: String,
}


pub async fn fetch_module(
    client: Client,
    module: VersionedModule,
    kind: ModuleKind,
) -> Result<Vec<u8>> {
    let cache_path = format!("cache/{}/{}.zip", module.version, module.id);
    if let Ok(bytes) = read_from_file(&cache_path).await {
        let now = Instant::now();
        if FETCH_MODULE_LAST.get_or_init(DashMap::new).get(&cache_path).is_none_or(
            |last| now.duration_since(*last.value()) > FETCH_MODULE_COOLDOWN
        ) {
            tokio::spawn(async move {
                if let Ok(url) = fetch_module_url_from_modrinth(&client, &module, &kind).await {
                    let _ = client.get(url).send().await;
                    FETCH_MODULE_LAST.get_or_init(DashMap::new).insert(cache_path, now);
                }
            });
        }
        Ok(bytes)
    } else {
        let bytes = fetch_module_from_sources(&client, &module, &kind).await?;
        write_to_file(&cache_path, &bytes).await?;

        Ok(bytes)
    }
}


async fn fetch_module_from_sources(
    client: &Client,
    module: &VersionedModule,
    kind: &ModuleKind,
) -> Result<Vec<u8>> {
    let url = match fetch_module_url_from_modrinth(client, module, kind).await {
        Ok(url) => url,
        Err(_) => fetch_module_url_from_github(client, module, kind)
            .await
            .context("Failed to fetch module from sources")?,
    };

    let response = client.get(url).send().await?.error_for_status()?;
    let bytes = response.bytes().await?;

    Ok(bytes.to_vec())
}


#[cached(
    result = true,
    sync_writes = "by_key",
    convert = r#"{ format!("{}:{}", module, kind) }"#,
    key = "String",
)]
async fn fetch_module_url_from_modrinth(
    client: &Client,
    module: &VersionedModule,
    kind: &ModuleKind,
) -> Result<String> {
    let url = format!("https://api.modrinth.com/v3/project/{}/version/{}", module.slug, module.version);
    let response = client.get(url).send().await?.error_for_status()?;
    let data = response.json::<ModrinthVersion>().await?;

    let predicate = if module.kind == ModuleKind::Combined && *kind == ModuleKind::ResourcePack {
        |file: &&ModrinthFile| file.file_type.ends_with("resource-pack")
    } else {
        |file: &&ModrinthFile| file.primary
    };

    Ok(data.files
        .iter()
        .find(predicate)
        .context("Failed to find file")?
        .url
        .to_string()
    )
}


#[cached(
    result = true,
    sync_writes = "by_key",
    convert = r#"{ format!("{}:{}", module, kind) }"#,
    key = "String",
)]
async fn fetch_module_url_from_github(
    client: &Client,
    module: &VersionedModule,
    kind: &ModuleKind,
) -> Result<String> {
    let mut prefix = format!("{}-", module.id);
    let release = fetch_module_release_from_github(client, &module.version).await?;
    if module.kind == ModuleKind::Combined && *kind == ModuleKind::ResourcePack {
        prefix.push_str("rp");
    }

    Ok(release.assets
        .iter()
        .find(|asset| asset.name.starts_with(&prefix))
        .context("Failed to find asset")?
        .url.to_string()
    )
}


#[cached(
    result = true,
    sync_writes = "by_key",
    convert = r#"{ version.to_string() }"#,
    key = "String",
)]
async fn fetch_module_release_from_github(
    client: &Client,
    version: &str,
) -> Result<GithubRelease> {
    let url = format!("https://api.github.com/repos/mcbookshelf/Bookshelf/releases/tags/v{}", version);
    let response = client.get(url).header("User-Agent", "Bookshelf-API").send().await?.error_for_status()?;

    Ok(response.json().await?)
}
