use std::fmt::Display;

use super::key::KlcKey;

pub struct KlcLayout {
    pub rows: Vec<KlcLayoutRow>,
}

impl Display for KlcLayout {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Hardcoded columns. .klc can support a different number
        // of columns or column assignments than this
        // but we don't support that.
        f.write_str("SHIFTSTATE\n\n")?;
        f.write_str("0 // 4\n")?;
        f.write_str("1 // 5 Shift\n")?;
        f.write_str("2 // 6 Ctrl\n")?;
        f.write_str("6 // 7 Alt\n")?;
        f.write_str("7 // 8 Alt + Shift\n\n")?;

        f.write_str("LAYOUT\n\n")?;

        for row in &self.rows {
            f.write_str(&row.to_string())?;
        }

        f.write_str("\n")?;

        Ok(())
    }
}

#[derive(Copy, Clone)]
#[repr(u8)]
pub enum KlcLayer {
    Default,
    Shift,
    Ctrl,
    Alt,
    AltAndShift,
}

// Hardcoded columns. .klc can support a different number
// of columns or column assignments than this
// but we don't support that.
pub struct KlcLayoutRow {
    pub scancode: String,
    pub virtual_key: String,
    pub caps_mode: String,
    pub default_key: KlcKey,
    pub shift_key: KlcKey,
    pub ctrl_key: KlcKey,
    pub alt_key: KlcKey,
    pub alt_and_shift_key: KlcKey,
}

impl Display for KlcLayoutRow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            &self.scancode,
            &self.virtual_key,
            &self.caps_mode,
            &self.default_key,
            &self.shift_key,
            &self.ctrl_key,
            &self.alt_key,
            &self.alt_and_shift_key,
        ))?;

        Ok(())
    }
}
