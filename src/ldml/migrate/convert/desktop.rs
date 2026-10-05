//! Desktop sections (`ldml.migrate.platforms`): each becomes a hardware
//! variant of the same name, its layer names modifier sets, its 48 tokens
//! re-split into rows of 13, 12, 12 and 11 on `form: iso`, and its dead
//! keys `\d{X}` tokens.

use super::super::defect::Code;
use super::super::out::HardwareOut;
use super::super::source::{DeadList, Desktop, DesktopSection, Layer3};
use super::super::text::{dead_token, decode_v3};
use super::{Cell, Ctx, DesktopTrace, LayerTrace, Out};

/// The tokens of each row of form `iso`.
const ISO_ROWS: [usize; 4] = [13, 12, 12, 11];

/// The ISO positions `E00`…`B10` that a desktop layer fills, in order.
pub const POSITIONS: [&str; 48] = [
    "E00", "E01", "E02", "E03", "E04", "E05", "E06", "E07", "E08", "E09", "E10", "E11", "E12",
    "D01", "D02", "D03", "D04", "D05", "D06", "D07", "D08", "D09", "D10", "D11", "D12", "C01",
    "C02", "C03", "C04", "C05", "C06", "C07", "C08", "C09", "C10", "C11", "C12", "B00", "B01",
    "B02", "B03", "B04", "B05", "B06", "B07", "B08", "B09", "B10",
];

const NO_KEY: &str = "\\u{0}";

/// The v4 layer key of a v3 layer, and whether it is native-only.
pub fn layer_key(platform: Desktop, name: &str) -> (&'static str, bool) {
    let alt = match platform {
        Desktop::MacOs => "alt",
        Desktop::Windows | Desktop::ChromeOs => "altR",
    };
    match (name, alt) {
        ("default", _) => ("none", false),
        ("shift", _) => ("shift", false),
        ("caps", _) => ("caps", false),
        ("caps+shift", _) => ("caps shift", false),
        ("alt", "alt") => ("alt", false),
        ("alt", _) => ("altR", false),
        ("alt+shift", "alt") => ("alt shift", false),
        ("alt+shift", _) => ("altR shift", false),
        ("alt+caps", "alt") => ("alt caps", false),
        ("alt+caps", _) => ("altR caps", false),
        ("ctrl", _) => ("ctrl", true),
        ("cmd", _) => ("cmd", true),
        ("cmd+shift", _) => ("cmd shift", true),
        ("cmd+alt", _) => ("cmd alt", true),
        _ => ("cmd alt shift", true),
    }
}

/// A layer's dead keys after M01, M02 and M04: the raw entries as v3
/// compared them, and the decoded identities v4 makes dead.
#[derive(Default)]
struct DeadSet {
    raw: Vec<String>,
    identities: Vec<String>,
}

/// The `deadKeys` entry of each layer. Entries naming a missing layer
/// (M02) or a token the layer lacks (M01) are dropped; an entry with no
/// transform blocks the layout (M04).
fn dead_sets(
    platform: &str,
    lists: &[DeadList],
    layers: &[(String, Vec<String>)],
    ctx: &mut Ctx,
) -> Vec<(String, DeadSet)> {
    let mut out = Vec::new();
    for list in lists {
        let Some((_, tokens)) = layers.iter().find(|(n, _)| *n == list.layer) else {
            ctx.defects.add(
                Code::M02,
                &list.path,
                format!(
                    "{platform} has no layer {}; its dead keys are dropped",
                    list.layer
                ),
            );
            continue;
        };
        let decoded: Vec<String> = tokens
            .iter()
            .map(|t| decode_v3(t).unwrap_or_else(|_| t.clone()))
            .collect();
        let mut set = DeadSet {
            raw: list.entries.clone(),
            identities: Vec::new(),
        };
        for (i, entry) in list.entries.iter().enumerate() {
            let at = format!("{}[{}]", list.path, i + 1);
            let Some((_, identity)) = ctx.decode(entry, &at, None) else {
                continue;
            };
            if !decoded.contains(&identity) {
                ctx.defects.add_at(
                    Code::M01,
                    &at,
                    None,
                    Some(entry),
                    format!(
                        "layer {} has no key {entry}; the dead key is dropped",
                        list.layer
                    ),
                );
                continue;
            }
            if ctx.dead_def(&identity).is_none() {
                ctx.defects.add_at(
                    Code::M04,
                    &at,
                    None,
                    Some(entry),
                    format!(
                        "{entry} is dead on layer {} but has no transform: should it be a plain key or a dead key with no compose entries?",
                        list.layer
                    ),
                );
                continue;
            }
            set.identities.push(identity);
        }
        out.push((list.layer.clone(), set));
    }
    out
}

/// What a v3 token did on `platform`, given the raw dead entries of its
/// layer (`kbdl.input.bundle`, `keylayout.keymaps.tokens`, and ChromeOS's
/// raw strings).
fn v3_out(platform: Desktop, raw: &str, dead_raw: &[String]) -> Out {
    let decoded = decode_v3(raw).unwrap_or_else(|_| raw.to_string());
    match platform {
        Desktop::Windows => {
            if raw == NO_KEY || decoded.is_empty() || decoded.starts_with('\0') {
                Out::NoKey
            } else if dead_raw.contains(&decoded) {
                Out::Dead(decoded)
            } else {
                Out::Text(decoded)
            }
        }
        Desktop::MacOs if dead_raw.iter().any(|d| d == raw) => Out::Dead(decoded),
        Desktop::MacOs => Out::Text(decoded),
        Desktop::ChromeOs if dead_raw.iter().any(|d| d == raw) => Out::Dead(raw.to_string()),
        Desktop::ChromeOs => Out::Text(raw.to_string()),
    }
}

/// The v4 token of one v3 token, and its trace cell.
fn token(
    platform: Desktop,
    raw: &str,
    set: Option<&DeadSet>,
    layer: &Layer3,
    row: usize,
    ctx: &mut Ctx,
) -> (String, Cell) {
    let empty = DeadSet::default();
    let set = set.unwrap_or(&empty);
    let out = v3_out(platform, raw, &set.raw);
    let mut explained = Vec::new();
    if platform == Desktop::ChromeOs && raw.contains("\\u{") {
        ctx.defects.add_at(
            Code::M14,
            &layer.path,
            None,
            Some(raw),
            "ChromeOS showed this escape as written; it is now decoded",
        );
        explained.push(Code::M14);
    }
    let Some((fixed, decoded)) = ctx.decode(raw, &layer.path, Some(row)) else {
        explained.push(Code::M99);
        return (NO_KEY.to_string(), Cell { out, explained });
    };
    if fixed != raw {
        explained.push(Code::M06);
    }
    if decoded.starts_with('\0') {
        if platform == Desktop::MacOs {
            ctx.defects.add_at(
                Code::M13,
                &layer.path,
                None,
                Some(raw),
                "macOS typed U+0000 here; it now means no key",
            );
            explained.push(Code::M13);
        }
        return (NO_KEY.to_string(), Cell { out, explained });
    }
    let written = match ctx.dead_def(&decoded) {
        Some(def) if set.identities.contains(&decoded) => {
            ctx.used.insert((platform.name(), decoded.clone()));
            if platform == Desktop::Windows && def.identity.chars().count() > 1 {
                explained.push(Code::M08);
            }
            dead_token(&def.spelling)
        }
        _ => ctx.literal(raw, &layer.path, row),
    };
    (written, Cell { out, explained })
}

/// The `macOS.space` entries as `space` entries of the variant. An entry
/// naming a layer the section lacks is dropped (M10); one that types
/// U+0020 is left to the default.
fn space(section: &DesktopSection, ctx: &mut Ctx) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for (name, raw, path) in &section.space {
        if !section.layers.iter().any(|l| &l.name == name) {
            ctx.defects.add(
                Code::M10,
                path,
                format!("{name} names no layer of macOS; the entry is not carried over"),
            );
            continue;
        }
        let Some((_, decoded)) = ctx.decode(raw, path, None) else {
            continue;
        };
        if decoded == " " {
            continue;
        }
        let (key, _) = layer_key(section.platform, name);
        let token = ctx.literal(raw, path, 1);
        out.push((key.to_string(), token));
    }
    out
}

/// A desktop section as a hardware variant, with its trace. A layer of
/// other than 48 tokens blocks the layout (M05) and is left out.
pub fn convert(section: &DesktopSection, ctx: &mut Ctx) -> (HardwareOut, DesktopTrace) {
    let platform = section.platform;
    let tokens: Vec<(String, Vec<String>)> = section
        .layers
        .iter()
        .map(|l| {
            (
                l.name.clone(),
                l.text.split_whitespace().map(str::to_string).collect(),
            )
        })
        .collect();
    let sets = dead_sets(platform.name(), &section.dead_keys, &tokens, ctx);
    let mut variant = HardwareOut {
        name: platform.name().to_string(),
        ..HardwareOut::default()
    };
    let mut trace = DesktopTrace {
        platform,
        layers: Vec::new(),
    };
    for (layer, (_, raw_tokens)) in section.layers.iter().zip(&tokens) {
        if raw_tokens.len() != POSITIONS.len() {
            ctx.defects.add(
                Code::M05,
                &layer.path,
                format!(
                    "layer {} has {} tokens, not 48; v3 {} and every key after the miscount is suspect",
                    layer.name,
                    raw_tokens.len(),
                    if raw_tokens.len() < 48 {
                        "panicked"
                    } else {
                        "ignored the extra tokens"
                    }
                ),
            );
            continue;
        }
        let set = sets.iter().find(|(n, _)| *n == layer.name).map(|(_, s)| s);
        let mut written = Vec::new();
        let mut cells = Vec::new();
        let mut start = 0;
        for (r, count) in ISO_ROWS.iter().enumerate() {
            let mut row = Vec::new();
            for raw in &raw_tokens[start..start + count] {
                let (t, cell) = token(platform, raw, set, layer, r + 1, ctx);
                row.push(t);
                cells.push(cell);
            }
            written.push(row);
            start += count;
        }
        let (key, native) = layer_key(platform, &layer.name);
        variant.layers.push((key.to_string(), written));
        trace.layers.push(LayerTrace {
            name: layer.name.clone(),
            key: key.to_string(),
            native,
            has_dead_list: section.dead_keys.iter().any(|d| d.layer == layer.name),
            path: layer.path.clone(),
            cells,
        });
    }
    variant.space = space(section, ctx);
    (variant, trace)
}
