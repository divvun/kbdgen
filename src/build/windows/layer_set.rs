use indexmap::IndexMap;
use once_cell::sync::Lazy;
use regex::Regex;

use crate::bundle::layout::windows::WindowsKbdLayer;

use super::klc::key::validate_for_klc;

pub const SG_CAP: &str = "SGCap";

pub static UNICODE_ESCAPES: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\\u\{([0-9A-Fa-f]{1,6})\}").expect("valid regex"));

#[derive(Eq, PartialEq)]
pub struct WindowsLayerSetKey {
    pub string: String,
    pub dead_key: bool,
}

#[derive(Default)]
pub struct WindowsLayerSet {
    pub default: Option<WindowsLayerSetKey>,
    pub shift: Option<WindowsLayerSetKey>,
    pub caps: Option<WindowsLayerSetKey>,
    pub caps_and_shift: Option<WindowsLayerSetKey>,
    pub alt: Option<WindowsLayerSetKey>,
    pub alt_and_shift: Option<WindowsLayerSetKey>,
    pub alt_and_caps: Option<WindowsLayerSetKey>,
    pub ctrl: Option<WindowsLayerSetKey>,
}

impl WindowsLayerSet {
    pub fn caps_mode(&self) -> String {
        // Shift correspondence increases caps mode by 1
        // Alt correspondence increases caps mode by 4
        // We do not really know or understand why

        if !&self.caps.is_none() && &self.default != &self.caps && &self.shift != &self.caps {
            SG_CAP.to_owned()
        } else if self.caps.is_none() {
            let mut caps = 0;
            if &self.default != &self.shift {
                caps += 1;
            }
            if &self.alt != &self.alt_and_shift {
                caps += 4;
            }

            caps.to_string()
        } else {
            let mut caps = 0;
            if &self.caps == &self.shift {
                caps += 1;
            }
            if &self.alt_and_caps == &self.alt_and_shift {
                caps += 4;
            }

            caps.to_string()
        }
    }
}

pub fn populate_layer_set(
    layer_set: &mut WindowsLayerSet,
    layer: &WindowsKbdLayer,
    key_map: Vec<String>,
    cursor: usize,
    dead_keys: Option<&IndexMap<WindowsKbdLayer, Vec<String>>>,
) {
    match layer {
        WindowsKbdLayer::Default => {
            layer_set.default = process_key(&layer, &key_map[cursor], dead_keys);
        }
        WindowsKbdLayer::Shift => {
            layer_set.shift = process_key(&layer, &key_map[cursor], dead_keys);
        }
        WindowsKbdLayer::Caps => {
            layer_set.caps = process_key(&layer, &key_map[cursor], dead_keys);
        }
        WindowsKbdLayer::CapsAndShift => {
            layer_set.caps_and_shift = process_key(&layer, &key_map[cursor], dead_keys);
        }
        WindowsKbdLayer::Alt => {
            layer_set.alt = process_key(&layer, &key_map[cursor], dead_keys);
        }
        WindowsKbdLayer::AltAndShift => {
            layer_set.alt_and_shift = process_key(&layer, &key_map[cursor], dead_keys);
        }
        WindowsKbdLayer::AltAndCaps => {
            layer_set.alt_and_caps = process_key(&layer, &key_map[cursor], dead_keys);
        }
        WindowsKbdLayer::Ctrl => {
            layer_set.ctrl = process_key(&layer, &key_map[cursor], dead_keys);
        }
    };
}

fn process_key(
    layer_key: &WindowsKbdLayer,
    key: &str,
    dead_keys: Option<&IndexMap<WindowsKbdLayer, Vec<String>>>,
) -> Option<WindowsLayerSetKey> {
    if key == r"\u{0}" {
        return None;
    }

    validate_for_klc(key);

    let key = decode_unicode_escapes(key);

    let utf16s = key.encode_utf16().collect::<Vec<_>>();
    if utf16s.len() == 0 || utf16s[0] == 0 {
        tracing::error!("Empty key: {:?}", key);
        return None;
    } else if utf16s.len() > 4 {
        tracing::error!("Input key too long: {:?}", key);
        return None;
    }

    let mut dead_key: bool = false;

    if let Some(dead_keys) = dead_keys {
        if let Some(layer_dead_keys) = dead_keys.get(layer_key) {
            if layer_dead_keys.contains(&key.to_string()) {
                dead_key = true;
            }
        }
    }

    Some(WindowsLayerSetKey {
        string: key.to_owned(),
        dead_key,
    })
}

fn decode_unicode_escapes(input: &str) -> String {
    let new = UNICODE_ESCAPES.replace_all(input, |hex: &regex::Captures| {
        let number = u32::from_str_radix(hex.get(1).unwrap().as_str(), 16).unwrap_or(0xfeff);
        std::char::from_u32(number).unwrap().to_string()
    });

    new.to_string()
}
