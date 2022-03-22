use indexmap::IndexMap;
use language_tags::LanguageTag;
use serde::{Deserialize, Deserializer, Serialize};
use serde_yaml::Value;

use transform::Transform;

use android::AndroidKbdLayerKey;
use chrome::ChromeKbdLayerKey;
use ios::IOsKbdLayerKey;
use macos::MacOsKbdLayerKey;
use windows::WindowsKbdLayerKey;

mod android;
mod chrome;
mod ios;
mod macos;
mod transform;
pub mod windows;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Layout {
    pub display_names: IndexMap<String, String>,

    pub windows: Option<WindowsTarget>,
    #[serde(rename = "chromeOS")]
    pub chrome_os: Option<ChromeOsTarget>,
    #[serde(rename = "macOS")]
    pub mac_os: Option<MacOsTarget>,
    #[serde(rename = "iOS")]
    pub i_os: Option<IOsTarget>,
    pub android: Option<AndroidTarget>,

    #[serde(default, deserialize_with = "from_mapped_sequence")]
    pub longpress: Option<IndexMap<String, Vec<String>>>,

    #[serde(default, deserialize_with = "from_nested_sequence")]
    pub transforms: Option<IndexMap<String, Transform>>,

    pub key_names: Option<KeyNames>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowsTarget {
    pub config: Option<WindowsConfig>,
    pub primary: WindowsPrimaryPlatform,
    pub dead_keys: Option<IndexMap<WindowsKbdLayerKey, Vec<String>>>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowsPrimaryPlatform {
    pub layers: IndexMap<WindowsKbdLayerKey, String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChromeOsTarget {
    pub config: ChromeConfig,
    pub primary: ChromeOsPrimaryPlatform,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChromeOsPrimaryPlatform {
    pub layers: IndexMap<ChromeKbdLayerKey, String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MacOsTarget {
    pub primary: MacOsPrimaryPlatform,
    pub dead_keys: Option<IndexMap<MacOsKbdLayerKey, Vec<String>>>,
    pub space: IndexMap<MacOsKbdLayerKey, String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MacOsPrimaryPlatform {
    pub layers: IndexMap<MacOsKbdLayerKey, String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct IOsTarget {
    pub config: IOsConfig,
    pub primary: Option<IOsPrimaryPlatform>,
    #[serde(rename = "iPad-9in")]
    pub i_pad_9in: Option<IOsIpad9InPlatform>,
    #[serde(rename = "iPad-12in")]
    pub i_pad_12in: Option<IOsIpad12InPlatform>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct IOsPrimaryPlatform {
    pub layers: IndexMap<IOsKbdLayerKey, String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct IOsIpad9InPlatform {
    pub layers: IndexMap<IOsKbdLayerKey, String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct IOsIpad12InPlatform {
    pub layers: IndexMap<IOsKbdLayerKey, String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AndroidTarget {
    pub config: AndroidConfig,
    pub primary: AndroidPrimaryPlatform,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AndroidPrimaryPlatform {
    pub layers: IndexMap<AndroidKbdLayerKey, String>,
}

fn from_mapped_sequence<'de, D>(
    deserializer: D,
) -> Result<Option<IndexMap<String, Vec<String>>>, D::Error>
where
    D: Deserializer<'de>,
{
    let map: IndexMap<String, String> = Deserialize::deserialize(deserializer)?;

    Ok(Some(
        map.into_iter()
            .map(|(key, value)| (key, value.split(" ").map(|s| s.to_owned()).collect()))
            .collect(),
    ))
}

fn from_nested_sequence<'de, D>(
    deserializer: D,
) -> Result<Option<IndexMap<String, Transform>>, D::Error>
where
    D: Deserializer<'de>,
{
    let mut output_map: IndexMap<String, Transform> = IndexMap::new();

    let transform_map: IndexMap<String, Value> = Deserialize::deserialize(deserializer)?;

    for (key, transform) in transform_map {
        let transform = process_transform(transform);

        output_map.insert(key.clone(), transform);
    }

    Ok(Some(output_map))
}

fn process_transform(value: Value) -> Transform {
    match value {
        Value::String(character) => Transform::End(character),
        Value::Mapping(mapping) => {
            let mut output_map: IndexMap<String, Transform> = IndexMap::new();

            for (map_key, map_value) in mapping {
                let key_character = match map_key {
                    Value::String(key_character) => key_character,
                    _ => panic!("Only Strings are supported within map transforms!"),
                };

                let inner_transform = process_transform(map_value);
                output_map.insert(key_character, inner_transform);
            }

            Transform::More(output_map)
        }
        _ => panic!("Only Strings and Maps are supported within transforms!"),
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct KeyNames {
    pub space: String,
    pub r#return: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WindowsConfig {
    pub locale: Option<LanguageTag>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChromeConfig {
    pub locale: Option<LanguageTag>,
    pub xkb_layout: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IOsConfig {
    pub speller_package_key: Option<String>,
    pub speller_path: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AndroidConfig {
    pub speller_package_key: Option<String>,
    pub speller_path: Option<String>,
}
