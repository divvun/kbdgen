use std::fmt::Display;

pub struct KlcDeadKeys {
    pub characters: Vec<char>,
}

impl Display for KlcDeadKeys {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.characters.is_empty() {
            return Ok(());
        }

        for dead_key in &self.characters {
            f.write_fmt(format_args!("DEADKEY {:04x}\n\n", *dead_key as u32))?;
        }

        f.write_str("\n")?;

        Ok(())
    }
}
