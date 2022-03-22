use std::fmt::Display;

use indexmap::IndexMap;

use crate::bundle::layout::transform::Transform;

pub struct KlcDeadKeys<'a> {
    pub characters: Vec<char>,
    pub transforms: &'a Option<IndexMap<String, Transform>>,
}

impl Display for KlcDeadKeys<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.characters.is_empty() {
            return Ok(());
        }

        if let Some(transforms) = self.transforms {
            for dead_key in &self.characters {
                if let Some(transform) = transforms.get(&dead_key.to_string()) {
                } else {
                    tracing::error!("No transforms for dead key {}", dead_key);
                }

                f.write_fmt(format_args!("DEADKEY {:04x}\n\n", *dead_key as u32))?;
            }

            f.write_str("\n")?;

            Ok(())
        } else {
            tracing::error!("Dead Keys present but no transforms");

            Ok(())
        }
    }
}
