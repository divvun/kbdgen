use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Project {
    pub locales: IndexMap<String, LocaleProjectDescription>,
    pub author: String,
    pub copyright: String,
    pub email: String,
    pub organisation: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LocaleProjectDescription {
    pub name: String,
    pub description: String,
}
