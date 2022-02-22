use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Layout {
    #[serde(rename = "displayNames")]
    pub display_names: IndexMap<String, String>,

    pub layers: Layers,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Layers {}
