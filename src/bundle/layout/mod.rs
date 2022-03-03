use indexmap::IndexMap;
use language_tags::LanguageTag;
use serde::{Deserialize, Deserializer, Serialize};

use android::{AndroidKbdLayerKey, AndroidPlatformKey};
use chrome::{ChromeKbdLayerKey, ChromePlatformKey};
use ios::{iOSKbdLayerKey, iOSPlatformKey};
use macos::{MacOSKbdLayerKey, MacOSPlatformKey};
use windows::{WindowsKbdLayerKey, WindowsPlatformKey};

mod android;
mod chrome;
mod ios;
mod macos;
pub mod windows;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Layout {
    pub display_names: IndexMap<String, String>,

    pub layers: Layers,

    #[serde(default, deserialize_with = "from_mapped_sequence")]
    pub longpress: Option<IndexMap<String, Vec<String>>>,

    pub space: Option<Space>,

    pub dead_keys: Option<DeadKeys>,

    pub key_names: Option<KeyNames>,

    pub targets: Option<Targets>,
}

#[allow(non_snake_case)]
#[derive(Debug, Serialize, Deserialize)]
pub struct Layers {
    pub windows: Option<IndexMap<WindowsPlatformKey, IndexMap<WindowsKbdLayerKey, String>>>,

    pub chrome: Option<IndexMap<ChromePlatformKey, IndexMap<ChromeKbdLayerKey, String>>>,

    #[serde(rename = "macos")]
    pub macOS: Option<IndexMap<MacOSPlatformKey, IndexMap<MacOSKbdLayerKey, String>>>,

    #[serde(rename = "ios")]
    pub iOS: Option<IndexMap<iOSPlatformKey, IndexMap<iOSKbdLayerKey, String>>>,

    pub android: Option<IndexMap<AndroidPlatformKey, IndexMap<AndroidKbdLayerKey, String>>>,
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

#[allow(non_snake_case)]
#[derive(Debug, Serialize, Deserialize)]
pub struct Space {
    #[serde(rename = "macos")]
    macOS: Option<IndexMap<MacOSKbdLayerKey, String>>,
}

#[allow(non_snake_case)]
#[derive(Debug, Serialize, Deserialize)]
pub struct DeadKeys {
    #[serde(rename = "macos")]
    macOS: Option<IndexMap<MacOSKbdLayerKey, Vec<String>>>,

    windows: Option<IndexMap<WindowsKbdLayerKey, Vec<String>>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct KeyNames {
    space: String,
    r#return: String,
}

#[allow(non_snake_case)]
#[derive(Debug, Serialize, Deserialize)]
pub struct Targets {
    windows: Option<WindowsTargetConfig>,
    chrome: Option<ChromeTargetConfig>,
    iOS: Option<iOSTargetConfig>,
    android: Option<AndroidTargetConfig>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WindowsTargetConfig {
    locale: Option<LanguageTag>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChromeTargetConfig {
    locale: Option<LanguageTag>,
    xkb_layout: Option<String>,
}

#[allow(non_camel_case_types)]
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct iOSTargetConfig {
    speller_package_key: Option<String>,
    speller_path: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AndroidTargetConfig {
    speller_package_key: Option<String>,
    speller_path: Option<String>,
}
