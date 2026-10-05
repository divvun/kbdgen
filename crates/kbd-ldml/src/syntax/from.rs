//! `transform@from`: §Transform From Grammar.

use kbd_model::{MAX_CAPTURES, is_nmtoken};

use super::{
    Atom, ClassMember, ClassSyntax, FROM_METACHARS, PatternSyntax, Quantified, fixed_for,
    fixed_letter, is_variable_id,
};
use crate::escape::{Cursor, SyntaxError, hex_escape, push_hex};
use crate::gencat::needs_escape;

/// Where an atom is being parsed: captures admit only plain atoms.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Context {
    Top,
    Group,
    Capture,
}

// [spec:kbdgen:syn:ldml.xml.from+1]
/// Parses a `from` value whose `${…}` references are already substituted
/// (`super::substitute_strings`). Well-formedness errors of the grammar
/// are errors here: nested captures, more than nine captures, empty
/// alternatives, `^` other than first, `$`, unbounded quantifiers,
/// backreferences, named groups, lookaround, `\p{…}` and undefined escapes.
/// Whether the pattern can match the empty text is checked on the model.
pub fn parse_pattern(value: &str) -> Result<PatternSyntax, SyntaxError> {
    let mut cur = Cursor::new(value);
    let anchored = cur.eat('^');
    let alternatives = parse_alternation(&mut cur, Context::Top)?;
    if let Some(c) = cur.peek() {
        return Err(SyntaxError::new(cur.offset, format!("unexpected {c:?}")));
    }
    let pattern = PatternSyntax {
        anchored,
        alternatives,
    };
    let captures = count_captures(&pattern.alternatives);
    if captures > usize::from(MAX_CAPTURES) {
        return Err(SyntaxError::new(
            0,
            format!("{captures} capture groups; at most nine are allowed"),
        ));
    }
    Ok(pattern)
}

fn count_captures(alternatives: &[Vec<Quantified>]) -> usize {
    alternatives
        .iter()
        .flatten()
        .map(|q| match &q.atom {
            Atom::Capture(_) => 1,
            Atom::Group(inner) => count_captures(inner),
            _ => 0,
        })
        .sum()
}

fn parse_alternation(
    cur: &mut Cursor,
    context: Context,
) -> Result<Vec<Vec<Quantified>>, SyntaxError> {
    let mut alternatives = vec![Vec::new()];
    loop {
        match cur.peek() {
            None | Some(')') => break,
            Some('|') => {
                if context == Context::Capture {
                    return Err(SyntaxError::new(
                        cur.offset,
                        "alternation inside a capture group; use (?:…|…) around captures",
                    ));
                }
                if alternatives.last().is_some_and(Vec::is_empty) {
                    return Err(SyntaxError::new(cur.offset, "empty alternative before |"));
                }
                cur.next();
                alternatives.push(Vec::new());
            }
            Some(_) => {
                let atoms = parse_quantified(cur, context)?;
                if let Some(sequence) = alternatives.last_mut() {
                    sequence.extend(atoms);
                }
            }
        }
    }
    if alternatives.last().is_some_and(Vec::is_empty) {
        return Err(SyntaxError::new(cur.offset, "empty pattern or alternative"));
    }
    Ok(alternatives)
}

/// One atom and its quantifier. A `\u{…}` naming several scalar values is
/// several atoms, grouped when quantified.
fn parse_quantified(cur: &mut Cursor, context: Context) -> Result<Vec<Quantified>, SyntaxError> {
    let start = cur.offset;
    let atoms = parse_atom(cur, context)?;
    let Some((min, max)) = parse_quantifier(cur)? else {
        return Ok(atoms.into_iter().map(Quantified::one).collect());
    };
    let atom = match <[Atom; 1]>::try_from(atoms) {
        Ok([atom]) => atom,
        Err(atoms) => {
            if context == Context::Capture {
                return Err(SyntaxError::new(
                    start,
                    "a quantified multi-character escape inside a capture needs a group",
                ));
            }
            Atom::Group(vec![atoms.into_iter().map(Quantified::one).collect()])
        }
    };
    Ok(vec![Quantified { atom, min, max }])
}

fn parse_quantifier(cur: &mut Cursor) -> Result<Option<(u8, u8)>, SyntaxError> {
    let start = cur.offset;
    let bounds = if cur.eat('?') {
        (0, 1)
    } else if cur.eat('{') {
        let digit = |c: Option<char>| c.and_then(|c| c.to_digit(10)).map(|d| d as u8);
        let min = digit(cur.next());
        let comma = cur.eat(',');
        let max = digit(cur.next());
        let close = cur.eat('}');
        match (min, comma, max, close) {
            (Some(min), true, Some(max), true) if max >= 1 && min <= max => (min, max),
            _ => {
                return Err(SyntaxError::new(
                    start,
                    "a bounded quantifier is {x,y} with single digits, y ≥ 1 and y ≥ x",
                ));
            }
        }
    } else {
        return match cur.peek() {
            Some('*' | '+') => Err(SyntaxError::new(
                start,
                "unbounded quantifiers are not allowed; use {x,y}",
            )),
            _ => Ok(None),
        };
    };
    match cur.peek() {
        Some('?' | '{' | '*' | '+') => Err(SyntaxError::new(
            cur.offset,
            "a quantifier may not follow a quantifier",
        )),
        _ => Ok(Some(bounds)),
    }
}

fn parse_atom(cur: &mut Cursor, context: Context) -> Result<Vec<Atom>, SyntaxError> {
    let start = cur.offset;
    let Some(c) = cur.next() else {
        return Err(SyntaxError::new(start, "expected an atom"));
    };
    let atom = match c {
        '(' => parse_group(cur, context, start)?,
        '[' => Atom::Class(parse_class(cur, start)?),
        '.' => Atom::Any,
        '\\' => return parse_escape(cur, start),
        '$' => {
            if !cur.eat('[') {
                return Err(SyntaxError::new(
                    start,
                    "unescaped $; the end of the context is implicit",
                ));
            }
            let id = until(cur, ']', start, "$[ variable")?;
            if !is_variable_id(&id) {
                return Err(SyntaxError::new(
                    start,
                    format!("{id:?} is not a variable id"),
                ));
            }
            Atom::Var(id)
        }
        '^' => return Err(SyntaxError::new(start, "^ is only allowed at the start")),
        '*' | '+' | '?' | '{' => {
            return Err(SyntaxError::new(
                start,
                format!("{c:?} has nothing to quantify"),
            ));
        }
        ')' | ']' | '}' | '|' => {
            return Err(SyntaxError::new(start, format!("unescaped {c:?}")));
        }
        c => Atom::Char(c),
    };
    Ok(vec![atom])
}

fn until(cur: &mut Cursor, end: char, start: usize, what: &str) -> Result<String, SyntaxError> {
    let mut out = String::new();
    loop {
        match cur.next() {
            Some(c) if c == end => return Ok(out),
            Some(c) => out.push(c),
            None => return Err(SyntaxError::new(start, format!("unterminated {what}"))),
        }
    }
}

fn parse_group(cur: &mut Cursor, context: Context, start: usize) -> Result<Atom, SyntaxError> {
    if context == Context::Capture {
        return Err(SyntaxError::new(
            start,
            "groups may not be nested inside a capture group",
        ));
    }
    let atom = if cur.eat('?') {
        if !cur.eat(':') {
            return Err(SyntaxError::new(
                start,
                "only (?:…) groups are allowed: no named groups or lookaround",
            ));
        }
        Atom::Group(parse_alternation(cur, Context::Group)?)
    } else {
        let mut alternatives = parse_alternation(cur, Context::Capture)?;
        Atom::Capture(alternatives.pop().unwrap_or_default())
    };
    if !cur.eat(')') {
        return Err(SyntaxError::new(start, "unclosed group"));
    }
    Ok(atom)
}

fn parse_escape(cur: &mut Cursor, start: usize) -> Result<Vec<Atom>, SyntaxError> {
    let Some(c) = cur.next() else {
        return Err(SyntaxError::new(start, "trailing backslash"));
    };
    if (c == 'u' || c == 'm') && cur.eat('{') {
        let body = until(cur, '}', start, "escape")?;
        if c == 'u' {
            return Ok(hex_escape(&body, start)?
                .into_iter()
                .map(Atom::Char)
                .collect());
        }
        if body == "." {
            return Ok(vec![Atom::AnyMarker]);
        }
        if !is_nmtoken(&body) {
            return Err(SyntaxError::new(
                start,
                format!("marker name {body:?} is not an NMTOKEN"),
            ));
        }
        return Ok(vec![Atom::Marker(body)]);
    }
    if let Some(fixed) = fixed_for(c) {
        return Ok(vec![Atom::Fixed(fixed)]);
    }
    if FROM_METACHARS.contains(c) {
        return Ok(vec![Atom::Char(c)]);
    }
    Err(SyntaxError::new(
        start,
        format!("\\{c} is not an allowed escape (no backreferences, \\p{{…}} or assertions)"),
    ))
}

/// One class member edge: a single character, or a marker.
enum Edge {
    Char(char),
    Chars(Vec<char>),
    Marker(String),
    AnyMarker,
}

fn class_edge(cur: &mut Cursor, start: usize) -> Result<Edge, SyntaxError> {
    let at = cur.offset;
    let Some(c) = cur.next() else {
        return Err(SyntaxError::new(start, "unterminated class"));
    };
    match c {
        '\\' => {
            let Some(e) = cur.next() else {
                return Err(SyntaxError::new(at, "trailing backslash"));
            };
            if (e == 'u' || e == 'm') && cur.eat('{') {
                let body = until(cur, '}', at, "escape")?;
                if e == 'u' {
                    let chars = hex_escape(&body, at)?;
                    return Ok(match <[char; 1]>::try_from(chars) {
                        Ok([c]) => Edge::Char(c),
                        Err(chars) => Edge::Chars(chars),
                    });
                }
                if body == "." {
                    return Ok(Edge::AnyMarker);
                }
                if !is_nmtoken(&body) {
                    return Err(SyntaxError::new(
                        at,
                        format!("marker name {body:?} is not an NMTOKEN"),
                    ));
                }
                return Ok(Edge::Marker(body));
            }
            if FROM_METACHARS.contains(e) || e == '-' {
                Ok(Edge::Char(e))
            } else {
                Err(SyntaxError::new(
                    at,
                    format!("\\{e} is not allowed in a class"),
                ))
            }
        }
        '[' => Err(SyntaxError::new(at, "classes may not be nested")),
        '-' => Err(SyntaxError::new(at, "unescaped - in a class; write \\-")),
        c => Ok(Edge::Char(c)),
    }
}

fn parse_class(cur: &mut Cursor, start: usize) -> Result<ClassSyntax, SyntaxError> {
    let negated = cur.eat('^');
    let mut members = Vec::new();
    loop {
        if cur.eat(']') {
            break;
        }
        let at = cur.offset;
        match class_edge(cur, start)? {
            Edge::Marker(m) => members.push(ClassMember::Marker(m)),
            Edge::AnyMarker => members.push(ClassMember::AnyMarker),
            Edge::Chars(chars) => {
                members.extend(chars.into_iter().map(|c| ClassMember::Range(c, c)))
            }
            Edge::Char(lo) => {
                if cur.peek() == Some('-') && !cur.starts_with("-]") {
                    cur.next();
                    let Edge::Char(hi) = class_edge(cur, start)? else {
                        return Err(SyntaxError::new(at, "a range edge is a single character"));
                    };
                    if hi < lo {
                        return Err(SyntaxError::new(at, "reversed range in a class"));
                    }
                    members.push(ClassMember::Range(lo, hi));
                } else {
                    members.push(ClassMember::Range(lo, lo));
                }
            }
        }
    }
    if members.is_empty() {
        return Err(SyntaxError::new(start, "empty class"));
    }
    Ok(ClassSyntax { negated, members })
}

// [spec:kbdgen:req:ldml.xml.export.escape]
/// Writes a pattern in `from` syntax: metacharacters escaped with `\`, and
/// characters of general category M, Cc, Cf or Z other than U+0020 as
/// `\u{…}` (`ldml.xml.export.escape`).
pub fn encode_pattern(pattern: &PatternSyntax) -> String {
    let mut out = String::new();
    if pattern.anchored {
        out.push('^');
    }
    encode_alternation(&mut out, &pattern.alternatives);
    out
}

fn encode_alternation(out: &mut String, alternatives: &[Vec<Quantified>]) {
    for (i, sequence) in alternatives.iter().enumerate() {
        if i > 0 {
            out.push('|');
        }
        for q in sequence {
            encode_atom(out, &q.atom);
            match (q.min, q.max) {
                (1, 1) => {}
                (0, 1) => out.push('?'),
                (min, max) => out.push_str(&format!("{{{min},{max}}}")),
            }
        }
    }
}

fn encode_char(out: &mut String, c: char, extra: &str) {
    if c != ' ' && needs_escape(c) {
        push_hex(out, c);
    } else if FROM_METACHARS.contains(c) || extra.contains(c) {
        out.push('\\');
        out.push(c);
    } else {
        out.push(c);
    }
}

fn encode_atom(out: &mut String, atom: &Atom) {
    match atom {
        Atom::Char(c) => encode_char(out, *c, ""),
        Atom::Any => out.push('.'),
        Atom::Fixed(f) => {
            out.push('\\');
            out.push(fixed_letter(*f));
        }
        Atom::Marker(m) => out.push_str(&format!("\\m{{{m}}}")),
        Atom::AnyMarker => out.push_str("\\m{.}"),
        Atom::Var(id) => out.push_str(&format!("$[{id}]")),
        Atom::Group(alternatives) => {
            out.push_str("(?:");
            encode_alternation(out, alternatives);
            out.push(')');
        }
        Atom::Capture(sequence) => {
            out.push('(');
            encode_alternation(out, std::slice::from_ref(sequence));
            out.push(')');
        }
        Atom::Class(class) => {
            out.push('[');
            if class.negated {
                out.push('^');
            }
            for member in &class.members {
                match member {
                    ClassMember::Range(lo, hi) => {
                        encode_char(out, *lo, "-");
                        if hi != lo {
                            out.push('-');
                            encode_char(out, *hi, "-");
                        }
                    }
                    ClassMember::Marker(m) => out.push_str(&format!("\\m{{{m}}}")),
                    ClassMember::AnyMarker => out.push_str("\\m{.}"),
                }
            }
            out.push(']');
        }
    }
}
