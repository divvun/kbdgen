use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Hash, Clone)]
#[serde(rename_all = "lowercase")]
pub enum ChromeOsKbdLayer {
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
