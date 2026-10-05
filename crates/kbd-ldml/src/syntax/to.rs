//! `transform@to`: §Transform To Grammar.

use kbd_model::is_nmtoken;

use super::is_variable_id;
use crate::escape::{Cursor, Piece, SyntaxError, hex_escape, push_hex};
use crate::gencat::needs_escape;

/// An element of a replacement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToItem {
    Char(char),
    Marker(String),
    /// `$0`–`$9`.
    Group(u8),
    /// `$[n:id]`
    MapSet {
        group: u8,
        set: String,
    },
}

// [spec:kbdgen:syn:ldml.xml.to]
/// Parses a `to` value. `string` returns the decoded value of a string
/// variable, which `${id}` inserts with its markers. Whether the groups and
/// mapped sets agree with `from` is checked against the pattern.
pub fn parse_replacement(
    value: &str,
    string: &dyn Fn(&str) -> Option<Vec<Piece>>,
) -> Result<Vec<ToItem>, SyntaxError> {
    let mut cur = Cursor::new(value);
    let mut out = Vec::new();
    while let Some(c) = cur.next() {
        let start = cur.offset - 1;
        match c {
            '\\' => match cur.next() {
                Some(e @ ('\\' | '$')) => out.push(ToItem::Char(e)),
                Some(e @ ('u' | 'm')) if cur.eat('{') => {
                    let body = cur.until_brace(start, "escape")?;
                    if e == 'u' {
                        out.extend(hex_escape(&body, start)?.into_iter().map(ToItem::Char));
                    } else if body != "." && is_nmtoken(&body) {
                        out.push(ToItem::Marker(body));
                    } else {
                        return Err(SyntaxError::new(
                            start,
                            format!("\\m{{{body}}} is not a marker name"),
                        ));
                    }
                }
                Some(e) => {
                    return Err(SyntaxError::new(
                        start,
                        format!("\\{e} is not an escape of transform@to; write \\\\ for \\"),
                    ));
                }
                None => return Err(SyntaxError::new(start, "trailing backslash")),
            },
            '$' => match cur.next() {
                Some('$') => out.push(ToItem::Char('$')),
                Some(d @ '0'..='9') => out.push(ToItem::Group(d as u8 - b'0')),
                Some('{') => {
                    let id = cur.until_brace(start, "${ variable")?;
                    let pieces = string(&id).ok_or_else(|| {
                        SyntaxError::new(start, format!("string variable {id:?} is not defined"))
                    })?;
                    out.extend(pieces.into_iter().map(|p| match p {
                        Piece::Char(c) => ToItem::Char(c),
                        Piece::Marker(m) => ToItem::Marker(m),
                    }));
                }
                Some('[') => {
                    let mut body = String::new();
                    loop {
                        match cur.next() {
                            Some(']') => break,
                            Some(c) => body.push(c),
                            None => return Err(SyntaxError::new(start, "unterminated $[")),
                        }
                    }
                    let mapped = body.split_once(':').and_then(|(n, id)| {
                        let group = match n.as_bytes() {
                            [d @ b'1'..=b'9'] => d - b'0',
                            _ => return None,
                        };
                        is_variable_id(id).then(|| ToItem::MapSet {
                            group,
                            set: id.to_string(),
                        })
                    });
                    out.push(mapped.ok_or_else(|| {
                        SyntaxError::new(start, format!("$[{body}] is not a mapped set $[n:set]"))
                    })?);
                }
                _ => {
                    return Err(SyntaxError::new(
                        start,
                        "$ starts $$, $n, ${string} or $[n:set]",
                    ));
                }
            },
            c => out.push(ToItem::Char(c)),
        }
    }
    Ok(out)
}

// [spec:kbdgen:req:ldml.xml.export.escape]
/// Writes a replacement in `to` syntax: `$` and `\` as `$$` and `\\`, and
/// characters of general category M, Cc, Cf or Z other than U+0020 as
/// `\u{…}` (`ldml.xml.export.escape`).
pub fn encode_replacement(items: &[ToItem]) -> String {
    let mut out = String::new();
    for item in items {
        match item {
            ToItem::Char('$') => out.push_str("$$"),
            ToItem::Char('\\') => out.push_str("\\\\"),
            ToItem::Char(c) if *c != ' ' && needs_escape(*c) => push_hex(&mut out, *c),
            ToItem::Char(c) => out.push(*c),
            ToItem::Marker(m) => out.push_str(&format!("\\m{{{m}}}")),
            ToItem::Group(n) => out.push_str(&format!("${n}")),
            ToItem::MapSet { group, set } => out.push_str(&format!("$[{group}:{set}]")),
        }
    }
    out
}
