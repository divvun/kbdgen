//! `emoji` (`ldml.yaml.emoji`): the preserved key, by ISO position, and
//! CLDR emoji annotations read through `xmlem`.

use std::path::Path;

use kbd_ldml::parse_modifiers;
use kbd_model::{Annotation, Emoji, EmojiKey, ModifierSet};
use serde_yaml::Value;
use std::str::FromStr;

use xmlem::{Document, Element};

use super::error::{At, Result};
use super::node::{Fields, string};

/// The 49 ISO positions of `keys.iso-order` and their scan codes.
const ISO_POSITIONS: [(&str, u8); 49] = [
    ("E00", 0x29),
    ("E01", 0x02),
    ("E02", 0x03),
    ("E03", 0x04),
    ("E04", 0x05),
    ("E05", 0x06),
    ("E06", 0x07),
    ("E07", 0x08),
    ("E08", 0x09),
    ("E09", 0x0A),
    ("E10", 0x0B),
    ("E11", 0x0C),
    ("E12", 0x0D),
    ("D01", 0x10),
    ("D02", 0x11),
    ("D03", 0x12),
    ("D04", 0x13),
    ("D05", 0x14),
    ("D06", 0x15),
    ("D07", 0x16),
    ("D08", 0x17),
    ("D09", 0x18),
    ("D10", 0x19),
    ("D11", 0x1A),
    ("D12", 0x1B),
    ("C01", 0x1E),
    ("C02", 0x1F),
    ("C03", 0x20),
    ("C04", 0x21),
    ("C05", 0x22),
    ("C06", 0x23),
    ("C07", 0x24),
    ("C08", 0x25),
    ("C09", 0x26),
    ("C10", 0x27),
    ("C11", 0x28),
    ("C12", 0x2B),
    ("B00", 0x56),
    ("B01", 0x2C),
    ("B02", 0x2D),
    ("B03", 0x2E),
    ("B04", 0x2F),
    ("B05", 0x30),
    ("B06", 0x31),
    ("B07", 0x32),
    ("B08", 0x33),
    ("B09", 0x34),
    ("B10", 0x35),
    ("B11", 0x73),
];

pub fn position_name(scan_code: u8) -> Option<&'static str> {
    ISO_POSITIONS
        .iter()
        .find(|(_, c)| *c == scan_code)
        .map(|(n, _)| *n)
}

fn key(value: &Value, at: &At) -> Result<EmojiKey> {
    let mut f = Fields::new(value, at)?;
    let (p, pa) = f.require("position")?;
    let position = string(p, &pa)?;
    let scan_code = ISO_POSITIONS
        .iter()
        .find(|(n, _)| *n == position)
        .map(|(_, c)| *c)
        .ok_or_else(|| pa.error(format!("{position} is not an ISO position E00…B11")))?;
    let (m, ma) = f.require("modifiers")?;
    let text = string(m, &ma)?;
    let modifiers = match parse_modifiers(text).map_err(|e| ma.error(e))?.as_slice() {
        [ModifierSet::Set(m)] => *m,
        _ => return Err(ma.error("the emoji key has one modifier set, not other")),
    };
    f.finish()?;
    Ok(EmojiKey {
        scan_code,
        modifiers,
    })
}

fn text_of(doc: &Document, el: Element) -> String {
    el.child_nodes(doc)
        .iter()
        .filter_map(|n| n.as_text())
        .map(|t| t.as_str(doc))
        .collect()
}

/// Reads a CLDR annotations file (LDML Part 2): per emoji, in order of
/// first appearance, the `type="tts"` text as its name and the other
/// annotations' `|`-separated keywords.
pub fn annotations(path: &Path, at: &At) -> Result<Vec<Annotation>> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| at.error(format!("cannot read {}: {e}", path.display())))?;
    let doc = Document::from_str(text.trim_start_matches('\u{FEFF}'))
        .map_err(|e| at.error(format!("{}: malformed XML: {e}", path.display())))?;
    let root = doc.root();
    let Some(list) = root
        .children(&doc)
        .into_iter()
        .find(|c| c.name(&doc) == "annotations")
    else {
        return Err(at.error(format!("{} has no annotations element", path.display())));
    };
    let mut out: Vec<Annotation> = Vec::new();
    for el in list.children(&doc) {
        if el.name(&doc) != "annotation" {
            continue;
        }
        let Some(cp) = el.attribute(&doc, "cp") else {
            return Err(at.error(format!("{}: an annotation lacks cp", path.display())));
        };
        let index = match out.iter().position(|a| a.emoji == cp) {
            Some(i) => i,
            None => {
                out.push(Annotation {
                    emoji: cp.to_string(),
                    name: String::new(),
                    keywords: Vec::new(),
                });
                out.len() - 1
            }
        };
        let body = text_of(&doc, el);
        let Some(entry) = out.get_mut(index) else {
            continue;
        };
        if el.attribute(&doc, "type") == Some("tts") {
            entry.name = body.trim().to_string();
        } else {
            entry.keywords.extend(
                body.split('|')
                    .map(str::trim)
                    .filter(|k| !k.is_empty())
                    .map(str::to_string),
            );
        }
    }
    Ok(out)
}

// [spec:kbdgen:def:ldml.yaml.emoji]
/// `emoji: {key?, annotations?}`, with the annotations path relative to
/// the layout file.
pub fn emoji(value: &Value, at: &At, dir: &Path) -> Result<Emoji> {
    let mut f = Fields::new(value, at)?;
    let mut out = Emoji::default();
    if let Some((v, a)) = f.take("key") {
        out.key = Some(key(v, &a)?);
    }
    if let Some((v, a)) = f.take("annotations") {
        out.annotations = annotations(&dir.join(string(v, &a)?), &a)?;
    }
    f.finish()?;
    Ok(out)
}
