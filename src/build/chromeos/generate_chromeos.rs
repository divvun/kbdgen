use std::{fmt, path::Path};

use anyhow::Result;
use async_trait::async_trait;
use indexmap::IndexMap;
use language_tags::LanguageTag;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use serde_json::to_string_pretty;

use crate::build::chromeos::keymap::CHROMEOS_KEYS;
use crate::build::chromeos::manifest::{
    ChromeOsManifest, ManifestBackground, ManifestIcons, ManifestInputComponent,
};
use crate::bundle::layout::{ChromeOsTarget, Layout};
use crate::{
    build::BuildStep,
    bundle::{layout::chrome::ChromeOsKbdLayer, KbdgenBundle},
    util::split_keys,
};

const BACKGROUND_FILE_NAME: &str = "background.js";
const MANIFEST_FILE_NAME: &str = "manifest.json";
const KEYBOARD_TEMPLATE: &str = include_str!("../../../resources/template-chromeos-keyboard.js");
const DEFAULT_LOCALE: &str = "en";

const DEFAULT_LONG_LOCALE: &str = "en-US";
const DEFAULT_XKB_LAYOUT: &str = "us";

const KEYBOARD_NAMES: Lazy<IndexMap<String, String>> = Lazy::new(|| {
    let mut map = IndexMap::new();

    {
        let arr = [
            ("nb", "{} tastatur"),
            ("no", "{} tastatur"),
            ("nn", "{} tastatur"),
            ("da", "{} tastatur"),
            ("sv", "{} tangentbord"),
            ("en", "{} keyboard"),
            ("fi", "{} näppäimistö"),
        ];

        for (key, value) in arr {
            map.insert(key.to_string(), value.to_string());
        }
    }

    map
});

// LOCALE START

#[derive(Serialize, Deserialize)]
pub struct LocaleMessage {
    message: String,
}

#[derive(Serialize, Deserialize)]
pub struct Locale {
    #[serde(flatten)]
    locales: IndexMap<String, LocaleMessage>,
    name: LocaleMessage,
    description: LocaleMessage,
}

// LOCALE END

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
            to_string_pretty(&self.descriptor).map_err(|_err| fmt::Error)?
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
            to_string_pretty(&self.dead_keys).map_err(|_err| fmt::Error)?
        )?;
        write!(
            f,
            "{}",
            to_string_pretty(&self.transforms).map_err(|_err| fmt::Error)?
        )?;

        Ok(())
    }
}

fn to_chrome_locale(locale: String) -> String {
    locale.replace("-", "_")
}

pub fn create_background(
    descriptor: IndexMap<LanguageTag, ChromeOsDescriptor>,
    output_folder_path: &Path,
) -> bool {
    if descriptor.len() > 0 {
        let template = KEYBOARD_TEMPLATE.clone();
        let background = ChromeOsBackground {
            template: template.to_string(),
            descriptor,
        };

        std::fs::write(
            output_folder_path.join(BACKGROUND_FILE_NAME),
            background.to_string(),
        )
        .unwrap();

        return true;
    }
    return false;
}

/// Creates a `manifest.json` file and populates it
pub fn create_manifest(
    bundle: &KbdgenBundle,
    input_components: Vec<ManifestInputComponent>,
    output_folder_path: &Path,
) -> bool {
    if let Some(target) = &bundle.targets.chromeos {
        let manifest = ChromeOsManifest {
            name: "__MSG_name__".to_string(),
            version: target.build.clone(),
            version_name: target.version.clone(),
            manifest_version: 2,
            description: "__MSG_description__".to_string(),
            background: ManifestBackground {
                scripts: vec![BACKGROUND_FILE_NAME.to_string()],
            },
            permissions: vec!["input".to_string()],
            input_components: input_components,
            default_locale: DEFAULT_LOCALE.to_string(),
            icons: ManifestIcons {
                icon_16: "icon16.png".to_string(),
                icon_48: "icon48.png".to_string(),
                icon_128: "icon128.png".to_string(),
            },
        };

        match std::fs::write(
            output_folder_path.join(MANIFEST_FILE_NAME),
            serde_json::to_string_pretty(&manifest)
                .expect(&format!("could not parse {:#?} to json", manifest)),
        ) {
            Ok(file) => file,
            Err(_) => return false,
        }

        return true;
    } else {
        return false;
    }
}

/// Creates a `_locales` folder with subfolders for each locale that is then populated with a `messages.json` file, which in turn contains metadata
pub fn create_locales(
    bundle: &KbdgenBundle,
    display_names: IndexMap<LanguageTag, String>,
    output_folder_path: &Path,
) -> bool {
    let locales_folder_path = output_folder_path.join("_locales");

    match std::fs::create_dir_all(&locales_folder_path) {
        Ok(folder) => folder,
        Err(_) => return false,
    }

    let default_locale_metadata = bundle
        .project
        .locales
        .get(&DEFAULT_LOCALE.to_string())
        .expect(&format!(
            "{} must be present in the project.yaml file",
            DEFAULT_LOCALE
        ));

    for (key, _value) in display_names {
        let locale_path = locales_folder_path.join(to_chrome_locale(key.to_string()));

        match std::fs::create_dir_all(&locale_path) {
            Ok(file) => file,
            Err(_) => return false,
        }

        let messages_path = locale_path.join("messages.json");
        let locale_metadata = bundle
            .project
            .locales
            .get(&key.to_string())
            .unwrap_or(default_locale_metadata);

        let mut locale_display_names: IndexMap<String, LocaleMessage> = IndexMap::new();
        for (layout_name, layout) in &bundle.layouts {
            if let Some(_target) = &layout.chrome_os {
                let display_name = layout.display_names.get(&key);

                if let Some(display_name) = display_name {
                    let temp = KEYBOARD_NAMES
                        .get(&key.to_string())
                        .cloned()
                        .or_else(|| KEYBOARD_NAMES.get("en").cloned())
                        .expect(&format!(
                            "no {} display name exists in {}.yaml",
                            DEFAULT_LOCALE, layout_name
                        ));
                    locale_display_names.insert(
                        to_chrome_locale(layout_name.to_string()),
                        LocaleMessage {
                            message: temp.replace("{}", display_name),
                        },
                    );
                }
            }
        }

        match std::fs::write(
            output_folder_path.join(messages_path),
            match serde_json::to_string_pretty(&Locale {
                locales: locale_display_names,
                name: LocaleMessage {
                    message: locale_metadata.name.clone(),
                },
                description: LocaleMessage {
                    message: locale_metadata.description.clone(),
                },
            }) {
                Ok(path) => path,
                Err(_) => return false,
            },
        ) {
            Ok(file) => file,
            Err(_) => return false,
        }
    }
    return true;
}

fn generate_dead_keys(chromeos_target: &ChromeOsTarget) -> IndexMap<ChromeOsKbdLayer, Vec<String>> {
    let mut json_dead_keys = IndexMap::new();
    if let Some(dead_keys) = &chromeos_target.dead_keys {
        for (dead_key_name, dead_key_list) in dead_keys {
            json_dead_keys.insert(dead_key_name.clone(), dead_key_list.to_vec());
        }
    }
    json_dead_keys
}

fn generate_transforms(
    bundle: &KbdgenBundle,
    layout: &Layout,
) -> IndexMap<String, IndexMap<String, String>> {
    if let Some(_layout_transforms) = &layout.transforms {
        let layout_file_path = bundle
            .path
            .join("layouts")
            .join(format!("{}.yaml", layout.language_tag));

        let yaml_file = std::fs::File::open(&layout_file_path).unwrap();
        let original_layout_yaml: serde_yaml::Value = serde_yaml::from_reader(yaml_file).unwrap();

        let owning_transform = *&original_layout_yaml.get("transforms").unwrap();

        return serde_yaml::from_value(owning_transform.clone()).unwrap();
    } else {
        return IndexMap::new();
    }
}

fn generate_layers(
    chromeos_target: &ChromeOsTarget,
    language_tag: LanguageTag,
) -> IndexMap<ChromeOsKbdLayer, IndexMap<String, String>> {
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
    json_layers
}

pub struct GenerateChromeOs;

#[async_trait(?Send)]
impl BuildStep for GenerateChromeOs {
    async fn build(&self, bundle: &KbdgenBundle, output_path: &Path) -> Result<()> {
        let mut descriptor = IndexMap::new();
        let mut manifest_input_components: Vec<ManifestInputComponent> = Vec::new();
        let mut display_name_map: IndexMap<LanguageTag, String> = IndexMap::new();
        // layout information is to be aggregated into a descriptor and then appended
        // to the end of the template
        for (language_tag, layout) in &bundle.layouts {
            if let Some(chromeos_target) = &layout.chrome_os {
                display_name_map.extend(layout.display_names.clone());

                let locale = chromeos_target
                    .config
                    .as_ref()
                    .and_then(|x| x.locale.as_ref().map(|x| x.clone()))
                    .unwrap_or_else(|| DEFAULT_LONG_LOCALE.parse().unwrap());

                let xkb_layout = chromeos_target
                    .config
                    .as_ref()
                    .and_then(|x| x.xkb_layout.clone())
                    .unwrap_or_else(|| DEFAULT_XKB_LAYOUT.to_string());

                let input_component = ManifestInputComponent::from_config(
                    language_tag.to_string(),
                    locale,
                    xkb_layout,
                );

                manifest_input_components.push(input_component);

                descriptor.insert(
                    language_tag.clone(),
                    ChromeOsDescriptor {
                        dead_keys: generate_dead_keys(chromeos_target),
                        transforms: generate_transforms(bundle, layout),
                        layers: generate_layers(chromeos_target, language_tag.clone()),
                    },
                );
            }
        }

        if create_background(descriptor, output_path) {
            if create_manifest(&bundle, manifest_input_components, output_path) {
                if !create_locales(&bundle, display_name_map, output_path) {
                    panic!("Could not generate locales")
                }
            } else {
                panic!("Could not generate manifest")
            }
        } else {
            panic!("Could not generate background.js")
        }

        Ok(())
    }
}
