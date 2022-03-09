use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub enum Transform {
    End(String),
    More(IndexMap<String, Transform>),
}
