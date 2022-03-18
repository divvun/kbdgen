use std::path::Path;
use std::sync::Arc;

use codecs::utf16::Utf16Ext;

use super::file::KlcFile;
use super::key::KlcKey;
use super::keymap::MSKLC_KEYS;
use super::layout::{KlcLayout, KlcLayoutRow};
use super::ligature::KlcLigature;

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

                    // Layout set to determine caps_mode and null keys
                    for (layer_key, key_map) in layers {
                        populate_layout_set(&mut layout_set, layer_key, &key_map, cursor);
                    }

                    klc_rows.push(KlcLayoutRow {
                        scancode: klc_key.scancode.clone(),
                        virtual_key: klc_key.virtual_key.clone(),
                        caps_mode: layout_set.caps_mode(),
                        default_key: convert_to_klc_key(layout_set.default),
                        shift_key: convert_to_klc_key(layout_set.shift),
                        ctrl_key: convert_to_klc_key(layout_set.ctrl),
                        alt_key: convert_to_klc_key(layout_set.alt),
                        alt_and_shift_key: convert_to_klc_key(layout_set.alt_and_shift),
                    });

                    cursor += 1;
                }

                let klc_file = KlcFile {
                    keyboard_name: language_tag.to_string(),
                    copyright: bundle.project.copyright.clone(),
                    company: bundle.project.organisation.clone(),
                    layout: KlcLayout { rows: klc_rows },
                    ligature: KlcLigature {},
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
    fn caps_mode(&self) -> String {
        // Shift correspondence increases caps mode by 1
        // Alt correspondence increases caps mode by 4
        // We do not really know or understand why

        if !&self.caps.is_none() && &self.default != &self.caps && &self.shift != &self.caps {
            "SGCap".to_owned()
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
            //if &self.alt_caps == &self.alt_shift {
            //    caps += 4;
            //}
            // TODO: add alt_and_caps if that's another layer

            caps.to_string()
        }
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
    println!("processing key: {}", key);

    let utf16s = key.encode_utf16().collect::<Vec<_>>();

    if utf16s.len() == 0 || utf16s[0] == 0 {
        println!("Null key1");
        return None;
    } else if utf16s.len() > 4 {
        println!("More than 4 UTF-16s");
        log::error!("Input key too long: {:?}", key);
        return None;
    }

    Some(key.to_owned())
}

fn convert_to_klc_key(key: Option<String>) -> KlcKey {
    match key {
        Some(key) => {
            let utf16s: Vec<u16> = key.encode_utf16().collect::<Vec<_>>();

            if utf16s.len() == 1 {
                let character = key.chars().next().unwrap();
                KlcKey::Character(character)
            } else {
                println!("Ligature");
                KlcKey::None
            }
        }
        None => {
            println!("Null key2");
            KlcKey::None
        }
    }
}
