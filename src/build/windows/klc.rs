use core::fmt::Display;
use std::path::Path;
use std::sync::Arc;

use codecs::utf16::Utf16Ext;

use super::klc_keymap::MSKLC_KEYS;

use crate::build::BuildStep;
use crate::bundle::layout::windows::WindowsKbdLayerKey;
use crate::bundle::KbdgenBundle;

const KLC_EXT: &str = "klc";

pub struct KlcFile {
    keyboard_name: String,
    copyright: String,
    rows: Vec<KlcRow>,
}

impl Display for KlcFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("VERSION\t1.0\n\n")?;
        f.write_fmt(format_args!("COPYRIGHT\t\"{}\"\n\n", self.copyright))?;

        for row in &self.rows {
            f.write_str(&row.to_string())?;
        }

        Ok(())
    }
}

pub struct KlcRow {
    pub scancode: String,
    pub virtual_key: String,
    pub cap_mode: String,
    pub default_character: String,
    pub shift_character: String,
}

impl Display for KlcRow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "{}\t{}\t{}\t{}\t{}\n",
            &self.scancode,
            &self.virtual_key,
            &self.cap_mode,
            &self.default_character,
            &self.shift_character
        ))?;

        Ok(())
    }
}

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
                    rows: default_klc_keys,
                };

                let klc_bytes = klc_file.to_string().encode_utf16_le_bom();
                let klc_path = output_path.join(format!("{}.{}", klc_file.keyboard_name, KLC_EXT));
                std::fs::write(klc_path, klc_bytes).unwrap();
            }
        }
    }
}

// sections

// 1. Preliminary text (keyboard name, copyright, etc.)

// 2. Shiftstate (??)

// 3. 0-1-2-6-7

// 4. LAYOUT
// - columns here. Involves ligatures

// 5. LIGATURE

// 6. DEADKEY

// 7. Random numbers? or these more DEADKEY?

// 8. KEYNAME because keyboard loves you

// 9.
