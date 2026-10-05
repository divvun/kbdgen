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
    pub layouts: Layouts,
    pub targets: Targets,
    pub resources: Resources,
}

/// A bundle's layouts keyed by language tag, always iterated in bundle layout
/// order: ascending by the tag's string form (`LanguageTag::as_str`), compared
/// byte-wise. The order is established on construction and no method can
/// insert or re-key an entry, so every iteration visits the layouts in the
/// same order on every run.
// [spec:kbdgen:req:bundle.layouts+1]
#[derive(Debug, Default)]
pub struct Layouts(IndexMap<LanguageTag, Layout>);

impl Layouts {
    pub fn iter(&self) -> indexmap::map::Iter<'_, LanguageTag, Layout> {
        self.0.iter()
    }

    pub fn keys(&self) -> indexmap::map::Keys<'_, LanguageTag, Layout> {
        self.0.keys()
    }

    pub fn values(&self) -> indexmap::map::Values<'_, LanguageTag, Layout> {
        self.0.values()
    }

    pub fn get(&self, tag: &LanguageTag) -> Option<&Layout> {
        self.0.get(tag)
    }

    pub fn get_mut(&mut self, tag: &LanguageTag) -> Option<&mut Layout> {
        self.0.get_mut(tag)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// Collects into bundle layout order. A tag seen twice keeps the value of
/// its last occurrence.
// [spec:kbdgen:req:bundle.layouts+1]
impl FromIterator<(LanguageTag, Layout)> for Layouts {
    fn from_iter<I: IntoIterator<Item = (LanguageTag, Layout)>>(iter: I) -> Self {
        let mut map: IndexMap<LanguageTag, Layout> = iter.into_iter().collect();
        map.sort_unstable_by(|a, _, b, _| a.as_str().cmp(b.as_str()));
        Layouts(map)
    }
}

impl<'a> IntoIterator for &'a Layouts {
    type Item = (&'a LanguageTag, &'a Layout);
    type IntoIter = indexmap::map::Iter<'a, LanguageTag, Layout>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl KbdgenBundle {
    // [spec:kbdgen:def:bundle.structure]
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

// [spec:kbdgen:def:bundle.structure]
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

// [spec:kbdgen:req:bundle.layouts+1]
fn read_layouts(path: &Path) -> Result<Layouts, Error> {
    tracing::debug!("Reading layouts");
    let mut paths: Vec<PathBuf> = read_dir(path)
        .map_err(|e| Error::Io(path.to_path_buf(), e))?
        .filter_map(Result::ok)
        .map(|file| file.path())
        .filter(|path| path.is_file())
        .filter(|path| match path.extension() {
            Some(ext) => ext == YAML_EXT,
            None => false,
        })
        .collect();
    // Files are loaded in byte-wise file-name order, which fixes both the
    // reported error when several layouts fail and the survivor when two
    // stems normalise to the same tag.
    paths.sort_by(|a, b| {
        a.file_name()
            .map(|n| n.as_encoded_bytes())
            .cmp(&b.file_name().map(|n| n.as_encoded_bytes()))
    });

    paths
        .into_iter()
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

            // [spec:kbdgen:req:bundle.layouts.autonym]
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

            // [spec:kbdgen:req:bundle.layouts+1]
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

// [spec:kbdgen:req:bundle.structure.targets.env]
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

// [spec:kbdgen:def:bundle.resources]
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

// [spec:kbdgen:req:bundle.structure.targets]
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

/// Builds bundle directories on disk for tests that exercise loading.
#[cfg(test)]
pub(crate) mod fixture {
    use std::fs;
    use std::path::{Path, PathBuf};

    /// Writes `<root>/<name>.kbdgen` with a minimal `project.yaml`, one
    /// `layouts/<stem>.yaml` per `layouts` entry, one `targets/<stem>.yaml`
    /// per `targets` entry and an empty `resources/<dir>` per `resources`
    /// entry, written in the given order. Returns the bundle path.
    pub(crate) fn write_bundle(
        root: &Path,
        name: &str,
        layouts: &[(&str, &str)],
        targets: &[(&str, &str)],
        resources: &[&str],
    ) -> PathBuf {
        let bundle = root.join(format!("{name}.kbdgen"));
        for dir in ["layouts", "targets", "resources"] {
            fs::create_dir_all(bundle.join(dir)).unwrap();
        }
        fs::write(
            bundle.join("project.yaml"),
            "locales:\n  en:\n    name: Test\n    description: Test\nauthor: Test\ncopyright: Test\nemail: test@example.com\norganisation: Test\n",
        )
        .unwrap();
        for (stem, yaml) in layouts {
            fs::write(bundle.join("layouts").join(format!("{stem}.yaml")), yaml).unwrap();
        }
        for (stem, yaml) in targets {
            fs::write(bundle.join("targets").join(format!("{stem}.yaml")), yaml).unwrap();
        }
        for dir in resources {
            fs::create_dir_all(bundle.join("resources").join(dir)).unwrap();
        }
        bundle
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout_yaml(tag: &str, autonym: &str) -> String {
        let primary = tag.split('-').next().unwrap();
        format!("languageTag: {tag}\ndisplayNames:\n  {primary}: {autonym}\n")
    }

    fn tags(layouts: &Layouts) -> Vec<&str> {
        layouts.keys().map(LanguageTag::as_str).collect()
    }

    // [spec:kbdgen:req:bundle.layouts+1/test]
    #[test]
    fn loaded_layouts_iterate_in_ascending_tag_order() {
        let root = tempfile::tempdir().unwrap();
        let path = fixture::write_bundle(
            root.path(),
            "ordered",
            &[
                ("sme", "displayNames:\n  sme: Davvisámegiella\n"),
                ("se-fi", "displayNames:\n  se: Davvisámegiella\n"),
                ("sma", "displayNames:\n  sma: Åarjelsaemien\n"),
                ("se", "displayNames:\n  se: Davvisámegiella\n"),
                ("en", "displayNames:\n  en: English\n"),
                ("smj-Latn", "displayNames:\n  smj: Julevsámegiella\n"),
            ],
            &[],
            &[],
        );

        let bundle = read_kbdgen_bundle(&path).unwrap();

        assert_eq!(
            tags(&bundle.layouts),
            ["en", "se", "se-FI", "sma", "sme", "smj-Latn"]
        );
        let iterated: Vec<&str> = (&bundle.layouts)
            .into_iter()
            .map(|(tag, layout)| {
                assert_eq!(tag, &layout.language_tag);
                tag.as_str()
            })
            .collect();
        assert_eq!(iterated, tags(&bundle.layouts));
        let values: Vec<&str> = bundle
            .layouts
            .values()
            .map(|layout| layout.language_tag.as_str())
            .collect();
        assert_eq!(values, tags(&bundle.layouts));
    }

    // [spec:kbdgen:req:bundle.layouts+1/test]
    #[test]
    fn layout_order_ignores_insertion_order() {
        let entries = ["sme", "se-FI", "en", "sma", "se", "fkv", "en-GB"];
        let collect = |order: &[usize]| -> Layouts {
            order
                .iter()
                .map(|&i| {
                    let tag: LanguageTag = entries[i].parse().unwrap();
                    let layout: Layout =
                        serde_yaml::from_str(&layout_yaml(entries[i], "Autonym")).unwrap();
                    (tag, layout)
                })
                .collect()
        };

        let expected = ["en", "en-GB", "fkv", "se", "se-FI", "sma", "sme"];
        for order in [
            [0, 1, 2, 3, 4, 5, 6],
            [6, 5, 4, 3, 2, 1, 0],
            [3, 0, 6, 1, 5, 2, 4],
        ] {
            assert_eq!(tags(&collect(&order)), expected, "{order:?}");
        }
    }

    // [spec:kbdgen:req:bundle.layouts+1/test]
    #[test]
    fn duplicate_tags_keep_the_last_file_by_name() {
        let root = tempfile::tempdir().unwrap();
        let path = fixture::write_bundle(
            root.path(),
            "duplicates",
            &[
                ("se-fi", "displayNames:\n  se: lower\n"),
                ("se-FI", "displayNames:\n  se: upper\n"),
            ],
            &[],
            &[],
        );
        let case_sensitive = std::fs::read_dir(path.join("layouts")).unwrap().count() == 2;

        let bundle = read_kbdgen_bundle(&path).unwrap();

        assert_eq!(tags(&bundle.layouts), ["se-FI"]);
        let tag: LanguageTag = "se-FI".parse().unwrap();
        let se: LanguageTag = "se".parse().unwrap();
        let autonym = &bundle.layouts.get(&tag).unwrap().display_names[&se];
        if case_sensitive {
            // `se-fi.yaml` sorts after `se-FI.yaml` byte-wise ('f' > 'F').
            assert_eq!(autonym, "lower");
        }
    }
}
