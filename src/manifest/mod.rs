use serde::{Deserialize, Serialize};

pub mod v1;
pub mod v2;


#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum ManifestKind {
    V1(v1::Manifest),
    V2(v2::Manifest),
}


impl ManifestKind {
    /// Converts the ManifestKind enum into the latest version (V2).
    /// This is useful for ensuring compatibility with V2-based logic.
    pub fn into_latest(self) -> v2::Manifest {
        match self {
            ManifestKind::V1(manifest) => manifest.into(),
            ManifestKind::V2(manifest) => manifest,
        }
    }
}

impl From<v1::Manifest> for v2::Manifest {
    fn from(manifest: v1::Manifest) -> Self {
        v2::Manifest {
            modules: manifest.datapacks.into_iter().flat_map(move |datapack| {
                datapack.modules.into_iter().map(move |module| v2::Module {
                    id: module.id.to_owned(),
                    name: module.name,
                    slug: module.id.replace("bs.", "bookshelf-"),
                    icon: None,
                    banner: None,
                    readme: None,
                    documentation: module.documentation.replace(
                        "https://bookshelf.docs.gunivers.net/",
                        "https://docs.mcbookshelf.dev/",
                    ),
                    description: module.description,
                    kind: v2::ModuleKind::default(),
                    tags: match datapack.name.as_str() {
                        "Bookshelf" => vec!["lib".to_string()],
                        "Bookshelf Dev" => vec!["dev".to_string()],
                        _ => vec![],
                    },
                    dependencies: module.dependencies,
                    weak_dependencies: module.weak_dependencies,
                })
            }).collect(),
        }
    }
}
