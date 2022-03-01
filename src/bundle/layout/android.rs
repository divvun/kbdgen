use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum AndroidPlatformKey {
    Primary,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum AndroidKbdLayerKey {
    Default,
    Shift,
}
