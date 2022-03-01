use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use windows::WindowsPlatform;

mod windows;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Layout {
    pub display_names: IndexMap<String, String>,

    pub layers: Layers,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Layers {
    #[serde(rename = "ios")]
    iOS: Option<IndexMap<iOSPlatformKey, IndexMap<iOSKbdLayerKey, String>>>,
    android: Option<IndexMap<AndroidPlatformKey, IndexMap<AndroidKbdLayerKey, String>>>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum iOSPlatformKey {
    iOSss,
    #[serde(rename = "ipad-9in")]
    iPad9in,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum AndroidPlatformKey {
    Android,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum iOSKbdLayerKey {
    Default,
    Shift,
    Alt,
    #[serde(rename = "alt+shift")]
    AltAndShift,
    #[serde(rename = "symbols-1")] 
    Symbols1,
    #[serde(rename = "symbols-2")]
    Symbols2,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum AndroidKbdLayerKey {
    Default,
    Shift,
}

/*
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum TargetKey {
    iOS,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum TargetPlatform {
    iOS(iOSPlatformKey),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum iOSPlatformValue {
    iOSiOSssLayer(iOSiOSssLayer),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct iOSiOSssLayer {
    default: String,
    shift: String,
    #[serde(rename = "symbols-1")]
    symbols1: String,
    #[serde(rename = "symbols-2")]
    symbols2: String,
}
*/

/*
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum TargetValue {
    #[serde(flatten)]
    iOS(String),
}*/

/*
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum LayerTargetKey {
    iOS,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LayerTargetValue {
    //Windows(IndexMap<WindowsPlatformKey, String>),
    iOSss(IndexMap<iOSPlatformKey, iOSTestEnum>),
    //Android(IndexMap<AndroidPlatformKey, String>),
}
*/


/*

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum iOSTestEnum {
    iOSss,
    iPad9In,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum WindowsPlatformKey {
    Default,
    Shift,
    Caps,
    #[serde(rename = "caps+shift")]
    CapsAndShift,
    Alt,
    #[serde(rename = "alt+shift")]
    AltAndShift,
    Ctrl,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum iOSPlatformKey {
    #[serde(flatten)]
    iOSss,
    #[serde(flatten)]
    iPad9In,
    /*
    Default,
    Shift,
    #[serde(rename = "symbols-1")] 
    Symbols1,
    #[serde(rename = "symbols-2")]
    Symbols2,
    */
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum AndroidPlatformKey {
    Default,
    Shift,
    #[serde(rename = "symbols-1")] 
    Symbols1,
    #[serde(rename = "symbols-2")]
    Symbols2,
}
*/