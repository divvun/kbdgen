use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum MacOsKbdLayer {
    Default,
    Shift,
    Caps,
    #[serde(rename = "caps+shift")]
    CapsAndShift,
    Alt,
    #[serde(rename = "alt+shift")]
    AltAndShift,
    #[serde(rename = "alt+caps")]
    AltAndCaps,
    Ctrl,
    Cmd,
    #[serde(rename = "cmd+shift")]
    CmdAndShift,
    #[serde(rename = "cmd+alt")]
    CmdAndAlt,
    #[serde(rename = "cmd+alt+shift")]
    CmdAndAltAndShift,
}
