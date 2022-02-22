use std::fs;
use std::fs::canonicalize;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug)]
pub struct KbdgenBundle {
    pub path: PathBuf,
    pub project: Project,
}

pub fn read_kbdgen_bundle(path: &Path) -> Result<KbdgenBundle, Error> {
    let canonical_bundle_path: PathBuf = canonicalize(path)?;

    println!("Canonical Bundle Path: {:?}", &canonical_bundle_path);

    let project: Project = serde_yaml::from_str(&fs::read_to_string(
        canonical_bundle_path.join("project.yaml"),
    )?)?;

    Ok(KbdgenBundle {
        path: canonical_bundle_path,
        project,
    })
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Project {
    pub author: String,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    IoError(#[from] std::io::Error),

    #[error("{0}")]
    YamlError(#[from] serde_yaml::Error),
}
