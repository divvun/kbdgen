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

#[derive(Serialize, Deserialize)]
pub struct ManifestBackground {
    scripts: Vec<String>,
}

#[derive(Serialize, Deserialize)]
pub struct ManifestInputComponent {
    name: String,
    #[serde(rename = "type")]
    input_type: String,
    id: String,
    description: String,
    language: String,
    layouts: Vec<String>,
}

impl ManifestInputComponent {
    fn from_config(language_tag: String, locale: LanguageTag, xkb_layout: String) -> Self {
        let underscore_name = format!("__MSG_{}__", language_tag.replace("-", "_"));
        Self {
            name: underscore_name.clone(),
            input_type: "ime".to_string(),
            id: language_tag.to_string(),
            description: underscore_name.clone(),
            language: locale.to_string(),
            layouts: vec![xkb_layout.to_string()], // should somehow be able to get more?
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct ManifestIcons {
    #[serde(rename = "16")]
    icon_16: String,
    #[serde(rename = "48")]
    icon_48: String,
    #[serde(rename = "128")]
    icon_128: String,
}

#[derive(Serialize, Deserialize)]
pub struct ChromeOsManifest {
    name: String,
    version: String,
    version_name: String,
    manifest_version: u8,
    description: String,
    background: ManifestBackground,
    permissions: Vec<String>,
    input_components: Vec<ManifestInputComponent>,
    default_locale: String,
    icons: ManifestIcons,
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
        let manifest_file_path = output_path.join("manifest.json");

        let template = KEYBOARD_TEMPLATE.clone();

        let mut descriptor = IndexMap::new();

        let mut json_transforms: IndexMap<String, IndexMap<String, String>>;

        let mut manifest_input_components: Vec<ManifestInputComponent> = Vec::new();
        // layout information is to be aggregated into a descriptor and then appended
        // to the end of the template
        for (language_tag, layout) in &bundle.layouts {
            if let Some(chromeos_target) = &layout.chrome_os {
                let input_component = ManifestInputComponent::from_config(
                    language_tag.to_string(),
                    chromeos_target
                        .config
                        .locale
                        .as_ref()
                        .map(|x| x.clone())
                        .unwrap_or_else(|| "en-US".parse().unwrap()),
                    chromeos_target
                        .config
                        .xkb_layout
                        .clone()
                        .unwrap_or_else(|| "us".to_string()),
                );

                manifest_input_components.push(input_component);

                let mut json_dead_keys = IndexMap::new();
                if let Some(dead_keys) = &chromeos_target.dead_keys {
                    for (dead_key_name, dead_key_list) in dead_keys {
                        json_dead_keys.insert(dead_key_name.clone(), dead_key_list.to_vec());
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

                let mut json_layers = IndexMap::new();
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

                    let mut modifiers = IndexMap::new();
                    for (cursor, (_iso_key, modifier_name)) in CHROMEOS_KEYS.iter().enumerate() {
                        modifiers.insert(modifier_name.clone(), key_map[cursor].clone());
                    }

                    json_layers.insert(layer_name.clone(), modifiers);
                }

                descriptor.insert(
                    language_tag.clone(),
                    ChromeOsDescriptor {
                        dead_keys: json_dead_keys,
                        transforms: json_transforms,
                        layers: json_layers,
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

        if let Some(target) = &bundle.targets.chromeos {
            let manifest = ChromeOsManifest {
                name: "__MSG_name__".to_string(),
                version: target.build.clone(),
                version_name: target.version.clone(),
                manifest_version: 2,
                description: "__MSG_description__".to_string(),
                background: ManifestBackground {
                    scripts: vec!["background.js".to_string()],
                },
                permissions: vec!["input".to_string()],
                input_components: manifest_input_components,
                default_locale: "en".to_string(),
                icons: ManifestIcons {
                    icon_16: "icon16.png".to_string(),
                    icon_48: "icon48.png".to_string(),
                    icon_128: "icon128.png".to_string(),
                },
            };

            std::fs::write(
                output_path.join(manifest_file_path),
                serde_json::to_string_pretty(&manifest).unwrap(),
            )
            .unwrap();
        }
    }
}
