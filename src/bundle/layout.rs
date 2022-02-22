use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Layout {
    #[serde(rename = "displayNames")]
    pub display_names: IndexMap<String, String>,
}
