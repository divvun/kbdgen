use std::fmt::Display;

use indexmap::IndexMap;

use crate::{bundle::layout::Transform, util::TRANSFORM_ESCAPE};

use super::key::validate_for_klc;

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
                f.write_fmt(format_args!("DEADKEY {:04x}\n\n", *dead_key as u32))?;

                if let Some(transform) = transforms.get(&dead_key.to_string()) {
                    match transform {
                        Transform::End(_character) => {
                            tracing::error!("Transform ended too soon for dead key {}", dead_key);
                        }
                        Transform::More(map) => {
                            for (next_char, transform) in map {
                                if next_char == TRANSFORM_ESCAPE {
                                    continue;
                                }

                                match transform {
                                    Transform::End(end_char) => {
                                        write_transform(next_char, end_char, f)?;
                                    }
                                    Transform::More(_transform) => {
                                        todo!("Recursion required ahead");
                                    }
                                }
                            }

                            let transform = map.get(TRANSFORM_ESCAPE).expect(&format!(
                                "The escape transform `{}` not found for dead key `{}`",
                                TRANSFORM_ESCAPE, &dead_key
                            ));

                            match transform {
                                Transform::End(end_char) => {
                                    write_transform(TRANSFORM_ESCAPE, end_char, f)?;
                                }
                                Transform::More(_transform) => {
                                    panic!("The escape transform should be a string, not another transform");
                                }
                            };
                        }
                    };
                } else {
                    tracing::error!("No transforms for dead key {}", dead_key);
                }

                f.write_str("\n")?;
            }

            Ok(())
        } else {
            tracing::error!("Dead Keys present but no transforms");

            Ok(())
        }
    }
}

fn write_transform(from: &str, to: &str, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    validate_for_klc(from);
    validate_for_klc(to);

    let from_16s = from.encode_utf16().collect::<Vec<_>>();
    let to_16s = to.encode_utf16().collect::<Vec<_>>();

    if from_16s.len() > 1 || to_16s.len() > 1 {
        tracing::error!(
            "Key or value of transform too long: {} {}, skipping",
            from,
            to
        );

        return Ok(());
    }

    f.write_fmt(format_args!("{:04x}\t{:04x}\n", from_16s[0], to_16s[0]))
}
