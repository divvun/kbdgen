use core::fmt::Display;
use std::path::Path;
use std::sync::Arc;

use codecs::utf16::Utf16Ext;

use crate::build::BuildStep;
use crate::bundle::KbdgenBundle;

const KLC_EXT: &str = "klc";

pub struct KlcFile {}

impl Display for KlcFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("VERSION\t1.0\n\n")?;

        Ok(())
    }
}

pub struct GenerateKlc {}

impl BuildStep for GenerateKlc {
    fn build(&self, bundle: Arc<KbdgenBundle>, output_path: &Path) {
        // TODO: Create a .klc file for every layer
        // TODO: in the windows platforms
        let klc_file = KlcFile {};

        // .klc files must be UTF-16 encoded
        let klc_bytes = klc_file.to_string().encode_utf16_le_bom();

        let klc_path = output_path.join(format!("test.{}", KLC_EXT));
        std::fs::write(klc_path, klc_bytes).unwrap();
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
