use serde::{Deserialize, Serialize};

#[allow(non_camel_case_types)]
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum iOSPlatformKey {
    Primary,
    #[serde(rename = "ipad-9in")]
    iPad9in,
    #[serde(rename = "ipad-12in")]
    iPad12in,
}

#[allow(non_camel_case_types)]
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
