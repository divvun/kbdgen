//! Strings between the formats: v3 escape decoding (`keys.escape`), the
//! rewrite of a `\u` without braces (M06), and the v4 spelling of a
//! decoded string, which keeps the author's spelling whenever v4 reads it
//! as the same text (`ldml.yaml.escape`, `ldml.yaml.tokens`).

use kbd_ldml::escape::{Piece, decode_plain, decode_text};

use crate::util::UNICODE_ESCAPES;

/// Decodes `\u{H}` escapes as v3 does: anywhere in the string, with any
/// other text verbatim. An escape outside the Unicode scalar values, which
/// v3 panics on, is an error naming it.
pub fn decode_v3(raw: &str) -> Result<String, String> {
    let mut out = String::with_capacity(raw.len());
    let mut last = 0;
    for captures in UNICODE_ESCAPES.captures_iter(raw) {
        let (Some(whole), Some(hex)) = (captures.get(0), captures.get(1)) else {
            continue;
        };
        let value = u32::from_str_radix(hex.as_str(), 16).unwrap_or(u32::MAX);
        let c = char::from_u32(value)
            .ok_or_else(|| format!("\\u{{{}}} is not a Unicode scalar value", hex.as_str()))?;
        out.push_str(&raw[last..whole.start()]);
        out.push(c);
        last = whole.end();
    }
    out.push_str(&raw[last..]);
    Ok(out)
}

/// `raw` with each `\u` that is not followed by `{` rewritten (M06): the
/// up to four hex digits after it become `\u{XXXX}`, and a `\u` with no
/// hex digits gets its backslash escaped. `None` when there is none.
pub fn braced(raw: &str) -> Option<String> {
    let chars: Vec<char> = raw.chars().collect();
    let mut out = String::new();
    let mut changed = false;
    let mut i = 0;
    while let Some(c) = chars.get(i) {
        if *c == '\\' && chars.get(i + 1) == Some(&'u') && chars.get(i + 2) != Some(&'{') {
            let hex: String = chars
                .iter()
                .skip(i + 2)
                .take(4)
                .take_while(|c| c.is_ascii_hexdigit())
                .collect();
            changed = true;
            if hex.is_empty() {
                out.push_str("\\u{5C}u");
                i += 2;
            } else {
                out.push_str(&format!("\\u{{{}}}", hex.to_ascii_uppercase()));
                i += 2 + hex.chars().count();
            }
            continue;
        }
        out.push(*c);
        i += 1;
    }
    changed.then_some(out)
}

/// Whether YAML can hold `c` as written in any scalar: its printable
/// characters, without the byte order mark.
pub fn printable(c: char) -> bool {
    matches!(u32::from(c), 0x20..=0x7E | 0xA0..=0xD7FF | 0xE000..=0xFFFD | 0x10000..)
        && c != '\u{FEFF}'
}

/// Whether a row token may hold `c` as written: printable and not a blank,
/// so rows stay split on ASCII spaces alone.
fn token_char(c: char) -> bool {
    printable(c) && !c.is_whitespace()
}

fn push_hex(out: &mut String, c: char) {
    out.push_str(&format!("\\u{{{:X}}}", u32::from(c)));
}

/// `text` spelled with `\u{…}` for every backslash, dollar sign and
/// character `keep` refuses, which v4 reads back as `text` in every
/// syntax.
fn spelled(text: &str, keep: fn(char) -> bool) -> String {
    let mut out = String::new();
    for c in text.chars() {
        if c == '\\' || c == '$' || !keep(c) {
            push_hex(&mut out, c);
        } else {
            out.push(c);
        }
    }
    out
}

fn chars_of(pieces: &[Piece]) -> Option<String> {
    pieces
        .iter()
        .map(|p| match p {
            Piece::Char(c) => Some(*c),
            Piece::Marker(_) => None,
        })
        .collect()
}

/// Whether v4 reads `s` as a `key@output` text (`ldml.yaml.escape`) equal
/// to `text`, with no marker and no variable.
fn reads_as_output(s: &str, text: &str) -> bool {
    braced(s).is_none()
        && decode_text(s, &|_| None)
            .ok()
            .and_then(|p| chars_of(&p))
            .is_some_and(|d| d == text)
}

/// Whether v4 reads `s` as a plain string equal to `text`.
fn reads_as_plain(s: &str, text: &str) -> bool {
    braced(s).is_none() && decode_plain(s).is_ok_and(|d| d == text)
}

/// The v4 spelling of a plain string (dead-key identities, compose inputs
/// and outputs, labels): `raw` when v4 reads it as `text` and YAML can
/// hold it, else an escaped spelling.
pub fn plain(raw: &str, text: &str) -> String {
    if raw.chars().all(printable) && reads_as_plain(raw, text) {
        raw.to_string()
    } else {
        spelled(text, printable)
    }
}

/// The v4 spelling of an output (`decimal`, `longPress` outputs).
pub fn output(raw: &str, text: &str) -> String {
    if raw.chars().all(printable) && reads_as_output(raw, text) {
        raw.to_string()
    } else {
        spelled(text, printable)
    }
}

/// The v4 spelling of a literal row token whose output is `text`
/// (`ldml.yaml.tokens`): `raw` when it reads as that literal and no
/// earlier token form claims it, else an escaped spelling.
pub fn token(raw: &str, text: &str) -> String {
    let claimed = ["\\s{", "\\d{", "\\k{", "\\l{"]
        .iter()
        .any(|p| raw.starts_with(p));
    if !raw.is_empty() && !claimed && raw.chars().all(token_char) && reads_as_output(raw, text) {
        raw.to_string()
    } else {
        spelled(text, token_char)
    }
}

pub fn dead_token(identity: &str) -> String {
    let body = spelled_inside_token(identity);
    format!("\\d{{{body}}}")
}

/// An identity spelling made safe inside a token: blanks escaped.
fn spelled_inside_token(identity: &str) -> String {
    let mut out = String::new();
    for c in identity.chars() {
        if token_char(c) {
            out.push(c);
        } else {
            push_hex(&mut out, c);
        }
    }
    out
}
