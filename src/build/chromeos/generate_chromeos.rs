use std::path::Path;

use async_trait::async_trait;
use indexmap::IndexMap;
use pahkat_client::types::repo::Index;
use serde::{Deserialize, Serialize};

use crate::{
    build::BuildStep,
    bundle::{layout::Transform, KbdgenBundle},
};

#[derive(Debug, Serialize, Deserialize)]
pub struct ChromeOsLayout {
    pub transforms: IndexMap<String, IndexMap<String, String>>,
}

pub struct GenerateChromeOs;

#[async_trait(?Send)]
impl BuildStep for GenerateChromeOs {
    async fn build(&self, bundle: &KbdgenBundle, output_path: &Path) {
        // One .klc file per language with Windows primary platform
        for (language_tag, layout) in &bundle.layouts {
            if let Some(chromeos_target) = &layout.chrome_os {
                let chrome_layout;

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

                    chrome_layout = ChromeOsLayout {
                        transforms: json_transforms,
                    };
                } else {
                    chrome_layout = ChromeOsLayout {
                        transforms: IndexMap::new(),
                    };
                }

                let serde_chrome_layout = serde_json::to_string_pretty(&chrome_layout).unwrap();

                let temp_file_path = Path::new("temp.json");

                std::fs::write(output_path.join(temp_file_path), serde_chrome_layout).unwrap();
            }
        }
    }
}
