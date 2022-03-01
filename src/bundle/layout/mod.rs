use indexmap::IndexMap;
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
mod windows;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Layout {
    pub display_names: IndexMap<String, String>,

    pub layers: Layers,

    #[serde(default, deserialize_with = "from_mapped_sequence")]
    pub longpress: Option<IndexMap<String, Vec<String>>>,

    pub space: Option<Space>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Layers {
    windows: Option<IndexMap<WindowsPlatformKey, IndexMap<WindowsKbdLayerKey, String>>>,
    chrome: Option<IndexMap<ChromePlatformKey, IndexMap<ChromeKbdLayerKey, String>>>,
    #[serde(rename = "macos")]
    macOS: Option<IndexMap<MacOSPlatformKey, IndexMap<MacOSKbdLayerKey, String>>>,
    #[serde(rename = "ios")]
    iOS: Option<IndexMap<iOSPlatformKey, IndexMap<iOSKbdLayerKey, String>>>,
    android: Option<IndexMap<AndroidPlatformKey, IndexMap<AndroidKbdLayerKey, String>>>,
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

#[derive(Debug, Serialize, Deserialize)]
pub struct Space {
    #[serde(rename = "macos")]
    macOS: Option<IndexMap<MacOSKbdLayerKey, String>>,
}
