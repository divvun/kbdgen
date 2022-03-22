use std::fmt::Display;

use super::key::KlcKey;

pub struct KlcDeadKeys {
    pub keys: Vec<KlcKey>,
}

impl Display for KlcDeadKeys {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.keys.is_empty() {
            return Ok(());
        }

        f.write_str("DEADKEY\n\n")?;

        f.write_str("\n")?;

        Ok(())
    }
}
