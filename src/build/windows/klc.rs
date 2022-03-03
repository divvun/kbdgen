use core::fmt::Display;

pub struct KlcFile {

}

impl Display for KlcFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {

        f.write_str("VERSION\t1.0\n\n")?;

        Ok(())
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
