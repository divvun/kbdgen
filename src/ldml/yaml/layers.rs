//! Hardware and touch layers of a host document: rows checked against the
//! form, tokens turned into key ids, and the implied caps layers.

use kbd_model::{BottomRow, Component, ExtraModifierKey, Form, ModifierSet, Modifiers};

use super::error::{At, Result};
use super::keys::{Ctx, KeyTable};
use super::schema::{FormSpec, HardwareVariant, LongPressEntry, Size, TouchVariant};
use super::text::plain_text;
use super::tokens::Token;
use super::variants::check_hardware_token;

/// A layer: its modifier sets, sorted, and rows of keys. A position past
/// a row's end has no key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grid<K> {
    pub sets: Vec<ModifierSet>,
    pub rows: Vec<Vec<K>>,
}

/// The hardware set of a host document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HardwareOut {
    pub form: Form,
    pub custom: bool,
    pub layers: Vec<Grid<String>>,
    pub extra_modifiers: Vec<ExtraModifierKey>,
    pub implied: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TouchOut {
    pub name: String,
    pub min_device_width: Option<u16>,
    pub bottom_row: BottomRow,
    pub layers: Vec<(String, Vec<Vec<String>>)>,
}

/// The rows of a form that hold characters: all but a final row that is
/// the space bar alone.
pub fn character_rows(form: &Form) -> (&[Vec<u8>], bool) {
    match form.rows.split_last() {
        Some((last, rest)) if last.as_slice() == [0x39] => (rest, true),
        _ => (&form.rows, false),
    }
}

fn applies(set: ModifierSet) -> Option<Modifiers> {
    match set {
        ModifierSet::Set(m)
            if !m.contains(Component::Caps) && !m.contains(Component::Shift) && !m.is_native() =>
        {
            Some(m)
        }
        _ => None,
    }
}

fn holds(sets: &[ModifierSet], m: Modifiers) -> bool {
    sets.iter().any(|s| match s {
        ModifierSet::Set(x) => !x.is_native() && x.overlaps(m),
        ModifierSet::Other => false,
    })
}

fn add_set<K>(layer: &mut Grid<K>, m: Modifiers) {
    let set = ModifierSet::Set(m);
    if !layer.sets.contains(&set) {
        layer.sets.push(set);
        layer.sets.sort();
    }
}

// [spec:kbdgen:sem:ldml.yaml.implied-layers+1]
// [spec:kbdgen:sem:ldml.scope.macos-rules]
/// `impliedLayers: macOS`: for each authored set S without `caps` or
/// `shift` that is not native-only, with Sh = S + `shift`:
///
/// 1. if Sh is authored and S + `caps shift` is not, Sh's layer also gets
///    S + `caps shift`, so Caps+Shift gives uppercase;
/// 2. if neither S + `caps` nor Sh is authored, S's layer also gets
///    S + `caps`;
/// 3. if S + `caps` is not authored and Sh is, a layer S + `caps` is
///    created from S's keys, each replaced by Sh's key at the same
///    position when `upper(s, sh)`. A created layer equal to an existing
///    one becomes an extra set of it.
///
/// A set counts as authored when an authored set that is not native-only
/// overlaps it, so an implied set never overlaps an authored one. Created
/// layers follow the authored ones. Import re-derives with the same
/// function, so both directions agree.
pub fn imply<K: Clone + PartialEq>(layers: &mut Vec<Grid<K>>, upper: &dyn Fn(&K, &K) -> bool) {
    let authored: Vec<ModifierSet> = layers.iter().flat_map(|l| l.sets.clone()).collect();
    for s in authored.iter().filter_map(|s| applies(*s)) {
        let sh = s.with(Component::Shift);
        let caps = s.with(Component::Caps);
        let caps_shift = caps.with(Component::Shift);
        let has_sh = authored.contains(&ModifierSet::Set(sh));
        let has_caps = holds(&authored, caps);
        if has_sh && !holds(&authored, caps_shift) {
            let target = layers
                .iter_mut()
                .find(|l| l.sets.contains(&ModifierSet::Set(sh)));
            if let Some(layer) = target {
                add_set(layer, caps_shift);
            }
        }
        if has_caps {
            continue;
        }
        let find = |m: Modifiers| {
            layers
                .iter()
                .position(|l| l.sets.contains(&ModifierSet::Set(m)))
        };
        let (Some(si), sh_index) = (find(s), find(sh)) else {
            continue;
        };
        let Some(sh_index) = sh_index else {
            if let Some(layer) = layers.get_mut(si) {
                add_set(layer, caps);
            }
            continue;
        };
        let (Some(base), Some(shifted)) = (layers.get(si), layers.get(sh_index)) else {
            continue;
        };
        let rows: Vec<Vec<K>> = base
            .rows
            .iter()
            .enumerate()
            .map(|(r, row)| {
                row.iter()
                    .enumerate()
                    .map(|(c, k)| match shifted.rows.get(r).and_then(|x| x.get(c)) {
                        Some(sk) if upper(k, sk) => sk.clone(),
                        _ => k.clone(),
                    })
                    .collect()
            })
            .collect();
        match layers.iter_mut().find(|l| l.rows == rows) {
            Some(same) => add_set(same, caps),
            None => layers.push(Grid {
                sets: vec![ModifierSet::Set(caps)],
                rows,
            }),
        }
    }
}

/// Whether `shifted` is the uppercase of `base`, both plain text: the
/// position test of `ldml.yaml.implied-layers` rule 3.
pub fn is_uppercase_of(base: &str, shifted: &str) -> bool {
    base.to_uppercase() == shifted && shifted != base
}

/// Positions as written to LDML: trailing positions with no key left
/// out, others with no key given the `gap` key, and trailing empty rows
/// left out. A row LDML cannot leave empty holds one `gap`.
fn finish_rows(rows: Vec<Vec<Option<String>>>) -> Vec<Vec<String>> {
    let mut out: Vec<Vec<String>> = rows
        .into_iter()
        .map(|row| {
            let end = row.iter().rposition(Option::is_some).map_or(0, |i| i + 1);
            row.into_iter()
                .take(end)
                .map(|k| k.unwrap_or_else(|| "gap".to_string()))
                .collect()
        })
        .collect();
    while out.last().is_some_and(Vec::is_empty) {
        out.pop();
    }
    for row in &mut out {
        if row.is_empty() {
            row.push("gap".to_string());
        }
    }
    out
}

/// The scan code of `B00`, which an `extraModifiers` entry can bind.
const B00_SCAN_CODE: u8 = 0x56;

// [spec:kbdgen:def:ldml.yaml.native]
/// Fails when a token may not stand at a hardware position: a touch-only
/// token anywhere, or anything but `\u{0}` at `B00` once `extraModifiers`
/// binds it.
fn check_position(token: &Token, code: u8, b00_bound: bool, at: &At) -> Result<()> {
    check_hardware_token(token, at)?;
    if b00_bound && code == B00_SCAN_CODE && *token != Token::NoKey {
        return Err(at.error("extraModifiers binds B00, so every B00 position must be \\u{0}"));
    }
    Ok(())
}

// [spec:kbdgen:def:ldml.yaml.native]
/// The note for a row of an `iso` layer with one token more than the form
/// has: the same row of `abnt2`, which adds the 49th key (`B11`).
fn abnt2_note(form: &Form, custom: bool, r: usize, tokens: usize, conforms_to: u8) -> &'static str {
    let abnt2 = (!custom && form.id == "iso")
        .then(|| kbd_ldml::implied_form(conforms_to, "abnt2"))
        .flatten();
    let fits = abnt2.is_some_and(|abnt2| {
        let iso = character_rows(form).0.get(r).map(Vec::len);
        let wide = character_rows(&abnt2).0.get(r).map(Vec::len);
        wide == Some(tokens) && iso.is_some_and(|n| n + 1 == tokens)
    });
    if fits {
        "; the 49th key needs form: abnt2"
    } else {
        ""
    }
}

fn form(spec: &FormSpec, conforms_to: u8, at: &At) -> Result<(Form, bool)> {
    match spec {
        FormSpec::Implied(id) => kbd_ldml::implied_form(conforms_to, id)
            .map(|f| (f, false))
            .ok_or_else(|| at.error(format!("form {id} is not implied at {conforms_to}"))),
        FormSpec::Custom(form) => Ok((form.clone(), true)),
    }
}

// [spec:kbdgen:req:ldml.yaml.hardware.rows+1]
/// The hardware set of a variant. Each layer has exactly the form's
/// character rows, each with exactly its row's scan-code count, plus an
/// optional row holding the space position alone; without it the layer
/// gets its `space` entry, else `\s{space}`. Layers are taken in the
/// order the document holds them, those only kbdgen's namespace can hold
/// last, which fixes the order keys get their ids in.
pub fn hardware(
    variant: &HardwareVariant,
    table: &mut KeyTable,
    long_press: &[LongPressEntry],
    conforms_to: u8,
) -> Result<HardwareOut> {
    let (form, custom) = form(&variant.form, conforms_to, &variant.at)?;
    let (char_rows, has_space) = character_rows(&form);
    let b00_bound = variant.extra_modifiers.contains(&ExtraModifierKey::B00);
    let ctx = Ctx {
        long_press,
        touch: None,
    };
    let mut layers = Vec::new();
    let (plain, native): (Vec<_>, Vec<_>) = variant
        .layers
        .iter()
        .partition(|l| !kbd_ldml::is_kbdgen_layer(&l.sets));
    for layer in plain.into_iter().chain(native) {
        let n = layer.rows.len();
        let space_row = has_space && n == char_rows.len() + 1;
        if n != char_rows.len() && !space_row {
            return Err(layer.at.error(format!(
                "layer {} has {n} rows; form {} has {} character rows{}",
                layer.key,
                form.id,
                char_rows.len(),
                if has_space {
                    " and an optional space row"
                } else {
                    ""
                }
            )));
        }
        let mut rows: Vec<Vec<Option<String>>> = Vec::new();
        for (r, (row, codes)) in layer.rows.iter().zip(char_rows).enumerate() {
            if row.len() != codes.len() {
                return Err(layer.at.row(r).error(format!(
                    "layer {} row {} has {} tokens; form {} row {} has {} scan codes{}",
                    layer.key,
                    r + 1,
                    row.len(),
                    form.id,
                    r + 1,
                    codes.len(),
                    abnt2_note(&form, custom, r, row.len(), conforms_to)
                )));
            }
            let mut ids = Vec::new();
            for (c, (token, code)) in row.iter().zip(codes).enumerate() {
                let at = layer.at.row(r).token(c);
                check_position(token, *code, b00_bound, &at)?;
                ids.push(table.key(token, &ctx, &[], &at)?);
            }
            rows.push(ids);
        }
        if has_space {
            let r = char_rows.len();
            let (token, at) = match layer.rows.get(r) {
                Some(row) if row.len() != 1 => {
                    return Err(layer.at.row(r).error(format!(
                        "the space row of layer {} has {} tokens, not 1",
                        layer.key,
                        row.len()
                    )));
                }
                Some(row) => (
                    row.first().cloned().unwrap_or(Token::NoKey),
                    layer.at.row(r).token(0),
                ),
                None => match variant.space.iter().find(|s| s.sets == layer.sets) {
                    Some(s) => (s.token.clone(), s.at.clone()),
                    None => (Token::Space(None), layer.at.clone()),
                },
            };
            check_hardware_token(&token, &at)?;
            rows.push(vec![table.key(&token, &ctx, &[], &at)?]);
        }
        layers.push(Grid {
            sets: layer.sets.clone(),
            rows: finish_rows(rows),
        });
    }
    if variant.implied {
        let text = |id: &String| table.output_of(id).and_then(plain_text);
        imply(&mut layers, &|s, sh| match (text(s), text(sh)) {
            (Some(s), Some(sh)) => is_uppercase_of(&s, &sh),
            _ => false,
        });
    }
    Ok(HardwareOut {
        form,
        custom,
        layers,
        extra_modifiers: variant.extra_modifiers.clone(),
        implied: variant.implied,
    })
}

fn touch_size(
    size: &Size,
    table: &mut KeyTable,
    long_press: &[LongPressEntry],
) -> Result<TouchOut> {
    let layer_ids: Vec<&str> = size.layers.iter().map(|l| l.id.as_str()).collect();
    let mut layers = Vec::new();
    for layer in &size.layers {
        let ctx = Ctx {
            long_press,
            touch: Some((layer.id.as_str(), layer_ids.clone())),
        };
        for f in &layer.flicks {
            if f.rows.len() != layer.rows.len() {
                return Err(f.at.error(format!(
                    "flick {} has {} rows; layer {} has {}",
                    f.key,
                    f.rows.len(),
                    layer.id,
                    layer.rows.len()
                )));
            }
        }
        let mut rows = Vec::new();
        for (r, row) in layer.rows.iter().enumerate() {
            let mut ids = Vec::new();
            for (c, token) in row.iter().enumerate() {
                let at = layer.at.row(r).token(c);
                if *token == Token::NoKey {
                    return Err(at.error("\\u{0} is for hardware and flick rows; use \\s{gap}"));
                }
                let flicks = flicks_at(layer, r, c, row.len(), token)?;
                if let Some(id) = table.key(token, &ctx, &flicks, &at)? {
                    ids.push(id);
                }
            }
            rows.push(ids);
        }
        layers.push((layer.id.clone(), rows));
    }
    Ok(TouchOut {
        name: size.name.clone(),
        min_device_width: size.min_device_width,
        bottom_row: size.bottom_row,
        layers,
    })
}

// [spec:kbdgen:sem:ldml.yaml.touch.flicks]
/// The flick targets of the key at (`r`, `c`): for each flick entry, the
/// token at the same position, unless it is `\u{0}`, a gap, or a role
/// token equal to the main row's.
fn flicks_at(
    layer: &super::schema::TouchLayerSpec,
    r: usize,
    c: usize,
    width: usize,
    main: &Token,
) -> Result<Vec<(Vec<kbd_model::Direction>, Token, At)>> {
    let mut out = Vec::new();
    for f in &layer.flicks {
        let Some(row) = f.rows.get(r) else {
            continue;
        };
        if row.len() != width {
            return Err(f.at.row(r).error(format!(
                "flick {} row {} has {} tokens; the layer row has {width}",
                f.key,
                r + 1,
                row.len()
            )));
        }
        let at = f.at.row(r).token(c);
        match row.get(c) {
            None | Some(Token::NoKey | Token::Gap(_)) => {}
            Some(t @ Token::Role(..)) if t == main => {}
            Some(Token::Role(..)) => {
                return Err(at.error("a role token in a flick row equals the main row's"));
            }
            Some(t) => out.push((f.directions.clone(), t.clone(), at)),
        }
    }
    Ok(out)
}

// [spec:kbdgen:def:ldml.yaml.touch+1]
/// The touch sets of a variant in ascending `minDeviceWidth`, the size
/// without one first. A variant's `longPress` entries replace the global
/// ones with the same output.
pub fn touch(
    variant: &TouchVariant,
    table: &mut KeyTable,
    global: &[LongPressEntry],
) -> Result<Vec<TouchOut>> {
    let mut long_press: Vec<LongPressEntry> = global
        .iter()
        .filter(|g| !variant.long_press.iter().any(|v| v.output == g.output))
        .cloned()
        .collect();
    long_press.extend(variant.long_press.iter().cloned());
    let mut sizes: Vec<&Size> = variant.sizes.iter().collect();
    sizes.sort_by_key(|s| s.min_device_width);
    sizes
        .into_iter()
        .map(|s| touch_size(s, table, &long_press))
        .collect()
}
