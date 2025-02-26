use std::collections::HashSet;
use std::fmt;
use std::io::Cursor;
use std::io::Write;

use anyhow::Result;
use reqwest::Client;
use zip::write::SimpleFileOptions;
use zip::ZipArchive;
use zip::ZipWriter;

use crate::bundle::fetch::fetch_module;
use crate::manifest::v2::ModuleKind;

pub mod fetch;


#[derive(Clone, Debug)]
pub struct VersionedModule {
    id: String,
    slug: String,
    kind: ModuleKind,
    version: String,
}

impl VersionedModule {
    pub fn new(
        id: String,
        slug: String,
        kind: ModuleKind,
        version: String,
    ) -> Self {
        Self { id, slug, kind, version }
    }
}

impl fmt::Display for VersionedModule {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}@{}", self.id, self.version)
    }
}


pub async fn create_bundle(modules: Vec<VersionedModule>) -> Result<Vec<u8>> {
    let client = Client::new();
    let mut data_packs = Vec::with_capacity(modules.len());
    let mut resource_packs = Vec::with_capacity(modules.len());

    for module in modules {
        match module.kind {
            ModuleKind::DataPack => data_packs.push(module),
            ModuleKind::ResourcePack => resource_packs.push(module),
            ModuleKind::Combined => {
                data_packs.push(module.clone());
                resource_packs.push(module);
            }
        }
    }

    if !data_packs.is_empty() && !resource_packs.is_empty() {
        create_packs(&client, data_packs, resource_packs).await
    } else if !data_packs.is_empty() {
        create_pack(&client, data_packs, ModuleKind::DataPack).await
    } else {
        create_pack(&client, resource_packs, ModuleKind::ResourcePack).await
    }
}


async fn create_packs(
    client: &Client,
    data_packs: Vec<VersionedModule>,
    resource_packs: Vec<VersionedModule>,
) -> Result<Vec<u8>> {
    let mut buffer = Vec::new();
    let mut zip_writer = ZipWriter::new(Cursor::new(&mut buffer));
    let options = SimpleFileOptions::default();
    zip_writer.start_file("data_packs.zip", options)?;
    zip_writer.write_all(&create_pack(client, data_packs, ModuleKind::DataPack).await?)?;
    zip_writer.start_file("resource_packs.zip", options)?;
    zip_writer.write_all(&create_pack(client, resource_packs, ModuleKind::ResourcePack).await?)?;
    zip_writer.finish()?;

    Ok(buffer)
}


async fn create_pack(
    client: &Client,
    modules: Vec<VersionedModule>,
    kind: ModuleKind,
) -> Result<Vec<u8>> {
    let mut buffer = Vec::new();
    let mut duplicates = HashSet::new();
    let mut writer = ZipWriter::new(Cursor::new(&mut buffer));
    let options = SimpleFileOptions::default();

    for module in modules {
        let data = fetch_module(client.clone(), module, kind).await?;
        let mut archive = ZipArchive::new(Cursor::new(data))?;

        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;

            if duplicates.insert(file.name().to_string()) {
                writer.start_file(file.name(), options)?;
                std::io::copy(&mut file, &mut writer)?;
            }
        }
    }

    writer.finish()?;
    Ok(buffer)
}
