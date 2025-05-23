use std::fmt::{Display, Write};

const EXCEEDS_BMP: char = '\u{FFFF}';

pub enum KlcKey {
    Character(char),
    DeadKey(char),
    Ligature,
    None,
    Skip,
}

impl Display for KlcKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KlcKey::Character(character) => display_klc_character(*character, f),
            KlcKey::DeadKey(character) => {
                display_klc_character(*character, f)?;
                f.write_char('@')
            }
            KlcKey::Ligature => f.write_str("%%"),
            KlcKey::None => f.write_str("-1"),
            KlcKey::Skip => Ok(()),
        }
    }
}

// ASCII characters can be represented as is
// Unicode characters must be converted to their UTF-16 representation
pub fn display_klc_character(character: char, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    if character.is_ascii_graphic() {
        f.write_char(character)
    } else {
        f.write_fmt(format_args!("{:04x}", character as u32))
    }
}

pub fn validate_for_klc(key: &str) {
    key.chars().for_each(|character| {
        if character > EXCEEDS_BMP {
            panic!("Unrepresentable key detected! {}", key);
        }
    });
}
