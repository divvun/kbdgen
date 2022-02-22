use std::fs;
use std::fs::canonicalize;
use std::path::{Path, PathBuf};

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

static PROJECT_FILENAME: &str = "project.yaml";

#[derive(Debug)]
pub struct KbdgenBundle {
    pub path: PathBuf,
    pub project: Project,
}

pub fn read_kbdgen_bundle(path: &Path) -> Result<KbdgenBundle, Error> {
    let canonical_bundle_path: PathBuf = canonicalize(path)?;

    println!("Canonical Bundle Path: {:?}", &canonical_bundle_path);

    let project: Project = serde_yaml::from_str(&fs::read_to_string(
        canonical_bundle_path.join(PROJECT_FILENAME),
    )?)?;

    Ok(KbdgenBundle {
        path: canonical_bundle_path,
        project,
    })
}

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

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    IoError(#[from] std::io::Error),

    #[error("{0}")]
    YamlError(#[from] serde_yaml::Error),
}
