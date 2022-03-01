use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use android::{AndroidKbdLayerKey, AndroidPlatformKey};
use ios::{iOSKbdLayerKey, iOSPlatformKey};
use windows::{WindowsKbdLayerKey, WindowsPlatformKey};

mod android;
mod ios;
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
    #[serde(rename = "ios")]
    iOS: Option<IndexMap<iOSPlatformKey, IndexMap<iOSKbdLayerKey, String>>>,
    android: Option<IndexMap<AndroidPlatformKey, IndexMap<AndroidKbdLayerKey, String>>>,
}
