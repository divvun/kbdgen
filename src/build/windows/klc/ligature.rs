use std::fmt::Display;

pub struct KlcLigature {
    pub rows: Vec<KlcLigatureRow>,
}

impl Display for KlcLigature {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.rows.is_empty() {
            return Ok(());
        }

        f.write_str("LIGATURE\n\n")?;

        for ligature in &self.rows {
            f.write_fmt(format_args!(
                "{}\t{}",
                ligature.virtual_key, ligature.shift_state
            ))?;
            for utf16 in &ligature.utf16s {
                f.write_fmt(format_args!("\t{:04x}", utf16))?;
            }
            f.write_str("\n")?;
        }

        f.write_str("\n")?;

        Ok(())
    }
}

pub struct KlcLigatureRow {
    pub virtual_key: String,
    pub shift_state: String,
    pub utf16s: Vec<u16>,
}
