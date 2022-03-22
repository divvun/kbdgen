use std::path::Path;
use std::sync::Arc;

use codecs::utf16::Utf16Ext;
use indexmap::IndexMap;

use super::dead_key::KlcDeadKeys;
use super::file::KlcFile;
use super::key::KlcKey;
use super::keymap::MSKLC_KEYS;
use super::layout::{KlcLayout, KlcLayoutRow};
use super::ligature::{KlcLigature, KlcLigatureRow};

use crate::build::BuildStep;
use crate::bundle::layout::windows::WindowsKbdLayerKey;
use crate::bundle::KbdgenBundle;

const KLC_EXT: &str = "klc";

#[derive(Copy, Clone)]
#[repr(u8)]
pub enum LayerColumn {
    Default,
    Shift,
    Ctrl,
    Alt,
    AltAndShift,
}

pub struct GenerateKlc {}

impl BuildStep for GenerateKlc {
    fn build(&self, bundle: Arc<KbdgenBundle>, output_path: &Path) {
        // One .klc file per language with Windows primary platform
        for (language_tag, layout) in &bundle.layouts {
            if let Some(windows_layout) = &layout.windows {
                let layers = &windows_layout.primary.layers;

                let mut klc_layout_rows = Vec::new();
                let mut klc_ligature_rows = Vec::new();
                let mut klc_dead_keys = Vec::new();

                let mut cursor = 0;
                for (_iso_key, klc_key) in MSKLC_KEYS.iter() {
                    let mut layout_set = WindowsLayoutSet::default();

                    // Layout set to determine caps_mode and null keys
                    for (layer_key, key_map) in layers {
                        populate_layout_set(
                            windows_layout.dead_keys.as_ref(),
                            &mut layout_set,
                            layer_key,
                            &key_map,
                            cursor,
                        );
                    }

                    klc_layout_rows.push(KlcLayoutRow {
                        scancode: klc_key.scancode.clone(),
                        virtual_key: klc_key.virtual_key.clone(),
                        caps_mode: layout_set.caps_mode(),
                        default_key: convert_to_klc_key(
                            layout_set.default,
                            &mut klc_ligature_rows,
                            &klc_key.virtual_key,
                            LayerColumn::Default,
                        ),
                        shift_key: convert_to_klc_key(
                            layout_set.shift,
                            &mut klc_ligature_rows,
                            &klc_key.virtual_key,
                            LayerColumn::Shift,
                        ),
                        ctrl_key: convert_to_klc_key(
                            layout_set.ctrl,
                            &mut klc_ligature_rows,
                            &klc_key.virtual_key,
                            LayerColumn::Ctrl,
                        ),
                        alt_key: convert_to_klc_key(
                            layout_set.alt,
                            &mut klc_ligature_rows,
                            &klc_key.virtual_key,
                            LayerColumn::Alt,
                        ),
                        alt_and_shift_key: convert_to_klc_key(
                            layout_set.alt_and_shift,
                            &mut klc_ligature_rows,
                            &klc_key.virtual_key,
                            LayerColumn::AltAndShift,
                        ),
                    });

                    cursor += 1;
                }

                let klc_file = KlcFile {
                    keyboard_name: language_tag.to_string(),
                    copyright: bundle.project.copyright.clone(),
                    company: bundle.project.organisation.clone(),
                    layout: KlcLayout {
                        rows: klc_layout_rows,
                    },
                    ligature: KlcLigature {
                        rows: klc_ligature_rows,
                    },
                    dead_keys: KlcDeadKeys {
                        keys: klc_dead_keys,
                    },
                };

                let klc_bytes = klc_file.to_string().encode_utf16_le_bom();
                let klc_path = output_path.join(format!("{}.{}", klc_file.keyboard_name, KLC_EXT));
                std::fs::write(klc_path, klc_bytes).unwrap();
            }
        }
    }
}

#[derive(Eq, PartialEq)]
pub struct WindowsLayoutKey {
    pub string: String,
    pub dead_key: bool,
}

#[derive(Default)]
pub struct WindowsLayoutSet {
    pub default: Option<WindowsLayoutKey>,
    pub shift: Option<WindowsLayoutKey>,
    pub caps: Option<WindowsLayoutKey>,
    pub caps_and_shift: Option<WindowsLayoutKey>,
    pub alt: Option<WindowsLayoutKey>,
    pub alt_and_shift: Option<WindowsLayoutKey>,
    pub ctrl: Option<WindowsLayoutKey>,
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
    dead_keys: Option<&IndexMap<WindowsKbdLayerKey, Vec<String>>>,
    layout_set: &mut WindowsLayoutSet,
    layer_key: &WindowsKbdLayerKey,
    key_map: &str,
    cursor: usize,
) {
    let key_map: Vec<String> = split_keys(&key_map);

    match layer_key {
        WindowsKbdLayerKey::Default => {
            layout_set.default = process_key(&layer_key, &key_map[cursor], dead_keys);
        }
        WindowsKbdLayerKey::Shift => {
            layout_set.shift = process_key(&layer_key, &key_map[cursor], dead_keys);
        }
        WindowsKbdLayerKey::Caps => {
            layout_set.caps = process_key(&layer_key, &key_map[cursor], dead_keys);
        }
        WindowsKbdLayerKey::CapsAndShift => {
            layout_set.caps_and_shift = process_key(&layer_key, &key_map[cursor], dead_keys);
        }
        WindowsKbdLayerKey::Alt => {
            layout_set.alt = process_key(&layer_key, &key_map[cursor], dead_keys);
        }
        WindowsKbdLayerKey::AltAndShift => {
            layout_set.alt_and_shift = process_key(&layer_key, &key_map[cursor], dead_keys);
        }
        WindowsKbdLayerKey::Ctrl => {
            layout_set.ctrl = process_key(&layer_key, &key_map[cursor], dead_keys);
        }
    };
}

fn split_keys(layer: &str) -> Vec<String> {
    layer.split_whitespace().map(|v| v.to_string()).collect()
}

fn process_key(
    layer_key: &WindowsKbdLayerKey,
    key: &str,
    dead_keys: Option<&IndexMap<WindowsKbdLayerKey, Vec<String>>>,
) -> Option<WindowsLayoutKey> {
    if key == r"\u{0}" {
        return None;
    }

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

    Some(WindowsLayoutKey {
        string: key.to_owned(),
        dead_key,
    })
}

fn convert_to_klc_key(
    key: Option<WindowsLayoutKey>,
    klc_ligature_rows: &mut Vec<KlcLigatureRow>,
    virtual_key: &str,
    layer_column: LayerColumn,
) -> KlcKey {
    match key {
        Some(key) => {
            let utf16s: Vec<u16> = key.string.encode_utf16().collect::<Vec<_>>();

            if utf16s.len() == 1 {
                let character = key.string.chars().next().unwrap();

                if key.dead_key {
                    let dead_key = KlcKey::DeadKey(character);

                    // add to list

                    dead_key
                } else {
                    KlcKey::Character(character)
                }
            } else {
                let ligature_row = KlcLigatureRow {
                    virtual_key: virtual_key.to_owned(),
                    shift_state: (layer_column as u8).to_string(),
                    utf16s,
                };

                klc_ligature_rows.push(ligature_row);

                KlcKey::Ligature
            }
        }
        None => KlcKey::None,
    }
}
