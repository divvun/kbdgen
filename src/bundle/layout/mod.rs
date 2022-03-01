use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

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
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Layers {
    windows: Option<IndexMap<WindowsPlatformKey, IndexMap<WindowsKbdLayerKey, String>>>,
    chrome: Option<IndexMap<ChromePlatformKey, IndexMap<ChromeKbdLayerKey, String>>>,
    macOS: Option<IndexMap<MacOSPlatformKey, IndexMap<MacOSKbdLayerKey, String>>>,
    #[serde(rename = "ios")]
    iOS: Option<IndexMap<iOSPlatformKey, IndexMap<iOSKbdLayerKey, String>>>,
    android: Option<IndexMap<AndroidPlatformKey, IndexMap<AndroidKbdLayerKey, String>>>,
}
