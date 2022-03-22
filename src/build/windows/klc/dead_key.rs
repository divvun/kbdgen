use std::fmt::Display;

pub struct KlcDeadKey {
    pub rows: Vec<KlcDeadKeyRow>,
}

impl Display for KlcDeadKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.rows.is_empty() {
            return Ok(());
        }

        f.write_str("DEADKEY\n\n")?;

        f.write_str("\n")?;

        Ok(())
    }
}

pub struct KlcDeadKeyRow {
    pub dead_key: String,
}
