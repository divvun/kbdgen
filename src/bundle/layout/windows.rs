use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum WindowsPlatformKey {
    Primary,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum WindowsKbdLayerKey {
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
