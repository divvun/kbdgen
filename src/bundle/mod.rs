use std::path::Path;

pub struct KbdgenBundle {

}

pub fn read_kbdgen_bundle(path: &Path) -> Result<KbdgenBundle, Error> {
    Ok(KbdgenBundle {

    })
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    IoError(#[from] std::io::Error),

    #[error("{0}")]
    YamlError(#[from] serde_yaml::Error),
}
