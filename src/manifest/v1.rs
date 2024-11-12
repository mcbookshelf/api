use serde::de;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Clone, Debug, Serialize)]
pub struct Manifest {
    pub datapacks: Vec<Datapack>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Datapack {
    pub name: String,
    pub modules: Vec<Module>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Module {
    #[serde(rename = "name")]
    pub id: String,
    #[serde(rename = "display_name")]
    pub name: String,
    #[serde(default)]
    pub documentation: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub dependencies: Vec<String>,
    #[serde(default)]
    pub weak_dependencies: Vec<String>,
}

impl<'de> Deserialize<'de> for Manifest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        let value: Value = Value::deserialize(deserializer)?;
        match value
            .as_array()
            .or_else(|| value.get("datapacks").and_then(|v| v.as_array()))
        {
            None => Err(de::Error::custom(
                "Expected either an array or an object with a 'datapacks' key",
            )),
            Some(datapacks) => Ok(Manifest {
                datapacks: serde_json::from_value(json!(datapacks)).map_err(de::Error::custom)?,
            }),
        }
    }
}
