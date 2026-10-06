//! `reorder@from` and `@before`: strings of elements that each match one
//! character (§Element: reorder), and the per-element value lists.

use kbd_model::ClassRange;

use super::{FROM_METACHARS, Uset, parse_uset};
use crate::escape::{Cursor, SyntaxError, hex_escape, push_hex};
use crate::gencat::needs_escape;
use crate::syntax::encode_uset;

/// One element of a reorder string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReorderElem {
    Char(char),
    /// A bracketed UnicodeSet or a `$[uset]` reference, normalized.
    Ranges(Vec<ClassRange>),
}

/// Parses a reorder string whose `${…}` references are already
/// substituted. Markers are an error: reorder never matches them.
pub fn parse_reorder(
    value: &str,
    uset: &dyn Fn(&str) -> Option<Uset>,
) -> Result<Vec<ReorderElem>, SyntaxError> {
    let mut cur = Cursor::new(value);
    let mut out = Vec::new();
    while let Some(c) = cur.peek() {
        let start = cur.offset;
        match c {
            '[' => {
                let set = take_bracketed(&mut cur)?;
                let set = parse_uset(&set, uset).map_err(|e| shift(e, start))?;
                out.push(ReorderElem::Ranges(set.ranges));
            }
            '$' if cur.starts_with("$[") => {
                let mut reference = String::new();
                while let Some(c) = cur.next() {
                    reference.push(c);
                    if c == ']' {
                        break;
                    }
                }
                let set = parse_uset(&reference, uset).map_err(|e| shift(e, start))?;
                out.push(ReorderElem::Ranges(set.ranges));
            }
            '\\' => {
                cur.next();
                match cur.next() {
                    Some('u') if cur.eat('{') => {
                        let body = cur.until_brace(start, "\\u{ escape")?;
                        out.extend(hex_escape(&body, start)?.into_iter().map(ReorderElem::Char));
                    }
                    Some('m') => {
                        return Err(SyntaxError::new(start, "reorder does not match markers"));
                    }
                    Some(e) if FROM_METACHARS.contains(e) || e == '-' => {
                        out.push(ReorderElem::Char(e));
                    }
                    Some(e) => {
                        return Err(SyntaxError::new(
                            start,
                            format!("\\{e} is not an allowed escape"),
                        ));
                    }
                    None => return Err(SyntaxError::new(start, "trailing backslash")),
                }
            }
            c => {
                cur.next();
                out.push(ReorderElem::Char(c));
            }
        }
    }
    Ok(out)
}

fn shift(mut e: SyntaxError, by: usize) -> SyntaxError {
    e.offset += by;
    e
}

/// The text of a bracketed set, nested brackets and escapes included.
fn take_bracketed(cur: &mut Cursor) -> Result<String, SyntaxError> {
    let start = cur.offset;
    let mut depth = 0usize;
    let mut out = String::new();
    while let Some(c) = cur.next() {
        out.push(c);
        match c {
            '\\' => {
                if let Some(n) = cur.next() {
                    out.push(n);
                }
            }
            '[' => depth += 1,
            ']' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Ok(out);
                }
            }
            _ => {}
        }
    }
    Err(SyntaxError::new(start, "unterminated set"))
}

/// Parses `order` or `tertiary`: one integer from -128 to 127, or a
/// space-separated list with at most `len` entries, the last repeated to
/// pad it to `len`. Absent gives zeros.
pub fn parse_reorder_ints(value: Option<&str>, len: usize) -> Result<Vec<i8>, SyntaxError> {
    let items: Vec<i8> = match value {
        None => vec![0],
        Some(v) => v
            .split_ascii_whitespace()
            .map(|t| {
                t.parse::<i8>().map_err(|_| {
                    SyntaxError::new(0, format!("{t:?} is not an integer from -128 to 127"))
                })
            })
            .collect::<Result<_, _>>()?,
    };
    pad(items, len)
}

/// Parses `tertiaryBase` or `preBase`: `true` or `false` (also `1` and
/// `0`, as LDML's own example writes them), or a list padded as for
/// [`parse_reorder_ints`].
pub fn parse_reorder_bools(value: Option<&str>, len: usize) -> Result<Vec<bool>, SyntaxError> {
    let items: Vec<bool> = match value {
        None => vec![false],
        Some(v) => v
            .split_ascii_whitespace()
            .map(|t| match t {
                "true" | "1" => Ok(true),
                "false" | "0" => Ok(false),
                _ => Err(SyntaxError::new(0, format!("{t:?} is not true or false"))),
            })
            .collect::<Result<_, _>>()?,
    };
    pad(items, len)
}

fn pad<T: Copy>(mut items: Vec<T>, len: usize) -> Result<Vec<T>, SyntaxError> {
    if items.len() > len {
        return Err(SyntaxError::new(
            0,
            format!("{} values for {len} elements", items.len()),
        ));
    }
    let Some(&last) = items.last() else {
        return Err(SyntaxError::new(0, "empty value list"));
    };
    items.resize(len, last);
    Ok(items)
}

/// Writes a reorder string. A `Ranges` element is always bracketed, even
/// for one character, so that it parses back as `Ranges`.
pub fn encode_reorder(elems: &[ReorderElem]) -> String {
    let mut out = String::new();
    for elem in elems {
        match elem {
            ReorderElem::Char(c) if needs_escape(*c) => push_hex(&mut out, *c),
            ReorderElem::Char(c) if FROM_METACHARS.contains(*c) || *c == '-' => {
                out.push('\\');
                out.push(*c);
            }
            ReorderElem::Char(c) => out.push(*c),
            ReorderElem::Ranges(ranges) => out.push_str(&encode_uset(ranges)),
        }
    }
    out
}
