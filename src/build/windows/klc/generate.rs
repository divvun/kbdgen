use std::path::Path;
use std::sync::Arc;

use codecs::utf16::Utf16Ext;

use super::file::{KlcFile, KlcLayout, KlcRow};
use super::key::KlcKey;
use super::keymap::MSKLC_KEYS;

use crate::build::BuildStep;
use crate::bundle::layout::windows::WindowsKbdLayerKey;
use crate::bundle::KbdgenBundle;

const KLC_EXT: &str = "klc";

pub struct GenerateKlc {}

impl BuildStep for GenerateKlc {
    fn build(&self, bundle: Arc<KbdgenBundle>, output_path: &Path) {
        // One .klc file per language with Windows primary platform
        for (language_tag, layout) in &bundle.layouts {
            if let Some(windows_layout) = &layout.windows {
                let layers = &windows_layout.primary.layers;

                let mut klc_rows = Vec::new();

                let mut cursor = 0;
                for (_iso_key, klc_key) in MSKLC_KEYS.iter() {
                    let mut layout_set = WindowsLayoutSet::default();

                    for (layer_key, key_map) in layers {
                        populate_layout_set(&mut layout_set, layer_key, &key_map, cursor);
                    }

                    klc_rows.push(KlcRow {
                        scancode: klc_key.scancode.clone(),
                        virtual_key: klc_key.virtual_key.clone(),
                        caps_mode: layout_set.caps_mode(),
                        default_key: convert_to_klc_key(layout_set.default),
                        shift_key: convert_to_klc_key(layout_set.shift),
                    });

                    cursor = cursor + 1;
                }

                let klc_file = KlcFile {
                    keyboard_name: language_tag.to_string(),
                    copyright: bundle.project.copyright.clone(),
                    company: bundle.project.organisation.clone(),
                    layout: KlcLayout { rows: klc_rows },
                };

                let klc_bytes = klc_file.to_string().encode_utf16_le_bom();
                let klc_path = output_path.join(format!("{}.{}", klc_file.keyboard_name, KLC_EXT));
                std::fs::write(klc_path, klc_bytes).unwrap();
            }
        }
    }
}

#[derive(Default)]
pub struct WindowsLayoutSet {
    pub default: Option<String>,
    pub shift: Option<String>,
    pub caps: Option<String>,
    pub caps_and_shift: Option<String>,
    pub alt: Option<String>,
    pub alt_and_shift: Option<String>,
    pub ctrl: Option<String>,
}

impl WindowsLayoutSet {
    fn caps_mode(&self) -> i8 {
        let mut caps_mode: i8 = 0;

        -1
    }
}

fn populate_layout_set(
    layout_set: &mut WindowsLayoutSet,
    layer_key: &WindowsKbdLayerKey,
    key_map: &str,
    cursor: usize,
) {
    let key_map: Vec<String> = split_keys(&key_map);

    match layer_key {
        WindowsKbdLayerKey::Default => {
            layout_set.default = process_key(&key_map[cursor]);
        }
        WindowsKbdLayerKey::Shift => {
            layout_set.shift = process_key(&key_map[cursor]);
        }
        WindowsKbdLayerKey::Caps => {
            layout_set.caps = process_key(&key_map[cursor]);
        }
        WindowsKbdLayerKey::CapsAndShift => {
            layout_set.caps_and_shift = process_key(&key_map[cursor]);
        }
        WindowsKbdLayerKey::Alt => {
            layout_set.alt = process_key(&key_map[cursor]);
        }
        WindowsKbdLayerKey::AltAndShift => {
            layout_set.alt_and_shift = process_key(&key_map[cursor]);
        }
        WindowsKbdLayerKey::Ctrl => {
            layout_set.ctrl = process_key(&key_map[cursor]);
        }
    };
}

fn split_keys(layer: &str) -> Vec<String> {
    layer.split_whitespace().map(|v| v.to_string()).collect()
}

fn process_key(key: &str) -> Option<String> {
    let utf16s: Vec<u16> = key.encode_utf16().collect::<Vec<_>>();

    if utf16s.len() == 0 || utf16s[0] == 0 {
        return None;
    } else if utf16s.len() > 4 {
        log::error!("Input key too long: {:?}", key);
        return None;
    }

    None
}

fn convert_to_klc_key(key: Option<String>) -> KlcKey {
    match key {
        Some(key) => {
            let utf16s: Vec<u16> = key.encode_utf16().collect::<Vec<_>>();

            if utf16s.len() == 1 {
                let character = key.chars().next().unwrap();
                KlcKey::Character(character)
            } else {
                KlcKey::None
            }
        }
        None => KlcKey::None,
    }
}
