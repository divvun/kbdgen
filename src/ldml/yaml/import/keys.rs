//! How import writes each key (`ldml.yaml.import`, Tokens): as the
//! token that lowering would turn back into the same key, or as a `keys`
//! entry named by `\k{id}`.

use kbd_ldml::escape::{Piece, encode_text};
use kbd_ldml::{encode_width, implied_keys};
use kbd_model::{DEFAULT_WIDTH, Key, Keyboard, Text, TextElem};

use super::Level;
use crate::ldml::yaml::keys::{BaseId, Shape, base_id};

/// How one key of a document is written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyTok {
    /// An implied key, unchanged: its token names it without a `keys`
    /// entry.
    Implied(String),
    /// A key lowering makes from this token, with this id.
    Made(String),
    /// A `keys` entry, written `\k{id}`.
    Explicit,
    /// A key written `\k{id}` without a `keys` entry: an implied key that
    /// must not pick up a `longPress` entry for its output.
    Ref,
}

/// The decoded pieces of a model text, markers by name.
pub fn pieces(kb: &Keyboard, text: &Text) -> Vec<Piece> {
    text.elements()
        .iter()
        .map(|e| match e {
            TextElem::Char(c) => Piece::Char(*c),
            TextElem::Marker(m) => Piece::Marker(kb.marker_name(*m).unwrap_or("").to_string()),
        })
        .collect()
}

/// A string as a token or inside `\d{…}`: `key@output` syntax with spaces
/// escaped, since a token never holds whitespace, and with a backslash
/// that would start a token escape written as `\u{005C}`.
pub fn token_text(pieces: &[Piece]) -> String {
    let mut out = encode_text(pieces).replace(' ', "\\u{0020}");
    if ["\\s{", "\\d{", "\\k{", "\\l{"]
        .iter()
        .any(|p| out.starts_with(p))
    {
        out.replace_range(0..1, "\\u{005C}");
    }
    out
}

pub fn plain_token(s: &str) -> String {
    token_text(&s.chars().map(Piece::Char).collect::<Vec<_>>())
}

fn with_width(base: &str, width: u32) -> String {
    if width == DEFAULT_WIDTH {
        format!("\\{base}}}")
    } else {
        format!("\\{base}:{}}}", encode_width(width))
    }
}

/// Whether `id` is `base` or `base` with a collision suffix `-n`, n ≥ 2.
fn matches_base(id: &str, base: &str) -> bool {
    id == base
        || id
            .strip_prefix(base)
            .and_then(|rest| rest.strip_prefix('-'))
            .and_then(|n| n.parse::<u32>().ok())
            .is_some_and(|n| n >= 2)
}

/// Whether `id` is an `o-<n>` id, possibly with a suffix.
fn is_marker_output_id(id: &str) -> bool {
    let mut parts = id.split('-');
    parts.next() == Some("o")
        && parts.next().and_then(|n| n.parse::<u32>().ok()).is_some()
        && parts.all(|n| n.parse::<u32>().is_ok_and(|n| n >= 2))
        && id.split('-').count() <= 3
}

/// Where a document uses each key: in hardware rows, touch rows, or both,
/// counting long-press and flick targets where their key is used.
pub fn usage(kb: &Keyboard) -> Vec<(bool, bool)> {
    let mut used = vec![(false, false); kb.keys.len()];
    let mark = |i: u16, hw: bool, used: &mut Vec<(bool, bool)>| {
        let mut stack = vec![i];
        while let Some(k) = stack.pop() {
            let Some(slot) = used.get_mut(usize::from(k)) else {
                continue;
            };
            let seen = if hw { slot.0 } else { slot.1 };
            if seen {
                continue;
            }
            if hw {
                slot.0 = true;
            } else {
                slot.1 = true;
            }
            let Some(key) = kb.key(k) else {
                continue;
            };
            stack.extend(key.long_press.iter().copied());
            if let Some(f) = key.flick.and_then(|f| kb.flicks.get(usize::from(f))) {
                stack.extend(f.segments.iter().map(|s| s.key));
            }
        }
    };
    if let Some(hw) = &kb.hardware {
        for k in hw.layers.iter().flat_map(|l| l.rows.iter().flatten()) {
            mark(*k, true, &mut used);
        }
    }
    for k in kb
        .touch
        .iter()
        .flat_map(|s| s.layers.iter().flat_map(|l| l.rows.iter().flatten()))
    {
        mark(*k, false, &mut used);
    }
    used
}

/// The token of an implied key, unless the document overrides it.
fn implied_token(kb: &Keyboard, key: &Key) -> Option<String> {
    if !implied_keys(kb.conforms_to).contains(key) {
        return None;
    }
    Some(match key.id.as_str() {
        "space" => "\\s{space}".to_string(),
        "gap" => "\\s{gap}".to_string(),
        id => id.to_string(),
    })
}

/// How a key is written. At `Level::Verbatim` every key but the unchanged
/// implied ones is a `keys` entry. At `Level::Sugar` a key is made from a
/// token when its id is the one `ldml.yaml.key-ids` gives it and the
/// token can carry all its attributes where the key is used; `dead` maps a
/// top-level dead-key marker to its identity.
pub fn classify(
    kb: &Keyboard,
    key: &Key,
    used: (bool, bool),
    level: Level,
    dead: &dyn Fn(&str) -> Option<String>,
) -> KeyTok {
    if let Some(token) = implied_token(kb, key) {
        return KeyTok::Implied(token);
    }
    if level == Level::Verbatim || key.long_press_default.is_some() || !key.multi_tap.is_empty() {
        return KeyTok::Explicit;
    }
    let output = pieces(kb, &key.output);
    let (in_hardware, in_touch) = used;
    let plain = key.width == DEFAULT_WIDTH
        && !key.gap
        && !key.stretch
        && key.layer_id.is_none()
        && key.role.is_none()
        && key.long_press.is_empty()
        && key.flick.is_none();
    let shape = Shape {
        output: &output,
        gap: key.gap,
        layer: key.layer_id.as_deref(),
        stretch: key.stretch,
        role: key.role,
        plain,
    };
    let id_ok = match base_id(&shape, &|m| dead(m).is_some()) {
        BaseId::Id(base) => matches_base(&key.id, &base),
        BaseId::MarkerOutput => is_marker_output_id(&key.id),
    };
    let flick_ok = key
        .flick
        .and_then(|f| kb.flicks.get(usize::from(f)))
        .is_none_or(|f| f.id == format!("flick-{}", key.id) && !in_hardware);
    if !id_ok || !flick_ok {
        return KeyTok::Explicit;
    }
    let token = if let Some(role) = key.role {
        if in_hardware {
            return KeyTok::Explicit;
        }
        with_width(&format!("s{{{}", role.name()), key.width)
    } else if key.gap {
        with_width("s{gap", key.width)
    } else if let Some(layer) = &key.layer_id {
        if in_hardware || !output.is_empty() {
            return KeyTok::Explicit;
        }
        with_width(&format!("l{{{layer}"), key.width)
    } else if let [Piece::Marker(m)] = output.as_slice()
        && let Some(identity) = dead(m)
        && key.width == DEFAULT_WIDTH
    {
        format!("\\d{{{}}}", plain_token(&identity))
    } else if output == [Piece::Char(' ')] && key.stretch {
        with_width("s{space", key.width)
    } else if key.stretch || output.is_empty() {
        return KeyTok::Explicit;
    } else if key.width != DEFAULT_WIDTH {
        if in_hardware {
            return KeyTok::Explicit;
        }
        format!(
            "\\s{{\"{}\":{}}}",
            token_text(&output),
            encode_width(key.width)
        )
    } else {
        token_text(&output)
    };
    if in_hardware && !in_touch && token.starts_with("\\l{") {
        return KeyTok::Explicit;
    }
    KeyTok::Made(token)
}

/// The token written for key `index` in a row or a candidate list.
pub fn token_of(kb: &Keyboard, classes: &[KeyTok], index: u16) -> String {
    match classes.get(usize::from(index)) {
        Some(KeyTok::Implied(t) | KeyTok::Made(t)) => t.clone(),
        _ => format!("\\k{{{}}}", kb.key(index).map_or("", |k| k.id.as_str())),
    }
}
