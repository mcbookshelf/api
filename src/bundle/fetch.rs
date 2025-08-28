use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use cached::proc_macro::cached;
use dashmap::DashMap;
use reqwest::Client;
use serde::Deserialize;
use tokio::sync::Semaphore;
use tokio::time::timeout;

use crate::bundle::VersionedModule;
use crate::utils::{read_from_file, write_to_file};

const FETCH_MODULE_COOLDOWN: Duration = Duration::from_secs(600);

static FETCH_MODULE_LAST: OnceLock<DashMap<String, Instant>> = OnceLock::new();
static SEMAPHORE: OnceLock<Arc<Semaphore>> = OnceLock::new();


#[derive(Clone, Debug, Deserialize)]
struct ModrinthVersion {
    files: Vec<ModrinthFile>,
}

#[derive(Clone, Debug, Deserialize)]
struct ModrinthFile {
    url: String,
    primary: bool,
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
) -> Result<Vec<u8>> {
    let cache_path = format!("cache/{}/{}.zip", module.version, module.id);
    if let Ok(bytes) = read_from_file(&cache_path).await {
        let now = Instant::now();
        let map = FETCH_MODULE_LAST.get_or_init(DashMap::new);

        if map.get(&cache_path).is_none_or(|last| now.duration_since(*last.value()) > FETCH_MODULE_COOLDOWN) {
            let sem = SEMAPHORE.get_or_init(|| Arc::new(Semaphore::new(3))).clone();
            map.insert(cache_path.clone(), now);

            tokio::spawn(async move {
                if let Ok(_permit) = sem.acquire().await {
                    let url = match fetch_module_url_from_modrinth(&client, &module).await {
                        Ok(url) => url,
                        Err(_) => return,
                    };

                    if let Ok(Ok(resp)) = timeout(Duration::from_secs(5), client.get(url).send()).await {
                        let _ = resp.bytes().await;
                    }
                }
            });
        }
        Ok(bytes)
    } else {
        let bytes = fetch_module_from_sources(&client, &module).await?;
        write_to_file(&cache_path, &bytes).await?;

        Ok(bytes)
    }
}


async fn fetch_module_from_sources(
    client: &Client,
    module: &VersionedModule,
) -> Result<Vec<u8>> {
    let url = match fetch_module_url_from_modrinth(client, module).await {
        Ok(url) => url,
        Err(_) => fetch_module_url_from_github(client, module)
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
    convert = r#"{ module.to_string() }"#,
    key = "String",
)]
async fn fetch_module_url_from_modrinth(
    client: &Client,
    module: &VersionedModule,
) -> Result<String> {
    let url = format!("https://api.modrinth.com/v3/project/{}/version/{}", module.slug, module.version);
    let response = client.get(url).send().await?.error_for_status()?;
    let data = response.json::<ModrinthVersion>().await?;

    Ok(data.files
        .iter()
        .find(|file: &&ModrinthFile| file.primary)
        .context("Failed to find file")?
        .url
        .to_string()
    )
}


#[cached(
    result = true,
    sync_writes = "by_key",
    convert = r#"{ module.to_string() }"#,
    key = "String",
)]
async fn fetch_module_url_from_github(
    client: &Client,
    module: &VersionedModule,
) -> Result<String> {
    let release = fetch_module_release_from_github(client, &module.version).await?;

    Ok(release.assets
        .iter()
        .find(|asset| asset.name.starts_with(&module.id))
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
