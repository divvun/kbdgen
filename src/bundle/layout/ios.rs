use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Hash, Clone)]
#[serde(rename_all = "lowercase")]
pub enum IOsKbdLayer {
    Default,
    Shift,
    Caps,
    Alt,
    #[serde(rename = "alt+shift")]
    AltAndShift,
    #[serde(rename = "symbols-1")]
    Symbols1,
    #[serde(rename = "symbols-2")]
    Symbols2,
}
