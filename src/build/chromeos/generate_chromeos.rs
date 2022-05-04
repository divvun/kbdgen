use std::{fmt, path::Path};

use crate::build::chromeos::keymap::CHROMEOS_KEYS;
use async_trait::async_trait;
use indexmap::IndexMap;
use language_tags::LanguageTag;
use pahkat_client::types::repo::Index;
use serde::{Deserialize, Serialize};
use serde_json::{json, to_string_pretty};

use crate::{
    build::BuildStep,
    bundle::{
        layout::{chrome::ChromeOsKbdLayer, Transform},
        KbdgenBundle,
    },
    util::split_keys,
};

const KEYBOARD_TEMPLATE: &str = include_str!("../../../resources/template-chromeos-keyboard.js");

#[derive(Serialize, Deserialize)]
pub struct ChromeOsBackground {
    template: String,
    descriptor: IndexMap<LanguageTag, ChromeOsDescriptor>,
}

impl fmt::Display for ChromeOsBackground {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}\n", self.template)?;
        write!(f, "\n")?;
        write!(
            f,
            "const descriptor = {}\n",
            to_string_pretty(&self.descriptor).map_err(|err| fmt::Error)?
        )?;
        write!(f, "\nKeyboard.install(descriptor)")?;

        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
pub struct ChromeOsDescriptor {
    pub dead_keys: IndexMap<ChromeOsKbdLayer, Vec<String>>,
    pub transforms: IndexMap<String, IndexMap<String, String>>,
    pub layers: IndexMap<ChromeOsKbdLayer, IndexMap<String, String>>,
}

impl fmt::Display for ChromeOsDescriptor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            to_string_pretty(&self.dead_keys).map_err(|err| fmt::Error)?
        )?;
        write!(
            f,
            "{}",
            to_string_pretty(&self.transforms).map_err(|err| fmt::Error)?
        )?;

        Ok(())
    }
}

pub struct GenerateChromeOs;

#[async_trait(?Send)]
impl BuildStep for GenerateChromeOs {
    async fn build(&self, bundle: &KbdgenBundle, output_path: &Path) {
        let background_file_path = output_path.join("background.js");

        let template = KEYBOARD_TEMPLATE.clone();

        let mut descriptor = IndexMap::new();

        let mut json_transforms: IndexMap<String, IndexMap<String, String>>;
        // layout information is to be aggregated into a descriptor and then appended
        // to the end of the template
        for (language_tag, layout) in &bundle.layouts {
            if let Some(chromeos_target) = &layout.chrome_os {
                let mut dead_keys = IndexMap::new();
                if let Some(temp_dead_keys)  = &chromeos_target.dead_keys {
                    for (key, value) in temp_dead_keys {
                        dead_keys.insert(key.clone(), value.to_vec());
                    }
                }
                if let Some(layout_transforms) = &layout.transforms {
                    let layout_file_path = bundle
                        .path
                        .join("layouts")
                        .join(format!("{}.yaml", layout.language_tag));

                    let yaml_file = std::fs::File::open(&layout_file_path).unwrap();
                    let original_layout_yaml: serde_yaml::Value =
                        serde_yaml::from_reader(yaml_file).unwrap();

                    let owning_transform = *&original_layout_yaml.get("transforms").unwrap();

                    json_transforms = serde_yaml::from_value(owning_transform.clone()).unwrap();
                } else {
                    json_transforms = IndexMap::new();
                }

                let layers = &chromeos_target.primary.layers;

                let mut modifiers = IndexMap::new();
                for (layer_name, key_map) in layers {
                    let key_map: Vec<String> = split_keys(&key_map);

                    tracing::debug!(
                        "iso len: {}; keymap len: {}",
                        CHROMEOS_KEYS.len(),
                        key_map.len()
                    );
                    if CHROMEOS_KEYS.len() > key_map.len() {
                        panic!(
                            r#"Provided layer does not have enough keys, expected {} keys but got {}, in {}:{}:{}:{:?}: \n{:?}"#,
                            CHROMEOS_KEYS.len(),
                            key_map.len(),
                            language_tag.to_string(),
                            "Windows",
                            "Primary",
                            layer_name,
                            key_map
                        );
                    }

                    let mut keys = IndexMap::new();
                    for (cursor, (_iso_key, key_name)) in CHROMEOS_KEYS.iter().enumerate() {
                        keys.insert(key_name.clone(), key_map[cursor].clone());
                    }

                    modifiers.insert(layer_name.clone(), keys);
                }

                descriptor.insert(
                    language_tag.clone(),
                    ChromeOsDescriptor {
                        dead_keys: dead_keys,
                        transforms: json_transforms,
                        layers: modifiers,
                    },
                );
            }
        }

        let background = ChromeOsBackground {
            template: template.to_string(),
            descriptor,
        };

        std::fs::write(
            output_path.join(background_file_path),
            background.to_string(),
        )
        .unwrap();
    }
}
