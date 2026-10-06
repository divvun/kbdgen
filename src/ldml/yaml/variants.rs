//! Hardware and touch variants (`ldml.yaml.hardware`, `ldml.yaml.touch`):
//! parsing, `inherits`, and the checks that need a whole variant.

use kbd_ldml::parse_modifiers;
use kbd_model::{BottomRow, Direction, ExtraModifierKey, Form, ModifierSet};
use serde_yaml::Value;

use super::error::{At, Result};
use super::node::{Fields, entries, integer, list, string};
use super::schema::{
    FlickRows, FormSpec, HardwareVariant, LayerSpec, LongPressEntry, Size, SpaceSpec,
    TouchLayerSpec, TouchVariant, long_press,
};
use super::text::Strings;
use super::tokens::{Token, rows, token};

pub const HARDWARE_VARIANTS: [&str; 6] = [
    "default", "windows", "macOS", "chromeOS", "linux", "android",
];
pub const TOUCH_VARIANTS: [&str; 3] = ["default", "iOS", "android"];
const IMPLIED_FORMS: [&str; 5] = ["iso", "us", "jis", "ks", "abnt2"];

/// The default `minDeviceWidth` of a size name, in millimetres
/// (`ldml.yaml.touch`); `None` inside means the size has no width.
pub fn default_width(name: &str) -> Option<Option<u16>> {
    match name {
        "phone" => Some(None),
        "tablet" => Some(Some(95)),
        "tablet-large" => Some(Some(190)),
        _ => None,
    }
}

struct RawHardware {
    variant: HardwareVariant,
    inherits: Option<(String, At)>,
    form: bool,
    implied: bool,
    extra: bool,
}

fn modifier_sets(key: &str, at: &At) -> Result<Vec<ModifierSet>> {
    parse_modifiers(key).map_err(|e| at.error(format!("{key}: {e}")))
}

fn form(value: &Value, at: &At) -> Result<FormSpec> {
    if let Value::String(id) = value {
        if IMPLIED_FORMS.contains(&id.as_str()) {
            return Ok(FormSpec::Implied(id.clone()));
        }
        return Err(at.error(format!(
            "{id} is not iso, us, jis, ks or abnt2; a custom form is {{id, rows}}"
        )));
    }
    let mut f = Fields::new(value, at)?;
    let (id, id_at) = f.require("id")?;
    let id = string(id, &id_at)?.to_string();
    if IMPLIED_FORMS.contains(&id.as_str()) || id == "touch" {
        return Err(id_at.error(format!("a custom form may not be named {id}")));
    }
    let (rows_value, rows_at) = f.require("rows")?;
    let mut rows = Vec::new();
    for (row, ra) in list(rows_value, &rows_at)? {
        let codes = string(row, &ra)?
            .split_ascii_whitespace()
            .map(|c| {
                u8::from_str_radix(c, 16)
                    .ok()
                    .filter(|_| c.len() == 2)
                    .ok_or_else(|| ra.error(format!("{c} is not a two-digit hex scan code")))
            })
            .collect::<Result<Vec<_>>>()?;
        rows.push(codes);
    }
    f.finish()?;
    Ok(FormSpec::Custom(Form { id, rows }))
}

// [spec:kbdgen:def:ldml.yaml.native]
/// `extraModifiers`: up to three distinct keys from `rightCtrl`,
/// `capsLock` and `B00`, in order, the *i*-th binding `extra`*i*.
fn extra_modifiers(value: &Value, at: &At) -> Result<Vec<ExtraModifierKey>> {
    let mut out = Vec::new();
    for (v, a) in list(value, at)? {
        let key = match string(v, &a)? {
            "rightCtrl" => ExtraModifierKey::RightCtrl,
            "capsLock" => ExtraModifierKey::CapsLock,
            "B00" => ExtraModifierKey::B00,
            other => return Err(a.error(format!("{other} is not rightCtrl, capsLock or B00"))),
        };
        if out.contains(&key) {
            return Err(a.error("an extra modifier key occurs twice"));
        }
        out.push(key);
    }
    if out.len() > 3 {
        return Err(at.error("at most three extra modifiers"));
    }
    Ok(out)
}

fn raw_hardware(name: &str, value: &Value, at: &At, strings: &Strings) -> Result<RawHardware> {
    let mut f = Fields::new(value, at)?;
    let mut raw = RawHardware {
        variant: HardwareVariant {
            name: name.to_string(),
            at: at.clone(),
            form: FormSpec::Implied("iso".to_string()),
            implied: true,
            layers: Vec::new(),
            space: Vec::new(),
            extra_modifiers: Vec::new(),
        },
        inherits: None,
        form: false,
        implied: false,
        extra: false,
    };
    if let Some((v, a)) = f.take("form") {
        raw.variant.form = form(v, &a)?;
        raw.form = true;
    }
    if let Some((v, a)) = f.take("inherits") {
        raw.inherits = Some((string(v, &a)?.to_string(), a));
    }
    if let Some((v, a)) = f.take("impliedLayers") {
        raw.variant.implied = match string(v, &a)? {
            "macOS" => true,
            "none" => false,
            other => return Err(a.error(format!("{other} is neither macOS nor none"))),
        };
        raw.implied = true;
    }
    if let Some((v, a)) = f.take("layers") {
        for (key, rows_value, la) in entries(v, &a)? {
            raw.variant.layers.push(LayerSpec {
                key: key.to_string(),
                sets: modifier_sets(key, &la)?,
                rows: rows(string(rows_value, &la)?, strings, &la)?,
                at: la,
            });
        }
    }
    if let Some((v, a)) = f.take("space") {
        for (key, t, sa) in entries(v, &a)? {
            let text = string(t, &sa)?;
            if text.split_ascii_whitespace().count() != 1 {
                return Err(sa.error("a space entry is one token"));
            }
            raw.variant.space.push(SpaceSpec {
                key: key.to_string(),
                sets: modifier_sets(key, &sa)?,
                token: token(text.trim(), strings, &sa)?,
                at: sa,
            });
        }
    }
    if let Some((v, a)) = f.take("extraModifiers") {
        raw.variant.extra_modifiers = extra_modifiers(v, &a)?;
        raw.extra = true;
    }
    f.finish()?;
    Ok(raw)
}

fn merge_layers(base: &mut Vec<LayerSpec>, own: &[LayerSpec]) {
    for layer in own {
        match base.iter_mut().find(|l| l.sets == layer.sets) {
            Some(slot) => *slot = layer.clone(),
            None => base.push(layer.clone()),
        }
    }
}

fn resolve_hardware(
    raws: &[RawHardware],
    index: usize,
    stack: &mut Vec<usize>,
) -> Result<HardwareVariant> {
    let Some(raw) = raws.get(index) else {
        return Err(At::file("").error("no such variant"));
    };
    let Some((parent, parent_at)) = &raw.inherits else {
        return Ok(raw.variant.clone());
    };
    let p = raws
        .iter()
        .position(|r| &r.variant.name == parent)
        .ok_or_else(|| parent_at.error(format!("there is no hardware variant {parent}")))?;
    if stack.contains(&p) || p == index {
        return Err(parent_at.error(format!("inherits {parent} forms a cycle")));
    }
    stack.push(index);
    let mut variant = resolve_hardware(raws, p, stack)?;
    stack.pop();
    variant.name = raw.variant.name.clone();
    variant.at = raw.variant.at.clone();
    if raw.form {
        variant.form = raw.variant.form.clone();
    }
    if raw.implied {
        variant.implied = raw.variant.implied;
    }
    if raw.extra {
        variant.extra_modifiers = raw.variant.extra_modifiers.clone();
    }
    merge_layers(&mut variant.layers, &raw.variant.layers);
    for space in &raw.variant.space {
        match variant.space.iter_mut().find(|s| s.sets == space.sets) {
            Some(slot) => *slot = space.clone(),
            None => variant.space.push(space.clone()),
        }
    }
    Ok(variant)
}

// [spec:kbdgen:syn:ldml.yaml.modifier-names]
// [spec:kbdgen:def:ldml.yaml.native]
/// Fails when two layer keys of a variant name equal or overlapping sets,
/// other than native-only sets overlapping each other, when two layers use
/// `other`, and when a `space` entry names sets that no layer has.
fn check_sets(variant: &HardwareVariant) -> Result<()> {
    let all: Vec<(ModifierSet, &LayerSpec)> = variant
        .layers
        .iter()
        .flat_map(|l| l.sets.iter().map(move |s| (*s, l)))
        .collect();
    for (i, (a, la)) in all.iter().enumerate() {
        for (b, lb) in all.iter().skip(i + 1) {
            if std::ptr::eq(*la, *lb) {
                continue;
            }
            let clash = match (a, b) {
                (ModifierSet::Other, ModifierSet::Other) => true,
                (ModifierSet::Set(x), ModifierSet::Set(y)) => {
                    x.overlaps(*y) && !(x.is_native() && y.is_native())
                }
                _ => false,
            };
            if clash {
                return Err(lb.at.error(format!(
                    "{} overlaps {} of layer {}",
                    kbd_ldml::encode_modifiers(&[*b]),
                    kbd_ldml::encode_modifiers(&[*a]),
                    la.key
                )));
            }
        }
    }
    for space in &variant.space {
        if !variant.layers.iter().any(|l| l.sets == space.sets) {
            return Err(space.at.error(format!("no layer is {}", space.key)));
        }
    }
    Ok(())
}

// [spec:kbdgen:def:ldml.yaml.native]
/// Fails when a layer of the resolved variant uses `extra`*n* and
/// `extraModifiers` has fewer than *n* keys.
fn check_extra(variant: &HardwareVariant) -> Result<()> {
    let bound = variant.extra_modifiers.len();
    for layer in &variant.layers {
        let used = layer
            .sets
            .iter()
            .filter_map(|set| match set {
                ModifierSet::Set(m) => m.components().filter_map(|c| c.extra_number()).max(),
                ModifierSet::Other => None,
            })
            .max();
        if let Some(n) = used.filter(|n| usize::from(*n) > bound) {
            return Err(layer.at.error(format!(
                "layer {} uses extra{n}, but extraModifiers binds {bound} key(s), so nothing binds extra{n}",
                layer.key
            )));
        }
    }
    Ok(())
}

// [spec:kbdgen:def:ldml.yaml.hardware+1]
/// The hardware variants, each with `inherits` resolved: form,
/// `impliedLayers` and `extraModifiers` replaced when given, and layers
/// and `space` entries replaced key by key, keys compared as sets.
pub fn hardware(value: &Value, at: &At, strings: &Strings) -> Result<Vec<HardwareVariant>> {
    let mut raws = Vec::new();
    for (name, v, a) in entries(value, at)? {
        if !HARDWARE_VARIANTS.contains(&name) {
            return Err(a.error(format!(
                "{name} is not a hardware variant; one of {}",
                HARDWARE_VARIANTS.join(", ")
            )));
        }
        raws.push(raw_hardware(name, v, &a, strings)?);
    }
    let mut out = Vec::new();
    for i in 0..raws.len() {
        let variant = resolve_hardware(&raws, i, &mut Vec::new())?;
        check_sets(&variant)?;
        check_extra(&variant)?;
        out.push(variant);
    }
    Ok(out)
}

struct RawTouch {
    variant: TouchVariant,
    inherits: Option<(String, At)>,
}

fn flick_rows(value: &Value, at: &At, strings: &Strings) -> Result<Vec<FlickRows>> {
    let mut out = Vec::new();
    for (key, v, a) in entries(value, at)? {
        let directions = key
            .split_ascii_whitespace()
            .map(|d| {
                Direction::from_name(d).ok_or_else(|| a.error(format!("{d} is not a direction")))
            })
            .collect::<Result<Vec<_>>>()?;
        if directions.is_empty() {
            return Err(a.error("a flick names at least one direction"));
        }
        out.push(FlickRows {
            key: key.to_string(),
            directions,
            rows: rows(string(v, &a)?, strings, &a)?,
            at: a,
        });
    }
    Ok(out)
}

fn touch_layer(id: &str, value: &Value, at: &At, strings: &Strings) -> Result<TouchLayerSpec> {
    if let Value::String(text) = value {
        return Ok(TouchLayerSpec {
            id: id.to_string(),
            rows: rows(text, strings, at)?,
            flicks: Vec::new(),
            at: at.clone(),
        });
    }
    let mut f = Fields::new(value, at)?;
    let (r, ra) = f.require("rows")?;
    let layer = TouchLayerSpec {
        id: id.to_string(),
        rows: rows(string(r, &ra)?, strings, &ra)?,
        flicks: match f.take("flicks") {
            Some((v, a)) => flick_rows(v, &a, strings)?,
            None => Vec::new(),
        },
        at: ra,
    };
    f.finish()?;
    Ok(layer)
}

fn size(name: &str, value: &Value, at: &At, strings: &Strings) -> Result<Size> {
    let mut f = Fields::new(value, at)?;
    let min_device_width = match f.take("minDeviceWidth") {
        Some((v, a)) => {
            let w = integer(v, &a)?;
            Some(
                u16::try_from(w)
                    .ok()
                    .filter(|w| (1..=999).contains(w))
                    .ok_or_else(|| a.error(format!("{w} is not a width from 1 to 999 mm")))?,
            )
        }
        None => default_width(name).ok_or_else(|| {
            at.error(format!(
                "size {name} needs minDeviceWidth; only phone, tablet and tablet-large have one by default"
            ))
        })?,
    };
    let bottom_row = match f.take("bottomRow") {
        None => BottomRow::Host,
        Some((v, a)) => match string(v, &a)? {
            "host" => BottomRow::Host,
            "authored" => BottomRow::Authored,
            other => return Err(a.error(format!("{other} is neither host nor authored"))),
        },
    };
    let (layers_value, layers_at) = f.require("layers")?;
    let layers = entries(layers_value, &layers_at)?
        .into_iter()
        .map(|(id, v, a)| touch_layer(id, v, &a, strings))
        .collect::<Result<Vec<_>>>()?;
    if !layers.iter().any(|l| l.id == "base") {
        return Err(layers_at.error("every size has a base layer"));
    }
    f.finish()?;
    Ok(Size {
        name: name.to_string(),
        min_device_width,
        bottom_row,
        layers,
        at: at.clone(),
    })
}

fn raw_touch(name: &str, value: &Value, at: &At, strings: &Strings) -> Result<RawTouch> {
    let mut f = Fields::new(value, at)?;
    let inherits = match f.take("inherits") {
        Some((v, a)) => Some((string(v, &a)?.to_string(), a)),
        None => None,
    };
    let lp = match f.take("longPress") {
        Some((v, a)) => long_press(v, &a, strings)?,
        None => Vec::new(),
    };
    let mut sizes = Vec::new();
    if let Some((v, a)) = f.take("sizes") {
        for (size_name, sv, sa) in entries(v, &a)? {
            sizes.push(size(size_name, sv, &sa, strings)?);
        }
    }
    f.finish()?;
    Ok(RawTouch {
        variant: TouchVariant {
            name: name.to_string(),
            at: at.clone(),
            long_press: lp,
            sizes,
        },
        inherits,
    })
}

// [spec:kbdgen:def:ldml.yaml.touch+1]
fn merge_long_press(base: &mut Vec<LongPressEntry>, own: &[LongPressEntry]) {
    for entry in own {
        match base.iter_mut().find(|e| e.output == entry.output) {
            Some(slot) => *slot = entry.clone(),
            None => base.push(entry.clone()),
        }
    }
}

fn resolve_touch(raws: &[RawTouch], index: usize, stack: &mut Vec<usize>) -> Result<TouchVariant> {
    let Some(raw) = raws.get(index) else {
        return Err(At::file("").error("no such variant"));
    };
    let Some((parent, parent_at)) = &raw.inherits else {
        return Ok(raw.variant.clone());
    };
    let p = raws
        .iter()
        .position(|r| &r.variant.name == parent)
        .ok_or_else(|| parent_at.error(format!("there is no touch variant {parent}")))?;
    if stack.contains(&p) || p == index {
        return Err(parent_at.error(format!("inherits {parent} forms a cycle")));
    }
    stack.push(index);
    let mut variant = resolve_touch(raws, p, stack)?;
    stack.pop();
    variant.name = raw.variant.name.clone();
    variant.at = raw.variant.at.clone();
    merge_long_press(&mut variant.long_press, &raw.variant.long_press);
    for size in &raw.variant.sizes {
        match variant.sizes.iter_mut().find(|s| s.name == size.name) {
            Some(slot) => *slot = size.clone(),
            None => variant.sizes.push(size.clone()),
        }
    }
    Ok(variant)
}

// [spec:kbdgen:def:ldml.yaml.touch+1]
/// The touch variants, each with `inherits` resolved: the parent's sizes
/// replaced size by size and its `longPress` entries output by output.
/// Every size has a `base` layer, and a variant's sizes have distinct
/// widths.
pub fn touch(value: &Value, at: &At, strings: &Strings) -> Result<Vec<TouchVariant>> {
    let mut raws = Vec::new();
    for (name, v, a) in entries(value, at)? {
        if !TOUCH_VARIANTS.contains(&name) {
            return Err(a.error(format!(
                "{name} is not a touch variant; one of {}",
                TOUCH_VARIANTS.join(", ")
            )));
        }
        raws.push(raw_touch(name, v, &a, strings)?);
    }
    let mut out = Vec::new();
    for i in 0..raws.len() {
        let variant = resolve_touch(&raws, i, &mut Vec::new())?;
        for (j, size) in variant.sizes.iter().enumerate() {
            if let Some(other) = variant
                .sizes
                .iter()
                .take(j)
                .find(|s| s.min_device_width == size.min_device_width)
            {
                return Err(size.at.error(format!(
                    "sizes {} and {} have the same minDeviceWidth",
                    other.name, size.name
                )));
            }
        }
        out.push(variant);
    }
    Ok(out)
}

/// Whether a token may stand in a hardware row.
pub fn check_hardware_token(token: &Token, at: &At) -> Result<()> {
    if token.touch_only() {
        return Err(at.error("roles, \\l{} and \\s{\"x\":w} are touch-only tokens"));
    }
    Ok(())
}
