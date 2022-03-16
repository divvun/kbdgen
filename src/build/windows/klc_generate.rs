use std::path::Path;
use std::sync::Arc;

use codecs::utf16::Utf16Ext;

use super::klc_file::{KlcFile, KlcRow};
use super::klc_keymap::MSKLC_KEYS;

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

                // Testing
                let default_layer: Vec<String> = layers
                    .get(&WindowsKbdLayerKey::Default)
                    .unwrap()
                    .split_whitespace()
                    .map(|v| v.to_string())
                    .collect();
                let shift_layer: Vec<String> = layers
                    .get(&WindowsKbdLayerKey::Shift)
                    .unwrap()
                    .split_whitespace()
                    .map(|v| v.to_string())
                    .collect();

                let mut default_klc_keys = Vec::new();
                let mut cursor = 0;
                for (_iso_key, klc_key) in MSKLC_KEYS.iter() {
                    if cursor >= default_layer.len() {
                        print!("keymap less than iso map");
                        break;
                    }

                    default_klc_keys.push(KlcRow {
                        scancode: klc_key.scancode.clone(),
                        virtual_key: klc_key.virtual_key.clone(),
                        cap_mode: "9".to_owned(), // TODO: UUHHH
                        default_character: default_layer[cursor].clone(),
                        shift_character: shift_layer[cursor].clone(),
                    });

                    cursor = cursor + 1;
                }

                let klc_file = KlcFile {
                    keyboard_name: language_tag.to_string(),
                    copyright: bundle.project.copyright.clone(),
                    company: bundle.project.organisation.clone(),
                    rows: default_klc_keys,
                };

                let klc_bytes = klc_file.to_string().encode_utf16_le_bom();
                let klc_path = output_path.join(format!("{}.{}", klc_file.keyboard_name, KLC_EXT));
                std::fs::write(klc_path, klc_bytes).unwrap();
            }
        }
    }
}
