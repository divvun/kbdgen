use std::fmt::Display;

use indexmap::IndexMap;

use crate::bundle::layout::Transform;

const TRANSFORM_ESCAPE: &str = " ";

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
                                        let next_16s = next_char.encode_utf16().collect::<Vec<_>>();
                                        let end_16s = end_char.encode_utf16().collect::<Vec<_>>();

                                        if next_16s.len() > 1 || end_16s.len() > 1 {
                                            tracing::error!(
                                                "Key or value of transform too long: {} {}",
                                                next_char,
                                                end_char
                                            );
                                            continue;
                                        }

                                        f.write_fmt(format_args!(
                                            "{:04x}\t{:04x}\n",
                                            next_16s[0], end_16s[0]
                                        ))?;
                                    }
                                    Transform::More(_transform) => {
                                        todo!("Recursion required ahead");
                                    }
                                }
                            }
                        }
                    };
                } else {
                    tracing::error!("No transforms for dead key {}", dead_key);
                }

                f.write_str("\n")?;
            }

            // TODO: default transform for each dead key
            /*
            let default = transforms
                .get(" ")
                .and_then(|x| x.encode_utf16().nth(0))
                .unwrap_or_else(|| dk.into_inner().to_string().encode_utf16().nth(0).unwrap());
            f.write_fmt(format_args!("0020\t{:04x}\n\n", default))?;
            */

            Ok(())
        } else {
            tracing::error!("Dead Keys present but no transforms");

            Ok(())
        }
    }
}
