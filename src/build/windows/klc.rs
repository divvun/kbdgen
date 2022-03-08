use core::fmt::Display;
use std::path::Path;
use std::sync::Arc;

use codecs::utf16::Utf16Ext;

use crate::build::BuildStep;
use crate::bundle::KbdgenBundle;

const KLC_EXT: &str = "klc";

pub struct KlcFile {
    keyboard_name: String,
}

impl Display for KlcFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("VERSION\t1.0\n\n")?;

        Ok(())
    }
}

pub struct GenerateKlc {}

impl BuildStep for GenerateKlc {
    fn build(&self, bundle: Arc<KbdgenBundle>, output_path: &Path) {

        // commenting due to format change
        /*
        let supported_patform = WindowsPlatformKey::Primary;

        let windows_layouts = bundle.layouts.iter().filter(|(_, layout)| {
            layout.layers.windows.as_ref().map_or(false, |platform| {
                platform.contains_key(&supported_patform)
            })
        });

        let klc_files = windows_layouts.map(|(language_tag, layout)| {
            KlcFile {
                keyboard_name: language_tag.to_string(),
            }
        });





        // need keymaps by mode

        klc_files.for_each(|klc_file| {
            // .klc files must be UTF-16 encoded
            let klc_bytes = klc_file.to_string().encode_utf16_le_bom();
            let klc_path = output_path.join(format!("{}.{}", klc_file.keyboard_name, KLC_EXT));
            std::fs::write(klc_path, klc_bytes).unwrap();
        });
        */
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
