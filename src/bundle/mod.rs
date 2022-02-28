use std::collections::HashMap;
use std::fs;
use std::fs::{canonicalize, read_dir};
use std::path::{Path, PathBuf};

use language_tags::LanguageTag;

use layout::Layout;
use target::Target;
use project::Project;

mod layout;
mod target;
mod project;

const PROJECT_FILENAME: &str = "project.yaml";
const LAYOUTS_FOLDER: &str = "layouts";
const TARGETS_FOLDER: &str = "targets";

const YAML_EXT: &str = "yaml";

#[derive(Debug)]
pub struct KbdgenBundle {
    pub path: PathBuf,
    pub project: Project,
    pub layouts: HashMap<LanguageTag, Layout>,
    pub targets: Vec<Target>,
}

pub fn read_kbdgen_bundle(path: &Path) -> Result<KbdgenBundle, Error> {
    let canonical_bundle_path: PathBuf = canonicalize(path)?;

    println!("Canonical Bundle Path: {:?}", &canonical_bundle_path);

    let project: Project = serde_yaml::from_str(&fs::read_to_string(
        canonical_bundle_path.join(PROJECT_FILENAME),
    )?)?;

    let layouts_path = canonical_bundle_path.join(LAYOUTS_FOLDER);
    let targets_path = canonical_bundle_path.join(TARGETS_FOLDER);

    let layouts = read_layouts(&layouts_path)?;
    let targets = read_targets(&targets_path)?;

    Ok(KbdgenBundle {
        path: canonical_bundle_path,
        project,
        layouts,
        targets,
    })
}

fn read_layouts(path: &Path) -> Result<HashMap<LanguageTag, Layout>, Error> {
    read_dir(path)?
        .filter_map(Result::ok)
        .map(|file| file.path())
        .filter(|path| path.is_file())
        .filter(|path| match path.extension() {
            Some(ext) => ext == YAML_EXT,
            None => false,
        })
        .map(|path| {
            let tag = path
                .file_stem()
                .ok_or_else(|| Error::NoFileStem { path: path.clone() })?
                .to_string_lossy();

            let tag: LanguageTag = tag.parse().map_err(|_| Error::InvalidLanguageTag {
                tag: tag.to_string(),
            })?;

            let layout: Layout = serde_yaml::from_str(&fs::read_to_string(path)?)?;

            Ok((tag, layout))
        })
        .collect()
}

fn read_targets(path: &Path) -> Result<Vec<Target>, Error> {
    read_dir(path)?
        .filter_map(Result::ok)
        .map(|file| file.path())
        .filter(|path| path.is_file())
        .filter(|path| match path.extension() {
            Some(ext) => ext == YAML_EXT,
            None => false,
        })
        .map(|path| {
            let target_name = path
                .file_stem()
                .ok_or_else(|| Error::NoFileStem { path: path.clone() })?
                .to_string_lossy();

            let target: Target = match target_name.as_ref() {
                "ios" => {
                    let iOS_target = serde_yaml::from_str(&fs::read_to_string(path)?)?;
                    Target::iOS(iOS_target)
                },
                _ => {
                    todo!()
                },
            };
            
            Ok(target)
        })
        .collect()
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    IoError(#[from] std::io::Error),

    #[error("{0}")]
    YamlError(#[from] serde_yaml::Error),

    #[error(".yaml files must have a stem, failed to parse: `{}`", path.display())]
    NoFileStem { path: PathBuf },

    #[error("Failed to parse language tag: `{}`", tag)]
    InvalidLanguageTag { tag: String },
}
