use std::path::Path;

use async_trait::async_trait;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::to_string_pretty;

use crate::{build::BuildStep, bundle::KbdgenBundle};

#[derive(Serialize, Deserialize)]
pub struct IosInfo {
    name: String,
    locale: String,
    enter: String,
    space: String,
}

// #[derive(Serialize, Deserialize)]
// pub struct PlatformDeadKeys;

// #[derive(Serialize, Deserialize)]
// pub struct IosDeadKeys {
//     #[serde(flatten)]
//     dead_keys: IndexMap<String, PlatformDeadKeys>
// }

#[derive(Serialize, Deserialize)]
pub struct IosKeyboardDefinitions {
    #[serde(flatten)]
    info: IosInfo,
    longpress: IndexMap<String, Vec<String>>,
    // #[serde(rename = "camelCase")]
    // dead_keys: IosDeadKeys,
    // transforms: ????
}

pub struct GenerateIos;

#[async_trait(?Send)]
impl BuildStep for GenerateIos {
    async fn build(&self, bundle: &KbdgenBundle, output_path: &Path) {
        let keyboard_definitions_file_path = output_path.join("KeyboardDefinitions.json");

        let info = IosInfo {
            name: "test".to_string(),
            locale: "te".to_string(),
            enter: "Enter".to_string(),
            space: "mellanslag".to_string(),
        };

        let mut longpress: IndexMap<String, Vec<String>> = IndexMap::new();
        // let mut dead_keys_list: IndexMap<String, PlatformDeadKeys> = IndexMap::new();
        for (language_tag, layout) in &bundle.layouts {
            if let Some(ios_target) = &layout.i_os {
                if let Some(layout_longpress) = &layout.longpress {
                    for (key, value) in layout_longpress {
                        longpress.insert(key.clone(), value.clone());
                    }
                }
            }
        }

        std::fs::write(
            output_path.join(keyboard_definitions_file_path),
            serde_json::to_string_pretty(&[IosKeyboardDefinitions {
                info: info,
                longpress: longpress,
                // dead_keys: IosDeadKeys { dead_keys: dead_keys_list },
            }])
            .unwrap(),
        )
        .unwrap();
    }
}
