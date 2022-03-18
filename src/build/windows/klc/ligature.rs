use std::fmt::Display;

pub struct KlcLigature {}

impl Display for KlcLigature {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("LIGATURE\n\n")?;

        Ok(())
    }
}
