//! `set` and `uset` values (`ldml.xml.sets`).

use kbd_model::ClassRange;

use super::{complement_ranges, intersect_ranges, is_variable_id, normalize_ranges};
use crate::escape::{Cursor, Piece, SyntaxError, decode_text, encode_text, hex_escape, push_hex};
use crate::gencat::needs_escape;

// [spec:kbdgen:syn:ldml.xml.sets+1]
/// Parses a `set` value whose `${…}` references are already substituted:
/// ASCII-whitespace-separated items, each a text value, where an item that
/// is exactly `$[id]` splices in the items of an earlier set. `set`
/// returns an earlier set's items.
pub fn parse_set(
    value: &str,
    set: &dyn Fn(&str) -> Option<Vec<Vec<Piece>>>,
) -> Result<Vec<Vec<Piece>>, SyntaxError> {
    let mut items = Vec::new();
    for item in split_items(value) {
        let item = item.as_str();
        if let Some(id) = item.strip_prefix("$[").and_then(|r| r.strip_suffix(']'))
            && !id.contains('[')
        {
            if !is_variable_id(id) {
                return Err(SyntaxError::new(0, format!("{id:?} is not a variable id")));
            }
            let spliced = set(id).ok_or_else(|| {
                SyntaxError::new(0, format!("set {id:?} is not defined before this one"))
            })?;
            items.extend(spliced);
            continue;
        }
        if item.contains("$[") {
            return Err(SyntaxError::new(
                0,
                format!("{item:?}: set references must be separated by whitespace"),
            ));
        }
        items.push(decode_text(item, &|_| None)?);
    }
    Ok(items)
}

/// Splits a set value at ASCII whitespace outside `\u{…}` escapes, whose
/// own spaces separate scalar values of one item.
fn split_items(value: &str) -> Vec<String> {
    let mut items = Vec::new();
    let mut current = String::new();
    let mut in_escape = false;
    let mut prev_backslash = false;
    let mut chars = value.chars().peekable();
    while let Some(c) = chars.next() {
        if in_escape {
            current.push(c);
            in_escape = c != '}';
            continue;
        }
        if c.is_ascii_whitespace() {
            if !current.is_empty() {
                items.push(std::mem::take(&mut current));
            }
            prev_backslash = false;
            continue;
        }
        if prev_backslash && c == 'u' && chars.peek() == Some(&'{') {
            in_escape = true;
        }
        prev_backslash = c == '\\' && !prev_backslash;
        current.push(c);
    }
    if !current.is_empty() {
        items.push(current);
    }
    items
}

/// Writes set items: whitespace-separated text values, with every
/// character of general category M, Cc, Cf or Z, U+0020 included, as
/// `\u{…}` so that no item splits.
pub fn encode_set_items(items: &[Vec<Piece>]) -> String {
    let encoded: Vec<String> = items
        .iter()
        .map(|item| {
            let mut out = String::new();
            for piece in item {
                match piece {
                    Piece::Char(' ') => push_hex(&mut out, ' '),
                    other => out.push_str(&encode_text(std::slice::from_ref(other))),
                }
            }
            out
        })
        .collect();
    encoded.join(" ")
}

// [spec:kbdgen:syn:ldml.xml.sets+1]
/// Parses a `uset` value whose `${…}` references are already substituted,
/// in the UnicodeSet subset of §Element: uset: brackets, `^`, ranges,
/// `\u{…}`, backslash-escaped characters, `$[id]` references to earlier
/// usets, union and difference. Strings (`{…}`) and properties (`\p{…}`,
/// `[:…:]`) are errors. Whitespace between members is ignored. The result
/// is normalized.
pub fn parse_uset(
    value: &str,
    uset: &dyn Fn(&str) -> Option<Vec<ClassRange>>,
) -> Result<Vec<ClassRange>, SyntaxError> {
    let mut cur = Cursor::new(value.trim());
    let ranges = uset_operand(&mut cur, uset)?;
    skip_ws(&mut cur);
    if cur.peek().is_some() {
        return Err(SyntaxError::new(
            cur.offset,
            "unexpected text after the set",
        ));
    }
    Ok(ranges)
}

fn skip_ws(cur: &mut Cursor) {
    while cur.peek().is_some_and(|c| c.is_ascii_whitespace()) {
        cur.next();
    }
}

/// A bracketed set or a `$[id]` reference.
fn uset_operand(
    cur: &mut Cursor,
    uset: &dyn Fn(&str) -> Option<Vec<ClassRange>>,
) -> Result<Vec<ClassRange>, SyntaxError> {
    let start = cur.offset;
    if cur.starts_with("$[") {
        cur.next();
        cur.next();
        let mut id = String::new();
        loop {
            match cur.next() {
                Some(']') => break,
                Some(c) => id.push(c),
                None => return Err(SyntaxError::new(start, "unterminated $[")),
            }
        }
        return uset(&id).ok_or_else(|| {
            SyntaxError::new(start, format!("uset {id:?} is not defined before this one"))
        });
    }
    if cur.starts_with("[:") {
        return Err(SyntaxError::new(
            start,
            "properties ([:…:]) are not allowed",
        ));
    }
    if !cur.eat('[') {
        return Err(SyntaxError::new(start, "a uset is a bracketed set"));
    }
    let negated = cur.eat('^');
    let mut ranges: Vec<ClassRange> = Vec::new();
    loop {
        skip_ws(cur);
        let at = cur.offset;
        match cur.peek() {
            None => return Err(SyntaxError::new(start, "unterminated set")),
            Some(']') => {
                cur.next();
                break;
            }
            Some('[') | Some('$') => {
                let mut operand = uset_operand(cur, uset)?;
                skip_ws(cur);
                if cur.peek() == Some('-') && (cur.starts_with("-[") || cur.starts_with("-$[")) {
                    cur.next();
                    let other = uset_operand(cur, uset)?;
                    operand =
                        intersect_ranges(&normalize_ranges(operand), &complement_ranges(&other));
                }
                ranges.extend(operand);
            }
            Some(_) => {
                let lo = uset_char(cur)?;
                skip_ws(cur);
                if cur.peek() == Some('-') && !cur.starts_with("-]") {
                    cur.next();
                    skip_ws(cur);
                    let hi = uset_char(cur)?;
                    let ([lo], [hi]) = (lo.as_slice(), hi.as_slice()) else {
                        return Err(SyntaxError::new(at, "a range edge is a single character"));
                    };
                    if hi < lo {
                        return Err(SyntaxError::new(at, "reversed range"));
                    }
                    ranges.push(ClassRange { lo: *lo, hi: *hi });
                } else {
                    ranges.extend(lo.into_iter().map(ClassRange::single));
                }
            }
        }
    }
    let ranges = normalize_ranges(ranges);
    Ok(if negated {
        complement_ranges(&ranges)
    } else {
        ranges
    })
}

/// One member character, or several for a multi-value `\u{…}`.
fn uset_char(cur: &mut Cursor) -> Result<Vec<char>, SyntaxError> {
    let at = cur.offset;
    match cur.next() {
        Some('\\') => match cur.next() {
            Some('u') if cur.eat('{') => {
                let body = cur.until_brace(at, "\\u{ escape")?;
                hex_escape(&body, at)
            }
            Some('p' | 'P' | 'N') => Err(SyntaxError::new(
                at,
                "properties and names (\\p{…}, \\N{…}) are not allowed",
            )),
            Some(c) if !c.is_ascii_alphanumeric() => Ok(vec![c]),
            Some(c) => Err(SyntaxError::new(
                at,
                format!("\\{c} is not an allowed escape"),
            )),
            None => Err(SyntaxError::new(at, "trailing backslash")),
        },
        Some('{') => Err(SyntaxError::new(
            at,
            "strings ({…}) are not allowed in a uset",
        )),
        Some(c @ ('}' | '&' | '-' | '^')) => {
            Err(SyntaxError::new(at, format!("unescaped {c:?} in a uset")))
        }
        Some(c) => Ok(vec![c]),
        None => Err(SyntaxError::new(at, "unterminated set")),
    }
}

/// Writes normalized ranges as a bracketed uset, escaping the characters
/// that are UnicodeSet syntax.
pub fn encode_uset(ranges: &[ClassRange]) -> String {
    let mut out = String::from("[");
    let push = |out: &mut String, c: char| {
        if needs_escape(c) {
            push_hex(out, c);
        } else if "[]{}\\-&^$:".contains(c) {
            out.push('\\');
            out.push(c);
        } else {
            out.push(c);
        }
    };
    for r in ranges {
        push(&mut out, r.lo);
        if r.hi != r.lo {
            out.push('-');
            push(&mut out, r.hi);
        }
    }
    out.push(']');
    out
}
