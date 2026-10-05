//! Parsers, and their inverse encoders, for LDML's text syntaxes: the
//! regex-like `transform@from` (`ldml.xml.from`), `transform@to`
//! (`ldml.xml.to`), `set` and `uset` values (`ldml.xml.sets`) and
//! `reorder@from`/`@before`.
//!
//! The `regex` crate can express neither markers, mapped sets nor LDML's
//! restrictions (`ldml.crate.ldml`), so these are hand-written. Each parser
//! yields a syntax tree that still names markers and variables; resolution
//! interns the names into the model's tables. Each encoder writes a tree
//! back in the form `ldml.xml.export.escape` asks for, and parsing an
//! encoded tree gives it back.

mod from;
mod reorder;
mod sets;
mod to;

pub use from::{encode_pattern, parse_pattern};
pub use reorder::{
    ReorderElem, encode_reorder, parse_reorder, parse_reorder_bools, parse_reorder_ints,
};
pub use sets::{encode_set_items, encode_uset, parse_set, parse_uset};
pub use to::{ToItem, encode_replacement, parse_replacement};

use kbd_model::{ClassRange, Fixed};

use crate::escape::{Cursor, SyntaxError};

/// A pattern atom with its quantifier `{min,max}`; unquantified is `{1,1}`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Quantified {
    pub atom: Atom,
    pub min: u8,
    pub max: u8,
}

impl Quantified {
    pub fn one(atom: Atom) -> Self {
        Quantified {
            atom,
            min: 1,
            max: 1,
        }
    }
}

/// An atom of `transform@from`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Atom {
    Char(char),
    /// `.`
    Any,
    Fixed(Fixed),
    Marker(String),
    /// `\m{.}`
    AnyMarker,
    Class(ClassSyntax),
    /// `$[id]`, a `set` or a `uset`.
    Var(String),
    /// `(?:…)`, an alternation.
    Group(Vec<Vec<Quantified>>),
    /// `(…)`. The grammar's `catoms` admits neither alternation nor groups
    /// inside a capture, so a capture is one sequence of plain atoms.
    Capture(Vec<Quantified>),
}

/// A bracketed class: `[…]` or `[^…]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassSyntax {
    pub negated: bool,
    pub members: Vec<ClassMember>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClassMember {
    /// An inclusive range; a single character has `lo == hi`.
    Range(char, char),
    Marker(String),
    AnyMarker,
}

/// A parsed `transform@from`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatternSyntax {
    /// `^`
    pub anchored: bool,
    pub alternatives: Vec<Vec<Quantified>>,
}

const FIXED: [(char, Fixed); 11] = [
    ('s', Fixed::Space),
    ('S', Fixed::NotSpace),
    ('d', Fixed::Digit),
    ('D', Fixed::NotDigit),
    ('w', Fixed::Word),
    ('W', Fixed::NotWord),
    ('t', Fixed::Tab),
    ('r', Fixed::CarriageReturn),
    ('n', Fixed::LineFeed),
    ('f', Fixed::FormFeed),
    ('v', Fixed::VerticalTab),
];

pub(crate) fn fixed_for(c: char) -> Option<Fixed> {
    FIXED.iter().find(|(k, _)| *k == c).map(|(_, f)| *f)
}

pub(crate) fn fixed_letter(f: Fixed) -> char {
    FIXED.iter().find(|(_, g)| *g == f).map_or('s', |(k, _)| *k)
}

/// The characters that `transform@from` escapes with a backslash
/// (§Regex-like Syntax, Escapes).
pub(crate) const FROM_METACHARS: &str = ".()?[\\]{}*/^+|$";

/// Whether `id` matches `[0-9A-Za-z_]{1,32}`, the variable id pattern.
pub fn is_variable_id(id: &str) -> bool {
    (1..=32).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

// [spec:kbdgen:syn:ldml.xml.from+1]
/// Replaces each `${id}` of `value` by the raw value of string variable
/// `id`, textually, skipping backslash escapes so that `\$` stays literal.
/// `string` returns a variable's raw value, already substituted itself.
pub fn substitute_strings(
    value: &str,
    string: &dyn Fn(&str) -> Option<String>,
) -> Result<String, SyntaxError> {
    let mut cur = Cursor::new(value);
    let mut out = String::new();
    while let Some(c) = cur.next() {
        match c {
            '\\' => {
                out.push(c);
                if let Some(n) = cur.next() {
                    out.push(n);
                }
            }
            '$' if cur.peek() == Some('{') => {
                let start = cur.offset - 1;
                cur.next();
                let id = cur.until_brace(start, "${ variable")?;
                let raw = string(&id).ok_or_else(|| {
                    SyntaxError::new(start, format!("string variable {id:?} is not defined"))
                })?;
                out.push_str(&raw);
            }
            _ => out.push(c),
        }
    }
    Ok(out)
}

/// Sorts and merges ranges into ascending, disjoint, non-adjacent ones.
pub fn normalize_ranges(mut ranges: Vec<ClassRange>) -> Vec<ClassRange> {
    ranges.sort();
    let mut out: Vec<ClassRange> = Vec::new();
    for r in ranges {
        if let Some(last) = out.last_mut() {
            let adjacent = char::from_u32(u32::from(last.hi) + 1).is_some_and(|n| n >= r.lo)
                || last.hi >= r.lo
                || (last.hi == '\u{D7FF}' && r.lo == '\u{E000}');
            if adjacent {
                last.hi = last.hi.max(r.hi);
                continue;
            }
        }
        out.push(r);
    }
    out
}

/// The scalar values not in `ranges`, which must be normalized.
pub fn complement_ranges(ranges: &[ClassRange]) -> Vec<ClassRange> {
    let mut out = Vec::new();
    let mut next = Some('\0');
    for r in ranges {
        if let Some(lo) = next
            && lo < r.lo
        {
            out.push(ClassRange {
                lo,
                hi: before(r.lo).unwrap_or(lo),
            });
        }
        next = after(r.hi);
    }
    if let Some(lo) = next {
        out.push(ClassRange { lo, hi: char::MAX });
    }
    normalize_ranges(out)
}

/// The members of both `a` and `b`; both normalized.
pub fn intersect_ranges(a: &[ClassRange], b: &[ClassRange]) -> Vec<ClassRange> {
    let mut out = Vec::new();
    for x in a {
        for y in b {
            let lo = x.lo.max(y.lo);
            let hi = x.hi.min(y.hi);
            if lo <= hi {
                out.push(ClassRange { lo, hi });
            }
        }
    }
    normalize_ranges(out)
}

fn after(c: char) -> Option<char> {
    match c {
        '\u{D7FF}' => Some('\u{E000}'),
        _ => char::from_u32(u32::from(c) + 1),
    }
}

fn before(c: char) -> Option<char> {
    match c {
        '\u{E000}' => Some('\u{D7FF}'),
        '\0' => None,
        _ => char::from_u32(u32::from(c) - 1),
    }
}

#[cfg(test)]
mod tests;
