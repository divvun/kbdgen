use std::collections::HashMap;
use std::fs;
use std::fs::{canonicalize, read_dir};
use std::path::{Path, PathBuf};

use language_tags::LanguageTag;

use layout::Layout;
use project::Project;
use serde_yaml::Value;
use target::Target;

pub mod layout;
mod project;
mod target;

const PROJECT_FILENAME: &str = "project.yaml";
const LAYOUTS_FOLDER: &str = "layouts";
const TARGETS_FOLDER: &str = "targets";

const YAML_EXT: &str = "yaml";

pub const DEFAULT_DECIMAL: &str = ".";
const COMMA_DECIMAL: &str = ",";

#[derive(Debug)]
pub struct KbdgenBundle {
    pub path: PathBuf,
    pub project: Project,
    pub layouts: HashMap<LanguageTag, Layout>,
    pub targets: Vec<Target>,
}

pub fn read_kbdgen_bundle(path: &Path) -> Result<KbdgenBundle, Error> {
    let canonical_bundle_path: PathBuf = canonicalize(path)?;

    tracing::info!("Canonical Bundle Path: {:?}", &canonical_bundle_path);

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

            let mut yaml: Value = serde_yaml::from_str(&fs::read_to_string(path)?)?;
            yaml.as_mapping_mut()
                .expect("top level yaml type must be a mapping")
                .insert(
                    Value::String("languageTag".to_owned()),
                    Value::String(tag.to_string()),
                );

            let mut layout: Layout = serde_yaml::from_value(yaml)?;

            let _autonym = layout
                .display_names
                .get(&tag.primary_language().to_owned())
                .unwrap_or_else(|| {
                    panic!("displayNames for the layout do not have the autonym");
                });

            if let Some(decimal) = layout.decimal.as_ref() {
                if decimal != COMMA_DECIMAL && decimal != DEFAULT_DECIMAL {
                    tracing::error!(
                        "{} is not supported as a decimal character, setting to {}",
                        decimal,
                        DEFAULT_DECIMAL
                    );
                    layout.decimal = Some(DEFAULT_DECIMAL.to_owned());
                }
            };

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
                "windows" => {
                    let win_target = serde_yaml::from_str(&fs::read_to_string(path)?)?;
                    Target::Windows(win_target)
                }
                "ios" => {
                    #[allow(non_snake_case)]
                    let iOS_target = serde_yaml::from_str(&fs::read_to_string(path)?)?;
                    Target::iOS(iOS_target)
                }
                _ => {
                    todo!()
                }
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
