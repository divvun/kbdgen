use std::{fmt, path::Path};

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

        // layout information is to be aggregated into a descriptor and then appended
        // to the end of the template
        for (language_tag, layout) in &bundle.layouts {
            if let Some(chromeos_target) = &layout.chrome_os {
                if let Some(layout_transforms) = &layout.transforms {
                    let layout_file_path = bundle
                        .path
                        .join("layouts")
                        .join(format!("{}.yaml", layout.language_tag));

                    let yaml_file = std::fs::File::open(&layout_file_path).unwrap();
                    let original_layout_yaml: serde_yaml::Value =
                        serde_yaml::from_reader(yaml_file).unwrap();

                    let owning_transform = *&original_layout_yaml.get("transforms").unwrap();

                    let json_transforms: IndexMap<String, IndexMap<String, String>> =
                        serde_yaml::from_value(owning_transform.clone()).unwrap();

                    descriptor.insert(
                        language_tag.clone(),
                        ChromeOsDescriptor {
                            dead_keys: IndexMap::new(),
                            transforms: json_transforms,
                        },
                    );
                }
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
