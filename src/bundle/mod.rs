use std::collections::HashMap;
use std::fs;
use std::fs::{canonicalize, read_dir};
use std::path::{Path, PathBuf};

use language_tags::LanguageTag;

use layout::Layout;
use project::Project;
use serde_yaml::Value;
use target::Targets;

use self::resources::Resources;

pub mod layout;
pub(crate) mod project;
pub(crate) mod resources;
pub(crate) mod target;

const PROJECT_FILENAME: &str = "project.yaml";
const LAYOUTS_FOLDER: &str = "layouts";
const TARGETS_FOLDER: &str = "targets";
const RESOURCES_FOLDER: &str = "resources";

const YAML_EXT: &str = "yaml";

pub const DEFAULT_DECIMAL: &str = ".";
const COMMA_DECIMAL: &str = ",";

#[derive(Debug)]
pub struct KbdgenBundle {
    pub path: PathBuf,
    pub project: Project,
    pub layouts: HashMap<LanguageTag, Layout>,
    pub targets: Targets,
    pub resources: Resources,
}

impl KbdgenBundle {
    pub fn name(&self) -> &str {
        self.path
            .file_stem()
            .expect("No file stem?")
            .to_str()
            .expect("Must be valid utf-8")
    }
}

pub fn read_kbdgen_bundle(path: &Path) -> Result<KbdgenBundle, Error> {
    let canonical_bundle_path: PathBuf = canonicalize(path)?;

    tracing::info!("Canonical Bundle Path: {:?}", &canonical_bundle_path);

    let project: Project = serde_yaml::from_str(&fs::read_to_string(
        canonical_bundle_path.join(PROJECT_FILENAME),
    )?)?;

    let layouts_path = canonical_bundle_path.join(LAYOUTS_FOLDER);
    let targets_path = canonical_bundle_path.join(TARGETS_FOLDER);
    let resources_path = canonical_bundle_path.join(RESOURCES_FOLDER);

    let layouts = read_layouts(&layouts_path)?;
    let targets = read_targets(&targets_path)?;
    let resources = read_resources(&resources_path)?;

    Ok(KbdgenBundle {
        path: canonical_bundle_path,
        project,
        layouts,
        targets,
        resources,
    })
}

fn read_layouts(path: &Path) -> Result<HashMap<LanguageTag, Layout>, Error> {
    tracing::debug!("Reading layouts");
    read_dir(path)?
        .filter_map(Result::ok)
        .map(|file| file.path())
        .filter(|path| path.is_file())
        .filter(|path| match path.extension() {
            Some(ext) => ext == YAML_EXT,
            None => false,
        })
        .map(|path| {
            tracing::debug!("Loading {}", path.display());
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
                .get(&tag.primary_language().parse::<LanguageTag>().unwrap())
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

fn load_yaml<T>(path: &Path) -> Option<T>
where
    T: for<'de> serde::Deserialize<'de>,
{
    let s = match fs::read_to_string(path) {
        Ok(v) => v,
        Err(e) => {
            tracing::error!(error = ?e, "Failed to read file to string");
            return None;
        }
    };

    match serde_yaml::from_str::<T>(&s) {
        Ok(v) => Some(v),
        Err(e) => {
            tracing::error!(error = ?e, "Error parsing YAML");
            None
        }
    }
}

fn read_resources(path: &Path) -> Result<Resources, Error> {
    tracing::debug!("Reading resources");
    let mut resources = Resources::default();

    let iter = read_dir(path)?
        .filter_map(Result::ok)
        .map(|file| file.path())
        .filter(|path| path.is_dir());

    for path in iter {
        let target_name = path.file_name().map(|x| x.to_string_lossy()).unwrap();

        match target_name.as_ref() {
            "macos" => {
                resources.macos = resources::MacOS::load(&path).ok();
            }
            "chromeos" => {
                resources.chromeos = resources::ChromeOS::load(&path).ok();
            }
            "android" => {
                resources.android = resources::Android::load(&path).ok();
            }
            name => {
                tracing::warn!("Saw resource folder with name {name} but did not parse");
                continue;
            }
        };
    }
    Ok(resources)
}

fn read_targets(path: &Path) -> Result<Targets, Error> {
    tracing::debug!("Reading targets");
    let mut targets = Targets::default();

    let iter = read_dir(path)?
        .filter_map(Result::ok)
        .map(|file| file.path())
        .filter(|path| path.is_file())
        .filter(|path| match path.extension() {
            Some(ext) => ext == YAML_EXT,
            None => false,
        });

    for path in iter {
        let target_name = path
            .file_stem()
            .ok_or_else(|| Error::NoFileStem { path: path.clone() })
            .map(|x| x.to_string_lossy())?;

        match target_name.as_ref() {
            "windows" => {
                targets.windows = load_yaml(&path);
            }
            "ios" => {
                targets.ios = load_yaml(&path);
            }
            "macos" => {
                targets.macos = load_yaml(&path);
            }
            "chromeos" => targets.chromeos = load_yaml(&path),
            name => {
                tracing::warn!("Saw target with name {name} but did not parse");
                continue;
            }
        };
    }

    Ok(targets)
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
