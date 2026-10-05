//! LDML escapes in v4 strings (`ldml.yaml.escape`). Decoding is LDML's
//! own (`kbd_ldml::escape`); the one v4 difference is that a `\u` not
//! followed by `{`, which LDML reads as literal text, is an error.

use kbd_ldml::escape::{Piece, decode_plain, decode_text};

use super::error::{At, Result};

/// Which escapes a string's syntax has besides LDML's `\u{…}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Syntax {
    /// `key@output` and similar text values, where `\` escapes nothing
    /// else.
    Text,
    /// `transform@from`, `@to`, `uset` and `reorder` values, where `\\` is
    /// a backslash.
    Regex,
}

// [spec:kbdgen:syn:ldml.yaml.escape]
/// Fails on a `\u` that is not followed by `{`.
pub fn check_escapes(s: &str, syntax: Syntax, at: &At) -> Result<()> {
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while let Some(c) = chars.get(i) {
        if *c == '\\' {
            match chars.get(i + 1) {
                Some('u') if chars.get(i + 2) != Some(&'{') => {
                    return Err(at.error(format!(
                        "\\u at character {} is not followed by {{; write \\u{{…}}",
                        i + 1
                    )));
                }
                Some('\\') if syntax == Syntax::Regex => i += 1,
                _ => {}
            }
        }
        i += 1;
    }
    Ok(())
}

/// The string variables of a layout, decoded in definition order; a value
/// may use the strings defined before it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Strings(Vec<(String, Vec<Piece>)>);

impl Strings {
    pub fn get(&self, id: &str) -> Option<Vec<Piece>> {
        self.0.iter().find(|(k, _)| k == id).map(|(_, v)| v.clone())
    }

    pub fn define(&mut self, id: &str, value: &str, at: &At) -> Result<()> {
        let pieces = output(value, self, at)?;
        self.0.push((id.to_string(), pieces));
        Ok(())
    }
}

// [spec:kbdgen:syn:ldml.yaml.escape]
/// Decodes a value in `key@output` syntax: `\u{…}`, `\m{…}` and `${…}`.
pub fn output(s: &str, strings: &Strings, at: &At) -> Result<Vec<Piece>> {
    check_escapes(s, Syntax::Text, at)?;
    decode_text(s, &|id| strings.get(id)).map_err(|e| at.error(e.to_string()))
}

// [spec:kbdgen:syn:ldml.yaml.escape]
/// Decodes a value that may hold `\u{…}` but no markers or variables:
/// dead-key strings, displays and labels.
pub fn plain(s: &str, at: &At) -> Result<String> {
    check_escapes(s, Syntax::Text, at)?;
    decode_plain(s).map_err(|e| at.error(e.to_string()))
}

/// The text of `pieces` when it holds no marker.
pub fn plain_text(pieces: &[Piece]) -> Option<String> {
    pieces
        .iter()
        .map(|p| match p {
            Piece::Char(c) => Some(*c),
            Piece::Marker(_) => None,
        })
        .collect()
}

/// Scalar values as `XXXX` uppercase hex of at least four digits, joined
/// by `sep`: the spelling of generated key ids and markers.
pub fn hex_name(chars: impl Iterator<Item = char>, sep: &str) -> String {
    chars
        .map(|c| format!("{:04X}", u32::from(c)))
        .collect::<Vec<_>>()
        .join(sep)
}
