use std::fmt::{Display, Write};

pub enum KlcKey {
    Character(char),
    Ligature,
    None,
}

impl Display for KlcKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            // ASCII characters can be represented as is
            // Unicode characters must be converted to their UTF-16 representation
            KlcKey::Character(character) => {
                if character.is_ascii_graphic() {
                    f.write_char(*character)
                } else {
                    f.write_fmt(format_args!("{:04x}", *character as u32))
                }
            }
            &KlcKey::Ligature => f.write_str("%%"),
            KlcKey::None => f.write_str("-1"),
        }
    }
}
