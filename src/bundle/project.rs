use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Project {
    pub locales: IndexMap<String, LocaleProjectDescription>,
    pub author: String,
    pub copyright: String,
    pub email: String,
    pub organisation: String,
    #[serde(default)]
    pub dependencies: IndexMap<String, Dependency>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Dependency {
    pub url: String,
    pub layouts: Vec<String>,
    #[serde(default)]
    pub branch: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LocaleProjectDescription {
    pub name: String,
    pub description: String,
}
