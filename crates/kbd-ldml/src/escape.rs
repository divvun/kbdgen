//! LDML's escapes in text-valued attributes (`ldml.xml.escape`) and their
//! inverse for generated attributes (`ldml.xml.export.escape`).
//!
//! A text value is `key@output`, `display@output`, `display@display`,
//! `displayOptions@baseCharacter`, or a `string` or `set` item. In it:
//!
//! - `\u{h h …}` names scalar values; a malformed one is an error
//! - `\m{name}` is a marker
//! - `${id}` inserts a string variable
//! - any other `\` or `$` is literal, so a `\u` without `{` is literal
//!
//! The pattern and replacement syntaxes of `transform` live in
//! [`crate::syntax`]; they share [`hex_escape`].

use std::iter::Peekable;
use std::str::Chars;

use kbd_model::is_nmtoken;

use crate::gencat::needs_escape;

/// A decoded text element: a scalar value or a named marker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Piece {
    Char(char),
    Marker(String),
}

/// A syntax error at character offset `offset` of the value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxError {
    pub offset: usize,
    pub message: String,
}

impl SyntaxError {
    pub fn new(offset: usize, message: impl Into<String>) -> Self {
        SyntaxError {
            offset,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for SyntaxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} (at character {})", self.message, self.offset)
    }
}

/// A character cursor that counts the characters it has consumed.
pub(crate) struct Cursor<'a> {
    chars: Peekable<Chars<'a>>,
    pub(crate) offset: usize,
}

impl<'a> Cursor<'a> {
    pub(crate) fn new(s: &'a str) -> Self {
        Cursor {
            chars: s.chars().peekable(),
            offset: 0,
        }
    }

    pub(crate) fn peek(&mut self) -> Option<char> {
        self.chars.peek().copied()
    }

    pub(crate) fn next(&mut self) -> Option<char> {
        let c = self.chars.next();
        if c.is_some() {
            self.offset += 1;
        }
        c
    }

    pub(crate) fn eat(&mut self, c: char) -> bool {
        if self.peek() == Some(c) {
            self.next();
            true
        } else {
            false
        }
    }

    /// Whether the remaining input starts with `prefix`.
    pub(crate) fn starts_with(&self, prefix: &str) -> bool {
        let mut rest = self.chars.clone();
        prefix.chars().all(|p| rest.next() == Some(p))
    }

    /// Consumes up to, and including, the next `}`, returning what came
    /// before it.
    pub(crate) fn until_brace(&mut self, start: usize, what: &str) -> Result<String, SyntaxError> {
        let mut out = String::new();
        loop {
            match self.next() {
                Some('}') => return Ok(out),
                Some(c) => out.push(c),
                None => return Err(SyntaxError::new(start, format!("unterminated {what}"))),
            }
        }
    }
}

// [spec:kbdgen:syn:ldml.xml.escape+1]
/// Decodes the body of `\u{…}`: one or more space-separated groups of 1–6
/// hex digits, each a Unicode scalar value. `start` is the offset of the
/// backslash, for errors.
pub fn hex_escape(body: &str, start: usize) -> Result<Vec<char>, SyntaxError> {
    if body.trim().is_empty() {
        return Err(SyntaxError::new(start, "empty \\u{} escape"));
    }
    let mut out = Vec::new();
    for group in body.split(' ') {
        if group.is_empty() || group.len() > 6 || !group.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(SyntaxError::new(
                start,
                format!("\\u{{{body}}} is not 1-6 hex digits per scalar value"),
            ));
        }
        let value = u32::from_str_radix(group, 16)
            .map_err(|_| SyntaxError::new(start, format!("bad hex in \\u{{{body}}}")))?;
        let c = char::from_u32(value).ok_or_else(|| {
            SyntaxError::new(
                start,
                format!("\\u{{{group}}} is a surrogate or beyond U+10FFFF"),
            )
        })?;
        out.push(c);
    }
    Ok(out)
}

/// Parses the body of `\m{…}` outside patterns: an NMTOKEN, never `.`.
pub(crate) fn marker_name(body: String, start: usize) -> Result<String, SyntaxError> {
    if body == "." {
        return Err(SyntaxError::new(
            start,
            "\\m{.} matches any marker and is only valid in transform@from",
        ));
    }
    if !is_nmtoken(&body) {
        return Err(SyntaxError::new(
            start,
            format!("marker name {body:?} is not an NMTOKEN"),
        ));
    }
    Ok(body)
}

// [spec:kbdgen:syn:ldml.xml.escape+1]
/// Decodes a text value. `string` returns the decoded value of a string
/// variable, or none if it is not defined.
pub fn decode_text(
    value: &str,
    string: &dyn Fn(&str) -> Option<Vec<Piece>>,
) -> Result<Vec<Piece>, SyntaxError> {
    let mut cur = Cursor::new(value);
    let mut out = Vec::new();
    while let Some(c) = cur.peek() {
        let start = cur.offset;
        if c == '\\' && cur.starts_with("\\u{") {
            cur.next();
            cur.next();
            cur.next();
            let body = cur.until_brace(start, "\\u{ escape")?;
            out.extend(hex_escape(&body, start)?.into_iter().map(Piece::Char));
        } else if c == '\\' && cur.starts_with("\\m{") {
            cur.next();
            cur.next();
            cur.next();
            let body = cur.until_brace(start, "\\m{ marker")?;
            out.push(Piece::Marker(marker_name(body, start)?));
        } else if c == '$' && cur.starts_with("${") {
            cur.next();
            cur.next();
            let id = cur.until_brace(start, "${ variable")?;
            let pieces = string(&id).ok_or_else(|| {
                SyntaxError::new(start, format!("string variable {id:?} is not defined"))
            })?;
            out.extend(pieces);
        } else {
            cur.next();
            out.push(Piece::Char(c));
        }
    }
    Ok(out)
}

/// Decodes a text value that may hold neither markers nor variables, such
/// as a display string's escapes.
pub fn decode_plain(value: &str) -> Result<String, SyntaxError> {
    let pieces = decode_text(value, &|_| None)?;
    pieces
        .into_iter()
        .map(|p| match p {
            Piece::Char(c) => Ok(c),
            Piece::Marker(m) => Err(SyntaxError::new(0, format!("unexpected marker \\m{{{m}}}"))),
        })
        .collect()
}

/// Writes `c` as `\u{XXXX}`: uppercase hex, at least four digits.
pub(crate) fn push_hex(out: &mut String, c: char) {
    out.push_str(&format!("\\u{{{:04X}}}", u32::from(c)));
}

// [spec:kbdgen:req:ldml.xml.export.escape]
/// Encodes a text value for `output`, `display` and the like: characters
/// of general category M, Cc, Cf or Z other than U+0020 become `\u{…}`,
/// as do a `\` that would start an escape and a `$` before `{` or `[`.
pub fn encode_text(pieces: &[Piece]) -> String {
    let mut out = String::new();
    let mut iter = pieces.iter().peekable();
    while let Some(piece) = iter.next() {
        match piece {
            Piece::Marker(m) => {
                out.push_str("\\m{");
                out.push_str(m);
                out.push('}');
            }
            Piece::Char(c) => {
                let next = match iter.peek() {
                    Some(Piece::Char(n)) => Some(*n),
                    _ => None,
                };
                let starts_escape = match c {
                    '\\' => matches!(next, Some('u' | 'm')),
                    '$' => matches!(next, Some('{' | '[')),
                    _ => false,
                };
                if starts_escape || (*c != ' ' && needs_escape(*c)) {
                    push_hex(&mut out, *c);
                } else {
                    out.push(*c);
                }
            }
        }
    }
    out
}

/// Encodes a plain string as a text value.
pub fn encode_plain(s: &str) -> String {
    let pieces: Vec<Piece> = s.chars().map(Piece::Char).collect();
    encode_text(&pieces)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode(s: &str) -> Result<Vec<Piece>, SyntaxError> {
        decode_text(s, &|id| {
            (id == "v").then(|| vec![Piece::Char('x'), Piece::Marker("m".into())])
        })
    }

    fn chars(s: &str) -> Vec<Piece> {
        s.chars().map(Piece::Char).collect()
    }

    // [spec:kbdgen:syn:ldml.xml.escape+1/test]
    #[test]
    fn decodes_escapes_markers_and_variables() {
        assert_eq!(decode("a\\u{301 62}").unwrap(), chars("a\u{301}b"));
        assert_eq!(decode("\\u{1F600}").unwrap(), chars("😀"));
        assert_eq!(
            decode("\\m{dk_00B4}${v}").unwrap(),
            vec![
                Piece::Marker("dk_00B4".into()),
                Piece::Char('x'),
                Piece::Marker("m".into())
            ]
        );
        assert_eq!(decode("\\u0300 $x \\n").unwrap(), chars("\\u0300 $x \\n"));
    }

    // [spec:kbdgen:syn:ldml.xml.escape+1/test]
    #[test]
    fn rejects_malformed_escapes() {
        for bad in [
            "\\u{}",
            "\\u{D800}",
            "\\u{110000}",
            "\\u{1234567}",
            "\\u{12",
            "\\u{  }",
            "\\u{g}",
            "\\m{.}",
            "\\m{a b}",
            "${nope}",
        ] {
            assert!(decode(bad).is_err(), "{bad}");
        }
    }

    // [spec:kbdgen:req:ldml.xml.export.escape/test]
    #[test]
    fn encodes_marks_controls_and_escape_starts() {
        assert_eq!(encode_plain("a\u{301} b\u{A0}"), "a\\u{0301} b\\u{00A0}");
        assert_eq!(encode_plain("\\u{41}"), "\\u{005C}u{41}");
        assert_eq!(encode_plain("\\x$5${"), "\\x$5\\u{0024}{");
        assert_eq!(encode_plain("\u{200C}\t"), "\\u{200C}\\u{0009}");
        for s in ["a\u{301}", "\\u{41}", "$[x]", "\u{9CD}\u{200C}", "\\ $"] {
            assert_eq!(decode_plain(&encode_plain(s)).unwrap(), s);
        }
    }
}
