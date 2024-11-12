use std::collections::HashSet;
use std::fs::File;
use std::io::Cursor;
use std::io::Read;
use std::io::Write;

use anyhow::anyhow;
use anyhow::Result;
use cached::proc_macro::cached;
use reqwest::Client;
use zip::write::SimpleFileOptions;
use zip::CompressionMethod;
use zip::ZipArchive;
use zip::ZipWriter;

use crate::manifest::v2::Module;
use crate::manifest::v2::ModuleKind;

#[cached(time = 60, result = true)]
pub async fn create_bundle(version: String, modules: Vec<Module>) -> Result<Vec<u8>> {
    let (data_packs, resource_packs): (Vec<Module>, Vec<Module>) = modules
        .into_iter()
        .partition(|module| matches!(module.kind, ModuleKind::DataPack));

    match (data_packs.is_empty(), resource_packs.is_empty()) {
        (false, true) => create_specialized_bundle(&version, data_packs).await,
        (true, false) => create_specialized_bundle(&version, resource_packs).await,
        (false, false) => {
            let mut buffer = Vec::new();
            let mut zip_writer = ZipWriter::new(Cursor::new(&mut buffer));
            let options =
                SimpleFileOptions::default().compression_method(CompressionMethod::Stored);

            zip_writer.start_file("resource_packs.zip", options)?;
            zip_writer.write_all(&create_specialized_bundle(&version, data_packs).await?)?;
            zip_writer.start_file("data_packs.zip", options)?;
            zip_writer.write_all(&create_specialized_bundle(&version, resource_packs).await?)?;
            zip_writer.finish()?;

            Ok(buffer)
        }
        _ => Err(anyhow!("Cannot create an empty bundle")),
    }
}

async fn create_specialized_bundle(version: &str, modules: Vec<Module>) -> Result<Vec<u8>> {
    let mut buffer = Vec::new();
    let mut zip_writer = ZipWriter::new(Cursor::new(&mut buffer));
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .compression_level(Some(9));

    let mut duplicates = HashSet::new();
    for module in modules {
        let mut archive = fetch_module_archive(version.to_string(), module).await?;
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
async fn fetch_module_archive(
    version: String,
    module: Module,
) -> Result<ZipArchive<Cursor<Vec<u8>>>> {
    let mut buffer = Vec::new();
    if module.download.is_empty() {
        let mut file = File::open(format!("data/{}/{}.zip", version, module.id))?;
        file.read_to_end(&mut buffer)?;
    } else {
        let client = Client::new();
        let response = client
            .get(&module.download)
            .send()
            .await?;
        if response.status().is_success() {
            let bytes = response.bytes().await?;
            buffer.extend_from_slice(&bytes);
        } else {
            return Err(anyhow!("Failed to download the module"));
        }
    }

    Ok(ZipArchive::new(Cursor::new(buffer))?)
}
