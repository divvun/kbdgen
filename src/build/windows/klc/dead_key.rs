use std::fmt::Display;

pub struct KlcDeadKey {}

impl Display for KlcDeadKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DEADKEY\n\n")?;

        f.write_str("\n")?;

        Ok(())
    }
}
