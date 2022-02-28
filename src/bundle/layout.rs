use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Layout {
    #[serde(rename = "displayNames")]
    pub display_names: IndexMap<String, String>,

    pub layers: IndexMap<String, IndexMap<String, IndexMap<String, String>>>,
}

pub trait KeyboardBuild {
    fn build() {

    }

    // Target, Vec<Layers>


}

// May need to read the files into an expected yaml format first
// targets first
// targets become external categories
// then read the layers as classified by targets
// 