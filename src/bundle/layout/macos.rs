use std::fmt;

use serde::{Deserialize, Serialize};

/// Declares [`MacOsKbdLayer`] from one table of variants and the names
/// layout files spell them with, so that the serde names and
/// [`MacOsKbdLayer::as_str`] cannot drift apart.
macro_rules! macos_layers {
    ($($variant:ident = $name:literal,)+) => {
        #[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
        pub enum MacOsKbdLayer {
            $(#[serde(rename = $name)] $variant,)+
        }

        impl MacOsKbdLayer {
            /// The layer's name as layout files spell it.
            pub fn as_str(self) -> &'static str {
                match self {
                    $(MacOsKbdLayer::$variant => $name,)+
                }
            }
        }
    };
}

macos_layers! {
    Default = "default",
    Shift = "shift",
    Caps = "caps",
    CapsAndShift = "caps+shift",
    Alt = "alt",
    AltAndShift = "alt+shift",
    AltAndCaps = "alt+caps",
    Ctrl = "ctrl",
    Cmd = "cmd",
    CmdAndShift = "cmd+shift",
    CmdAndAlt = "cmd+alt",
    CmdAndAltAndShift = "cmd+alt+shift",
}

impl fmt::Display for MacOsKbdLayer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_spelled_as_in_layout_files() {
        let parsed: MacOsKbdLayer = crate::yaml::from_str("caps+shift").unwrap();
        assert_eq!(parsed, MacOsKbdLayer::CapsAndShift);
        assert_eq!(parsed.to_string(), "caps+shift");
    }
}
