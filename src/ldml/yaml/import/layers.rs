//! Import of layers (`ldml.yaml.import`, Layers and Generated groups):
//! hardware layers become a variant, with the caps sets that
//! `impliedLayers: macOS` recreates left out when re-deriving them
//! reproduces the layers exactly; touch sets become sizes; long press
//! becomes `longPress` entries.

use kbd_ldml::{encode_modifiers, implied_form, is_kbdgen_layer};
use kbd_model::{
    BottomRow, Component, Hardware, HardwareLayer, Keyboard, ModifierSet, Modifiers, TouchSet,
};
use serde_yaml::{Mapping, Value};

use super::keys::{KeyTok, pieces, token_of};
use super::{Doc, Level};
use crate::ldml::yaml::layers::{Grid, character_rows, imply, is_uppercase_of};
use crate::ldml::yaml::text::plain_text;
use crate::ldml::yaml::variants::default_width;

pub fn insert(map: &mut Mapping, key: &str, value: Value) {
    map.insert(Value::String(key.to_string()), value);
}

fn rows_value(rows: Vec<Vec<String>>) -> Value {
    let mut text = String::new();
    for row in rows {
        text.push_str(&row.join(" "));
        text.push('\n');
    }
    Value::String(text)
}

/// Layers in the order resolution stores them: LDML layers, then those
/// only kbdgen's namespace can hold.
fn model_order(grids: &[Grid<u16>]) -> Vec<Grid<u16>> {
    let (plain, native): (Vec<_>, Vec<_>) = grids
        .iter()
        .cloned()
        .partition(|g| !is_kbdgen_layer(&g.sets));
    plain.into_iter().chain(native).collect()
}

fn remove_set(grids: &[Grid<u16>], m: Modifiers) -> Vec<Grid<u16>> {
    let set = ModifierSet::Set(m);
    grids
        .iter()
        .cloned()
        .filter_map(|mut g| {
            g.sets.retain(|s| *s != set);
            (!g.sets.is_empty()).then_some(g)
        })
        .collect()
}

// [spec:kbdgen:sem:ldml.yaml.import]
// [spec:kbdgen:sem:ldml.yaml.implied-layers]
/// The authored layers of a hardware set and whether they use
/// `impliedLayers: macOS`: the layers without every caps set that
/// re-deriving recreates exactly, or all layers and `none` when re-deriving
/// would change them.
pub fn authored_layers(kb: &Keyboard, hw: &Hardware) -> (Vec<Grid<u16>>, bool) {
    let original: Vec<Grid<u16>> = hw
        .layers
        .iter()
        .map(|l| Grid {
            sets: l.modifiers.clone(),
            rows: l.rows.clone(),
        })
        .collect();
    let text = |k: &u16| {
        kb.key(*k)
            .and_then(|key| plain_text(&pieces(kb, &key.output)))
    };
    let upper = |a: &u16, b: &u16| match (text(a), text(b)) {
        (Some(a), Some(b)) => is_uppercase_of(&a, &b),
        _ => false,
    };
    let derive = |grids: &[Grid<u16>]| {
        let mut out = grids.to_vec();
        imply(&mut out, &upper);
        model_order(&out)
    };
    if derive(&original) != original {
        return (original, false);
    }
    let mut candidate = original.clone();
    let bases: Vec<Modifiers> = original
        .iter()
        .flat_map(|g| g.sets.iter())
        .filter_map(|s| match s {
            ModifierSet::Set(m)
                if !m.contains(Component::Caps)
                    && !m.contains(Component::Shift)
                    && !m.is_native() =>
            {
                Some(*m)
            }
            _ => None,
        })
        .collect();
    for s in bases {
        let caps = s.with(Component::Caps);
        let caps_shift = caps.with(Component::Shift);
        for removed in [vec![caps, caps_shift], vec![caps], vec![caps_shift]] {
            let trial = removed
                .iter()
                .fold(candidate.clone(), |g, m| remove_set(&g, *m));
            if trial != candidate && derive(&trial) == original {
                candidate = trial;
                break;
            }
        }
    }
    (candidate, true)
}

/// The tokens of a hardware layer. A gap before the row's last other key,
/// or a row that is one gap, is written `\u{0}`: lowering rebuilds the
/// same row, and a `B00` bound by `extraModifiers` must be `\u{0}`
/// (`ldml.yaml.native`).
fn hardware_rows(
    kb: &Keyboard,
    classes: &[KeyTok],
    hw: &Hardware,
    grid: &Grid<u16>,
) -> Vec<Vec<String>> {
    let (char_rows, has_space) = character_rows(&hw.form);
    let gap = kb.key_index("gap");
    let mut out = Vec::new();
    for (r, codes) in char_rows.iter().enumerate() {
        let row = grid.rows.get(r).map(Vec::as_slice).unwrap_or(&[]);
        let last_key = row.iter().rposition(|k| Some(*k) != gap);
        out.push(
            (0..codes.len())
                .map(|c| match row.get(c) {
                    None => "\\u{0}".to_string(),
                    Some(k)
                        if Some(*k) == gap
                            && (row.len() == 1 || last_key.is_some_and(|l| c < l)) =>
                    {
                        "\\u{0}".to_string()
                    }
                    Some(k) => token_of(kb, classes, *k),
                })
                .collect(),
        );
    }
    if has_space {
        let space = match grid.rows.get(char_rows.len()).and_then(|r| r.first()) {
            Some(k) => token_of(kb, classes, *k),
            None => "\\u{0}".to_string(),
        };
        if space != "\\s{space}" {
            out.push(vec![space]);
        }
    }
    out
}

/// A hardware variant of `doc`, its layers rows of tokens.
pub fn hardware(doc: &Doc, classes: &[KeyTok], level: Level) -> Value {
    let kb = doc.kb;
    let Some(hw) = &kb.hardware else {
        return Value::Null;
    };
    let mut map = Mapping::new();
    if implied_form(kb.conforms_to, &hw.form.id).as_ref() != Some(&hw.form) {
        let mut form = Mapping::new();
        insert(&mut form, "id", Value::String(hw.form.id.clone()));
        let rows: Vec<Value> = hw
            .form
            .rows
            .iter()
            .map(|r| {
                Value::String(
                    r.iter()
                        .map(|c| format!("{c:02X}"))
                        .collect::<Vec<_>>()
                        .join(" "),
                )
            })
            .collect();
        insert(&mut form, "rows", Value::Sequence(rows));
        insert(&mut map, "form", Value::Mapping(form));
    } else if hw.form.id != "iso" {
        insert(&mut map, "form", Value::String(hw.form.id.clone()));
    }
    let (grids, implied) = match level {
        Level::Sugar if doc.ext.implied_layers.as_deref() != Some("none") => {
            authored_layers(kb, hw)
        }
        _ => (
            hw.layers
                .iter()
                .map(|l: &HardwareLayer| Grid {
                    sets: l.modifiers.clone(),
                    rows: l.rows.clone(),
                })
                .collect(),
            false,
        ),
    };
    if !implied {
        insert(&mut map, "impliedLayers", Value::String("none".to_string()));
    }
    let mut layers = Mapping::new();
    for grid in &grids {
        insert(
            &mut layers,
            &encode_modifiers(&grid.sets),
            rows_value(hardware_rows(kb, classes, hw, grid)),
        );
    }
    insert(&mut map, "layers", Value::Mapping(layers));
    let extra = &kb.windows.extra_modifiers;
    if !extra.is_empty() {
        insert(
            &mut map,
            "extraModifiers",
            Value::Sequence(
                extra
                    .iter()
                    .map(|k| Value::String(k.name().to_string()))
                    .collect(),
            ),
        );
    }
    Value::Mapping(map)
}

/// The name a touch set gets as a size: its own, else `phone` without a
/// width and the size name whose default width it has, else `w<width>`.
fn size_name(set: &TouchSet, taken: &[String]) -> String {
    let base = set.name.clone().unwrap_or_else(|| {
        ["phone", "tablet", "tablet-large"]
            .into_iter()
            .find(|n| default_width(n) == Some(set.min_device_width))
            .map_or_else(
                || format!("w{}", set.min_device_width.unwrap_or(0)),
                str::to_string,
            )
    });
    let mut name = base.clone();
    let mut n = 2;
    while taken.contains(&name) {
        name = format!("{base}-{n}");
        n += 1;
    }
    name
}

/// The flicks of a touch layer as `{rows, flicks}`, when every flicked key
/// is made from a token and its flick is its own; otherwise the plain rows.
fn touch_layer(kb: &Keyboard, classes: &[KeyTok], rows: &[Vec<u16>]) -> Value {
    let tokens: Vec<Vec<String>> = rows
        .iter()
        .map(|r| r.iter().map(|k| token_of(kb, classes, *k)).collect())
        .collect();
    let mut directions: Vec<Vec<kbd_model::Direction>> = Vec::new();
    for k in rows.iter().flatten() {
        let made = matches!(classes.get(usize::from(*k)), Some(KeyTok::Made(_)));
        let flick = kb
            .key(*k)
            .and_then(|key| key.flick)
            .and_then(|f| kb.flicks.get(usize::from(f)));
        if let (true, Some(flick)) = (made, flick) {
            for segment in &flick.segments {
                if !directions.contains(&segment.directions) {
                    directions.push(segment.directions.clone());
                }
            }
        }
    }
    if directions.is_empty() {
        return rows_value(tokens);
    }
    let mut flicks = Mapping::new();
    for d in &directions {
        let flick_rows: Vec<Vec<String>> = rows
            .iter()
            .map(|r| {
                r.iter()
                    .map(|k| {
                        let made = matches!(classes.get(usize::from(*k)), Some(KeyTok::Made(_)));
                        kb.key(*k)
                            .and_then(|key| key.flick)
                            .and_then(|f| kb.flicks.get(usize::from(f)))
                            .filter(|_| made)
                            .and_then(|f| f.segments.iter().find(|s| &s.directions == d))
                            .map_or_else(|| "\\u{0}".to_string(), |s| token_of(kb, classes, s.key))
                    })
                    .collect()
            })
            .collect();
        let names: Vec<&str> = d.iter().map(|x| x.name()).collect();
        insert(&mut flicks, &names.join(" "), rows_value(flick_rows));
    }
    let mut map = Mapping::new();
    insert(&mut map, "rows", rows_value(tokens));
    insert(&mut map, "flicks", Value::Mapping(flicks));
    Value::Mapping(map)
}

/// A touch variant of `doc`: one size per touch set, and its own
/// `longPress` entries.
pub fn touch(doc: &Doc, classes: &[KeyTok], long_press: Mapping) -> Value {
    let kb = doc.kb;
    let mut sizes = Mapping::new();
    let mut taken: Vec<String> = Vec::new();
    for set in &kb.touch {
        let name = size_name(set, &taken);
        taken.push(name.clone());
        let mut size = Mapping::new();
        if default_width(&name) != Some(set.min_device_width)
            && let Some(w) = set.min_device_width
        {
            insert(&mut size, "minDeviceWidth", Value::Number(w.into()));
        }
        if set.bottom_row == BottomRow::Authored {
            insert(
                &mut size,
                "bottomRow",
                Value::String("authored".to_string()),
            );
        }
        let mut layers = Mapping::new();
        for layer in &set.layers {
            insert(
                &mut layers,
                &layer.id,
                touch_layer(kb, classes, &layer.rows),
            );
        }
        insert(&mut size, "layers", Value::Mapping(layers));
        insert(&mut sizes, &name, Value::Mapping(size));
    }
    let mut map = Mapping::new();
    if !long_press.is_empty() {
        insert(&mut map, "longPress", Value::Mapping(long_press));
    }
    insert(&mut map, "sizes", Value::Mapping(sizes));
    Value::Mapping(map)
}
