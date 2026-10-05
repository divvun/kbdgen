//! Touch sections (`ldml.migrate.platforms`): iOS and Android platforms
//! become sizes of the `iOS` and `android` touch variants, `default`
//! becomes `base`, and the `alt` and `alt+shift` layers become the south
//! flicks of `base` and `shift`.

use super::super::defect::Code;
use super::super::out::{SizeOut, TouchLayerOut, TouchOut};
use super::super::source::{Layer3, Source3, Touch, TouchPlatform, TouchSection};
use super::super::text::{dead_token, decode_v3};
use super::{Cell, Ctx, Out, SizeTrace, TouchLayerTrace, TouchTrace};

const NO_FLICK: &str = "\\u{0}";

/// A role, gap or space token, which the host draws or which types no
/// layout output: `\s{…}` other than `\s{"x":w}`.
fn special(raw: &str) -> bool {
    raw.starts_with("\\s{") && !raw.starts_with("\\s{\"")
}

/// The output `x` of a sized output token `\s{"x":w}`.
fn sized(raw: &str) -> Option<&str> {
    raw.strip_prefix("\\s{\"")?
        .rsplit_once("\":")
        .map(|(x, _)| x)
}

/// A special token that means no key and no flick: a gap or spacer.
fn gap(raw: &str) -> bool {
    raw.starts_with("\\s{gap") || raw.starts_with("\\s{spacer")
}

fn rows(text: &str) -> Vec<Vec<String>> {
    text.lines()
        .map(|l| l.split_whitespace().map(str::to_string).collect::<Vec<_>>())
        .filter(|r| !r.is_empty())
        .collect()
}

/// The dead keys v4 makes on each layer: for iOS the decoded `deadKeys`
/// entries of that layer, after M01, M02 and M04; for Android every
/// transform root, wherever its token occurs (`android.keys`).
struct DeadRule {
    platform: Touch,
    ios: Vec<(String, Vec<String>, Vec<String>)>,
    roots_raw: Vec<String>,
}

impl DeadRule {
    fn new(src: &Source3, section: &TouchSection, ctx: &mut Ctx) -> DeadRule {
        let mut rule = DeadRule {
            platform: section.platform,
            ios: Vec::new(),
            roots_raw: src.transforms.iter().map(|(k, _)| k.clone()).collect(),
        };
        if section.platform != Touch::Ios {
            return rule;
        }
        for list in &section.dead_keys {
            let layers: Vec<&Layer3> = section
                .platforms
                .iter()
                .flat_map(|p| p.layers.iter())
                .filter(|l| l.name == list.layer)
                .collect();
            if layers.is_empty() {
                ctx.defects.add(
                    Code::M02,
                    &list.path,
                    format!("iOS has no layer {}; its dead keys are dropped", list.layer),
                );
                continue;
            }
            let tokens: Vec<String> = layers
                .iter()
                .flat_map(|l| l.text.split_whitespace())
                .map(|t| decode_v3(t).unwrap_or_else(|_| t.to_string()))
                .collect();
            let mut identities = Vec::new();
            for (i, entry) in list.entries.iter().enumerate() {
                let at = format!("{}[{}]", list.path, i + 1);
                let Some((_, identity)) = ctx.decode(entry, &at, None) else {
                    continue;
                };
                if !tokens.contains(&identity) {
                    ctx.defects.add_at(
                        Code::M01,
                        &at,
                        None,
                        Some(entry),
                        format!(
                            "no iOS layer {} has the key {entry}; the dead key is dropped",
                            list.layer
                        ),
                    );
                } else if ctx.dead_def(&identity).is_none() {
                    ctx.defects.add_at(
                        Code::M04,
                        &at,
                        None,
                        Some(entry),
                        format!(
                            "{entry} is dead on iOS layer {} but has no transform: should it be a plain key or a dead key with no compose entries?",
                            list.layer
                        ),
                    );
                } else {
                    identities.push(identity);
                }
            }
            rule.ios
                .push((list.layer.clone(), list.entries.clone(), identities));
        }
        rule
    }

    /// Whether v3 made `raw` dead on `layer`, and whether v4 makes the
    /// decoded `text` dead there.
    fn dead(&self, layer: &str, raw: &str, text: &str, ctx: &Ctx) -> (bool, bool) {
        match self.platform {
            Touch::Ios => self.ios.iter().find(|(l, ..)| l == layer).map_or(
                (false, false),
                |(_, entries, ids)| {
                    (
                        entries.iter().any(|e| e == raw),
                        ids.iter().any(|i| i == text),
                    )
                },
            ),
            Touch::Android => (
                self.roots_raw.iter().any(|r| r == raw),
                ctx.dead_def(text).is_some(),
            ),
        }
    }
}

/// The v4 token of one v3 touch token, and its trace cell: none for
/// `\s{…}` tokens, which the host draws.
fn token(
    rule: &DeadRule,
    layer: &str,
    raw: &str,
    path: &str,
    row: usize,
    ctx: &mut Ctx,
) -> (String, Option<Cell>) {
    if special(raw) {
        return (raw.to_string(), None);
    }
    let mut explained = Vec::new();
    if raw.contains("\\u{") {
        ctx.defects.add_at(
            Code::M14,
            path,
            None,
            Some(raw),
            format!(
                "{} showed this escape as written; it is now decoded",
                rule.platform.name()
            ),
        );
        explained.push(Code::M14);
    }
    if let Some(x) = sized(raw) {
        let cell = Cell {
            out: Out::Text(x.to_string()),
            explained,
        };
        return (raw.to_string(), Some(cell));
    }
    let Some((fixed, text)) = ctx.decode(raw, path, Some(row)) else {
        return ("\\s{gap}".to_string(), None);
    };
    if fixed != raw {
        explained.push(Code::M06);
    }
    if text.starts_with('\0') {
        return ("\\s{gap}".to_string(), None);
    }
    let (v3_dead, v4_dead) = rule.dead(layer, raw, &text, ctx);
    let out = if v3_dead {
        Out::Dead(text.clone())
    } else {
        Out::Text(raw.to_string())
    };
    let written = match ctx.dead_def(&text) {
        Some(def) if v4_dead => {
            ctx.used.insert((rule.platform.name(), text.clone()));
            dead_token(&def.spelling)
        }
        _ => ctx.literal(raw, path, row),
    };
    (written, Some(Cell { out, explained }))
}

/// The rows of a layer, written, with their cells.
type Converted = (Vec<Vec<String>>, Vec<Vec<Option<Cell>>>);

fn layer_rows(rule: &DeadRule, layer: &Layer3, ctx: &mut Ctx) -> Converted {
    let mut written = Vec::new();
    let mut cells = Vec::new();
    for (r, row) in rows(&layer.text).iter().enumerate() {
        let (w, c): (Vec<_>, Vec<_>) = row
            .iter()
            .map(|t| token(rule, &layer.name, t, &layer.path, r + 1, ctx))
            .unzip();
        written.push(w);
        cells.push(c);
    }
    (written, cells)
}

/// The south flicks of a layer from the v3 layer `alt` reached by the same
/// positions. A row of another length, or a position where a role meets
/// anything else, is not aligned: its flicks are dropped (M15).
fn flicks(rule: &DeadRule, main: &Layer3, alt: &Layer3, ctx: &mut Ctx) -> Converted {
    let main_rows = rows(&main.text);
    let alt_rows = rows(&alt.text);
    let mut written = Vec::new();
    let mut cells = Vec::new();
    let mut unaligned: Vec<String> = Vec::new();
    for (r, row) in main_rows.iter().enumerate() {
        let alt_row = alt_rows.get(r).filter(|a| a.len() == row.len());
        let Some(alt_row) = alt_row else {
            unaligned.push(format!("row {}", r + 1));
            written.push(vec![NO_FLICK.to_string(); row.len()]);
            cells.push(vec![None; row.len()]);
            continue;
        };
        let mut w = Vec::new();
        let mut c = Vec::new();
        for (col, (m, a)) in row.iter().zip(alt_row).enumerate() {
            if gap(a) || (special(a) && a == m) {
                w.push(a.clone());
                c.push(None);
            } else if special(a) || special(m) {
                unaligned.push(format!("row {} key {}", r + 1, col + 1));
                w.push(NO_FLICK.to_string());
                c.push(None);
            } else {
                let (t, cell) = token(rule, &alt.name, a, &alt.path, r + 1, ctx);
                w.push(t);
                c.push(cell);
            }
        }
        written.push(w);
        cells.push(c);
    }
    if alt_rows.len() > main_rows.len() {
        unaligned.push(format!("rows after {}", main_rows.len()));
    }
    if !unaligned.is_empty() {
        ctx.defects.add(
            Code::M15,
            &alt.path,
            format!(
                "not aligned with {}: {}; those flicks are dropped",
                main.name,
                unaligned.join(", ")
            ),
        );
    }
    (written, cells)
}

/// The v3 layer whose keys become the south flicks of `name`.
fn flick_source(name: &str) -> Option<&'static str> {
    match name {
        "default" => Some("alt"),
        "shift" => Some("alt+shift"),
        _ => None,
    }
}

fn size(rule: &DeadRule, platform: &TouchPlatform, ctx: &mut Ctx) -> (SizeOut, SizeTrace) {
    let mut out = SizeOut {
        name: platform.size.to_string(),
        layers: Vec::new(),
    };
    let mut trace = SizeTrace {
        name: platform.size.to_string(),
        layers: Vec::new(),
    };
    for layer in &platform.layers {
        if let Some(target) = match layer.name.as_str() {
            "alt" => Some("default"),
            "alt+shift" => Some("shift"),
            _ => None,
        } && !platform.layers.iter().any(|l| l.name == target)
        {
            ctx.defects.add(
                Code::M15,
                &layer.path,
                format!(
                    "{} has no {target} layer for these flicks; they are dropped",
                    platform.name
                ),
            );
        }
    }
    for layer in &platform.layers {
        if layer.name == "alt" || layer.name == "alt+shift" {
            continue;
        }
        let id = if layer.name == "default" {
            "base".to_string()
        } else {
            layer.name.clone()
        };
        let (rows_out, row_cells) = layer_rows(rule, layer, ctx);
        let alt =
            flick_source(&layer.name).and_then(|a| platform.layers.iter().find(|l| l.name == a));
        let (flick_rows, flick_cells) = match alt {
            Some(alt) => {
                let (w, c) = flicks(rule, layer, alt, ctx);
                (Some(w), c)
            }
            None => (None, Vec::new()),
        };
        out.layers.push(TouchLayerOut {
            id: id.clone(),
            rows: rows_out,
            flicks: flick_rows,
        });
        trace.layers.push(TouchLayerTrace {
            id,
            path: layer.path.clone(),
            rows: row_cells,
            flicks: flick_cells,
        });
    }
    (out, trace)
}

/// A touch section as a touch variant, one size per platform, with its
/// trace.
pub fn convert(src: &Source3, section: &TouchSection, ctx: &mut Ctx) -> (TouchOut, TouchTrace) {
    let rule = DeadRule::new(src, section, ctx);
    let mut out = TouchOut {
        name: section.platform.name().to_string(),
        sizes: Vec::new(),
    };
    let mut trace = TouchTrace {
        platform: section.platform,
        sizes: Vec::new(),
    };
    for platform in &section.platforms {
        let (s, t) = size(&rule, platform, ctx);
        out.sizes.push(s);
        trace.sizes.push(t);
    }
    (out, trace)
}
