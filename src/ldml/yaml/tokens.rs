//! Rows and tokens (`ldml.yaml.rows`, `ldml.yaml.tokens`).

use kbd_ldml::escape::Piece;
use kbd_model::Role;

use super::error::{At, Result};
use super::text::{Strings, output, plain};

/// One token of a row. Widths are thousandths of a key width.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    /// `\u{0}`: no key here, or no flick in a flick row.
    NoKey,
    /// `\s{gap}`, `\s{gap:w}`, `\s{spacer:w}`.
    Gap(Option<u32>),
    /// `\s{space}`, `\s{space:w}`.
    Space(Option<u32>),
    /// `\s{R}`, `\s{R:w}`.
    Role(Role, Option<u32>),
    /// `\s{"x":w}`.
    Sized(Vec<Piece>, u32),
    /// `\d{X}`, by decoded identity.
    Dead(String),
    /// `\k{id}`.
    KeyRef(String),
    /// `\l{L}`, `\l{L:w}`.
    Layer(String, Option<u32>),
    /// Any other token: an output in `key@output` syntax.
    Output(Vec<Piece>),
}

impl Token {
    /// Whether the token is valid only in touch rows.
    pub fn touch_only(&self) -> bool {
        matches!(self, Token::Role(..) | Token::Sized(..) | Token::Layer(..))
    }
}

/// A width: a decimal greater than 0 with at most three fraction digits.
pub fn width(text: &str, at: &At) -> Result<u32> {
    kbd_ldml::parse_width(text)
        .filter(|w| *w > 0)
        .ok_or_else(|| {
            at.error(format!(
                "{text} is not a width greater than 0 with at most three decimals"
            ))
        })
}

fn named_width(body: &str, at: &At) -> Result<(String, Option<u32>)> {
    match body.split_once(':') {
        Some((name, w)) => Ok((name.to_string(), Some(width(w, at)?))),
        None => Ok((body.to_string(), None)),
    }
}

fn special(body: &str, strings: &Strings, at: &At) -> Result<Token> {
    if let Some(quoted) = body.strip_prefix('"') {
        let (text, w) = quoted.rsplit_once("\":").ok_or_else(|| {
            at.error(format!(
                "\\s{{{body}}} is not \\s{{\"x\":w}}, an output with a width"
            ))
        })?;
        let pieces = output(text, strings, at)?;
        if pieces.is_empty() {
            return Err(at.error("\\s{\"\":w} has no output"));
        }
        return Ok(Token::Sized(pieces, width(w, at)?));
    }
    let (name, w) = named_width(body, at)?;
    Ok(match name.as_str() {
        "gap" | "spacer" => Token::Gap(w),
        "space" => Token::Space(w),
        other => match Role::from_name(other) {
            Some(role) => Token::Role(role, w),
            None => return Err(at.error(format!("\\s{{{body}}}: {other} is not a role"))),
        },
    })
}

// [spec:kbdgen:syn:ldml.yaml.tokens]
/// Parses one token, trying the forms of `ldml.yaml.tokens` in order. A
/// token that starts `\s{`, `\d{`, `\k{` or `\l{` must end with `}`; any
/// other backslash is literal output.
pub fn token(raw: &str, strings: &Strings, at: &At) -> Result<Token> {
    for prefix in ["\\s{", "\\d{", "\\k{", "\\l{"] {
        let Some(rest) = raw.strip_prefix(prefix) else {
            continue;
        };
        let body = rest
            .strip_suffix('}')
            .ok_or_else(|| at.error(format!("{raw} does not end with }}")))?;
        if body.is_empty() {
            return Err(at.error(format!("{raw} is empty")));
        }
        return match prefix {
            "\\s{" => special(body, strings, at),
            "\\d{" => Ok(Token::Dead(plain(body, at)?)),
            "\\k{" => Ok(Token::KeyRef(body.to_string())),
            _ => {
                let (layer, w) = named_width(body, at)?;
                Ok(Token::Layer(layer, w))
            }
        };
    }
    let pieces = output(raw, strings, at)?;
    if pieces == [Piece::Char('\0')] {
        return Ok(Token::NoKey);
    }
    if pieces.contains(&Piece::Char('\0')) {
        return Err(at.error("U+0000 is only valid alone, as \\u{0}"));
    }
    Ok(Token::Output(pieces))
}

// [spec:kbdgen:syn:ldml.yaml.rows]
/// Parses a rows value: one row per non-blank line, tokens separated by
/// ASCII spaces and tabs. Rows are numbered from the first non-blank line.
pub fn rows(text: &str, strings: &Strings, at: &At) -> Result<Vec<Vec<Token>>> {
    let mut out = Vec::new();
    for line in text.lines() {
        let raw: Vec<&str> = line.split([' ', '\t']).filter(|t| !t.is_empty()).collect();
        if raw.is_empty() {
            continue;
        }
        let row_at = at.row(out.len());
        let row = raw
            .iter()
            .enumerate()
            .map(|(c, t)| token(t, strings, &row_at.token(c)))
            .collect::<Result<Vec<_>>>()?;
        out.push(row);
    }
    Ok(out)
}

/// Parses a list of tokens written on one or more lines, as a single row:
/// `longPress` candidates.
pub fn candidates(text: &str, strings: &Strings, at: &At) -> Result<Vec<Token>> {
    let row_at = at.row(0);
    text.split_ascii_whitespace()
        .enumerate()
        .map(|(c, t)| token(t, strings, &row_at.token(c)))
        .collect()
}
