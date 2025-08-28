use std::fmt;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;


#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct Manifest {
    pub modules: Vec<Module>,
}

#[derive(Clone, Debug, Deserialize, Serialize, Hash, Eq, PartialEq, ToSchema)]
pub struct Module {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub icon: Option<String>,
    pub banner: Option<String>,
    pub readme: Option<String>,
    pub documentation: String,
    pub description: String,
    #[serde(default)]
    pub kind: ModuleKind,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub dependencies: Vec<String>,
    #[serde(default)]
    pub weak_dependencies: Vec<String>,
}

#[derive(Copy, Clone, Debug, Default, Deserialize, Serialize, Hash, Eq, PartialEq, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ModuleKind {
    #[default]
    DataPack,
    ResourcePack,
}

impl fmt::Display for ModuleKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", match self {
            ModuleKind::DataPack => "dp",
            ModuleKind::ResourcePack => "rp"
        })
    }
}
