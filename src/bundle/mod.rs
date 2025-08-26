use std::collections::HashMap;
use std::fs;
use std::fs::{canonicalize, read_dir};
use std::path::{Path, PathBuf};

use indexmap::IndexMap;
use language_tags::LanguageTag;

use layout::Layout;
use project::Project;
use serde_yaml::Value;
use target::Targets;

use self::resources::Resources;

pub(crate) mod fetch;
pub mod layout;
pub(crate) mod project;
pub(crate) mod resources;
pub(crate) mod target;

pub use fetch::fetch;

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

    #[cfg(test)]
    pub fn new_test(name: String, layouts: IndexMap<LanguageTag, Layout>) -> Self {
        KbdgenBundle {
            path: PathBuf::from(format!("/test/{}", name)),
            project: project::Project {
                locales: IndexMap::new(),
                organisation: "Test".to_string(),
                author: "Test".to_string(),
                email: "test@example.com".to_string(),
                copyright: "Test".to_string(),
                dependencies: IndexMap::new(),
            },
            layouts: layouts.into_iter().collect(),
            targets: target::Targets::default(),
            resources: resources::Resources::default(),
        }
    }
}

pub fn read_kbdgen_bundle(path: &Path) -> Result<KbdgenBundle, Error> {
    let canonical_bundle_path: PathBuf =
        canonicalize(path).map_err(|e| Error::Io(path.to_path_buf(), e))?;

    tracing::info!("Canonical Bundle Path: {:?}", &canonical_bundle_path);

    let project_text = fs::read_to_string(canonical_bundle_path.join(PROJECT_FILENAME))
        .map_err(|e| Error::Io(path.to_path_buf(), e))?;
    let deserializer = serde_yaml::Deserializer::from_str(&project_text);
    let project: Project = serde_path_to_error::deserialize(deserializer)
        .map_err(|e| Error::Yaml(path.to_path_buf(), e))?;

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
    read_dir(path)
        .map_err(|e| Error::Io(path.to_path_buf(), e))?
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

            let yaml_text =
                fs::read_to_string(&path).map_err(|e| Error::Io(path.to_path_buf(), e))?;
            let deserializer = serde_yaml::Deserializer::from_str(&yaml_text);
            let mut yaml: Value = serde_path_to_error::deserialize(deserializer)
                .map_err(|e| Error::Yaml(path.to_path_buf(), e))?;
            yaml.as_mapping_mut()
                .expect("top level yaml type must be a mapping")
                .insert(
                    Value::String("languageTag".to_owned()),
                    Value::String(tag.to_string()),
                );

            let mut layout: Layout = serde_path_to_error::deserialize(yaml)
                .map_err(|e| Error::Yaml(path.to_path_buf(), e))?;

            let _autonym = match layout
                .display_names
                .get(&tag.primary_language().parse::<LanguageTag>().unwrap())
            {
                Some(v) => v,
                None => {
                    return Err(Error::MissingMandatoryDisplayName {
                        tag: tag.to_string(),
                    });
                }
            };

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

fn load_yaml<T>(path: &Path) -> Result<T, Error>
where
    T: for<'de> serde::Deserialize<'de>,
{
    let s = match fs::read_to_string(path) {
        Ok(v) => v,
        Err(e) => {
            return Err(Error::Io(path.to_path_buf(), e));
        }
    };

    let deserializer = serde_yaml::Deserializer::from_str(&s);
    serde_path_to_error::deserialize(deserializer).map_err(|e| Error::Yaml(path.to_path_buf(), e))
}

fn load_yaml_with_env<T>(path: &Path, env_vars: HashMap<&str, &str>) -> Result<T, Error>
where
    T: for<'de> serde::Deserialize<'de>,
{
    let s = match fs::read_to_string(path) {
        Ok(v) => v,
        Err(e) => {
            return Err(Error::Io(path.to_path_buf(), e));
        }
    };

    let deserializer = serde_yaml::Deserializer::from_str(&s);
    let mut raw: serde_yaml::Value = serde_path_to_error::deserialize(deserializer)
        .map_err(|e| Error::Yaml(path.to_path_buf(), e))?;

    {
        let root = raw.as_mapping_mut().unwrap();

        for (env_var, field_name) in env_vars {
            let value = match std::env::var(env_var) {
                Ok(v) => v,
                Err(_) => continue,
            };

            root.insert(
                serde_yaml::Value::String(field_name.to_string()),
                serde_yaml::Value::String(value),
            );
        }
    }

    match serde_path_to_error::deserialize(raw) {
        Ok(v) => Ok(v),
        Err(e) => Err(Error::Yaml(path.to_path_buf(), e)),
    }
}

fn read_resources(path: &Path) -> Result<Resources, Error> {
    tracing::debug!("Reading resources");
    let mut resources = Resources::default();

    let iter = read_dir(path)
        .map_err(|e| Error::Io(path.to_path_buf(), e))?
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
            "ios" => {
                resources.ios = resources::IOS::load(&path).ok();
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

    let iter = read_dir(path)
        .map_err(|e| Error::Io(path.to_path_buf(), e))?
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
                targets.windows = load_yaml(&path)?;
            }
            "ios" => {
                targets.ios = load_yaml_with_env(
                    &path,
                    [
                        ("MATCH_GIT_URL", "matchGitUrl"),
                        ("MATCH_PASSWORD", "matchPassword"),
                        ("FASTLANE_USER", "fastlaneUser"),
                        ("PRODUCE_USERNAME", "fastlaneUser"),
                        ("FASTLANE_PASSWORD", "fastlanePassword"),
                        ("APP_STORE_KEY_JSON", "appStoreKeyJson"),
                        ("TEAM_ID", "teamId"),
                        ("CODE_SIGN_ID", "codeSignId"),
                    ]
                    .into(),
                )?;
            }
            "macos" => {
                targets.macos = load_yaml(&path)?;
            }
            "chromeos" => targets.chromeos = load_yaml(&path)?,
            "android" => {
                targets.android = load_yaml_with_env(
                    &path,
                    [
                        ("ANDROID_KEYSTORE", "keyStore"),
                        ("ANDROID_KEYALIAS", "keyAlias"),
                        ("PLAY_STORE_ACCOUNT", "playStoreAccount"),
                        ("PLAY_STORE_P12", "playStoreP12"),
                        ("STORE_PW", "storePassword"),
                        ("KEY_PW", "keyPassword"),
                    ]
                    .into(),
                )?
            }
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
    #[error("IO error for path: {0}")]
    Io(PathBuf, #[source] std::io::Error),

    #[error("Error parsing YAML for path: {0} at {}", .1.path())]
    Yaml(
        PathBuf,
        #[source] serde_path_to_error::Error<serde_yaml::Error>,
    ),

    #[error(".yaml files must have a stem, failed to parse: `{}`", path.display())]
    NoFileStem { path: PathBuf },

    #[error("Failed to parse language tag: `{}`", tag)]
    InvalidLanguageTag { tag: String },

    #[error("Missing mandatory display name for language tag: `{}`", tag)]
    MissingMandatoryDisplayName { tag: String },
}
