use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct LocaleProjectDescription {
    pub name: String,
    pub description: String,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Project {
    pub locales: IndexMap<String, LocaleProjectDescription>,
    pub author: String,
    pub copyright: String,
    pub email: String,
    pub organisation: String,
}
