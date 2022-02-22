use std::fs::canonicalize;
use std::path::{Path, PathBuf};

pub struct KbdgenBundle {
    pub path: PathBuf,
}

pub fn read_kbdgen_bundle(path: &Path) -> Result<KbdgenBundle, Error> {

    let canonical_bundle_path: PathBuf = canonicalize(path)?;

    println!("Canonical Bundle Path: {:?}", &canonical_bundle_path);

    Ok(KbdgenBundle {
        path: canonical_bundle_path,
    })
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    IoError(#[from] std::io::Error),

    #[error("{0}")]
    YamlError(#[from] serde_yaml::Error),
}
