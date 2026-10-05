//! Forms and layers (`ldml.xml.resolve` step 7), with kbdgen's layer
//! extensions (`ldml.xml.special`).

use kbd_model::{
    BottomRow, Component, ExtraModifierKey, Form, Hardware, HardwareLayer, KeyIndex, ModifierSet,
    Modifiers, ScanCode, TouchLayer, TouchSet,
};

use super::Ctx;
use crate::cldr;
use crate::diag::Result;
use crate::read::read_import;
use crate::special::{check_attrs, kbdgen_children, plain_attr};
use crate::tree::El;

/// A width in thousandths of a key: a plain decimal with at most three
/// fraction digits.
pub(crate) fn parse_width(s: &str) -> Option<u32> {
    let (int, frac) = s.split_once('.').unwrap_or((s, ""));
    if (int.is_empty() && frac.is_empty()) || frac.len() > 3 {
        return None;
    }
    let digits = |t: &str| t.bytes().all(|b| b.is_ascii_digit());
    if !digits(int) || !digits(frac) {
        return None;
    }
    let int: u32 = if int.is_empty() { 0 } else { int.parse().ok()? };
    let frac: u32 = format!("{frac:0<3}").parse().ok()?;
    int.checked_mul(1000)?.checked_add(frac)
}

/// Parses a `modifiers` value: comma-separated sets of space-separated
/// components, `none` alone being the empty set and `other` alone LDML's
/// fallback. The result is sorted and distinct (`ldml.model.modifiers`).
pub fn parse_modifiers(value: &str) -> Result<Vec<ModifierSet>, String> {
    let mut sets = Vec::new();
    for set in value.split(',') {
        let components: Vec<&str> = set.split_ascii_whitespace().collect();
        let parsed = match components.as_slice() {
            [] => return Err("empty modifier set".to_string()),
            ["none"] => ModifierSet::Set(Modifiers::NONE),
            ["other"] => ModifierSet::Other,
            names => {
                let mut m = Modifiers::NONE;
                for name in names {
                    let c = Component::from_name(name).ok_or_else(|| match *name {
                        "none" | "other" => {
                            format!("{name} may not be combined with other components")
                        }
                        _ => format!("unknown modifier component {name}"),
                    })?;
                    if m.contains(c) {
                        return Err(format!("{name} occurs twice in one set"));
                    }
                    m = m.with(c);
                }
                if let Some((a, b)) = m.rejected_combination() {
                    return Err(format!("{} with {} is not allowed", a.name(), b.name()));
                }
                ModifierSet::Set(m)
            }
        };
        if sets.contains(&parsed) {
            return Err("a modifier set occurs twice".to_string());
        }
        sets.push(parsed);
    }
    sets.sort();
    Ok(sets)
}

/// Writes modifier sets canonically: components space-separated in
/// component order, sets separated by `", "`.
pub fn encode_modifiers(sets: &[ModifierSet]) -> String {
    let encoded: Vec<String> = sets
        .iter()
        .map(|s| match s {
            ModifierSet::Other => "other".to_string(),
            ModifierSet::Set(m) if m.is_empty() => "none".to_string(),
            ModifierSet::Set(m) => m
                .components()
                .map(Component::name)
                .collect::<Vec<_>>()
                .join(" "),
        })
        .collect();
    encoded.join(", ")
}

fn scan_codes(el: &El) -> Result<Vec<ScanCode>> {
    el.attr("codes")
        .unwrap_or("")
        .split_ascii_whitespace()
        .map(|c| {
            u8::from_str_radix(c, 16)
                .map_err(|_| el.attr_error("codes", format!("{c} is not a hex scan code")))
        })
        .collect()
}

fn form_rows(form: &El) -> Result<Vec<Vec<ScanCode>>> {
    form.children_named("scanCodes").map(scan_codes).collect()
}

/// The form `id`: the keyboard's own `forms` first, which may override,
/// then the implied forms at the version the keyboard conforms to.
fn form(ctx: &Ctx, root: &El, layers: &El, id: &str) -> Result<Form> {
    if let Some(own) = root
        .child("forms")
        .and_then(|f| f.children_named("form").find(|f| f.attr("id") == Some(id)))
    {
        return Ok(Form {
            id: id.to_string(),
            rows: form_rows(own)?,
        });
    }
    implied_form(ctx.kb.conforms_to, id)
        .ok_or_else(|| layers.attr_error("formId", format!("no form has id {id}")))
}

// [spec:kbdgen:sem:ldml.xml.implied]
/// The implied form `id` from `scanCodes-implied.xml` at the version a
/// keyboard conforming to `version` uses.
pub(crate) fn implied_form(version: u8, id: &str) -> Option<Form> {
    let path = format!("{}/scanCodes-implied.xml", cldr::implied_version(version));
    let source = read_import(&path, None, cldr::file(&path)?.as_bytes()).ok()?;
    let implied = El::from_document(&source.name, &source.document).ok()?;
    let found = implied
        .children_named("form")
        .find(|f| f.attr("id") == Some(id))?;
    Some(Form {
        id: id.to_string(),
        rows: form_rows(found).ok()?,
    })
}

fn rows(ctx: &Ctx, layer: &El, row_name: &str, prefix: Option<&str>) -> Result<Vec<Vec<KeyIndex>>> {
    layer
        .children
        .iter()
        .filter(|r| r.prefix.as_deref() == prefix && r.name == row_name)
        .map(|row| {
            row.attr("keys")
                .unwrap_or("")
                .split_ascii_whitespace()
                .map(|id| {
                    ctx.kb
                        .key_index(id)
                        .ok_or_else(|| row.attr_error("keys", format!("no key has id {id}")))
                })
                .collect()
        })
        .collect()
}

fn hardware_layer(
    ctx: &Ctx,
    layer: &El,
    prefix: Option<&str>,
    row_name: &str,
) -> Result<HardwareLayer> {
    let modifiers = parse_modifiers(layer.attr("modifiers").unwrap_or(""))
        .map_err(|e| layer.attr_error("modifiers", e))?;
    Ok(HardwareLayer {
        id: layer.attr("id").map(str::to_string),
        modifiers,
        rows: rows(ctx, layer, row_name, prefix)?,
    })
}

fn check_fits(layer: &El, form: &Form, rows: &[Vec<KeyIndex>]) -> Result<()> {
    if rows.len() > form.rows.len() {
        return Err(layer.error(format!(
            "{} rows; form {} has {}",
            rows.len(),
            form.id,
            form.rows.len()
        )));
    }
    for (r, (row, form_row)) in rows.iter().zip(&form.rows).enumerate() {
        if row.len() > form_row.len() {
            return Err(layer.error(format!(
                "row {} has {} keys; form {} row {} has {} scan codes",
                r + 1,
                row.len(),
                form.id,
                r + 1,
                form_row.len()
            )));
        }
    }
    Ok(())
}

pub(super) fn extra_key(el: &El) -> Result<ExtraModifierKey> {
    check_attrs(el, &["key"], &["key"])?;
    match el.attr("key") {
        Some("rightCtrl") => Ok(ExtraModifierKey::RightCtrl),
        Some("capsLock") => Ok(ExtraModifierKey::CapsLock),
        Some("B00") => Ok(ExtraModifierKey::B00),
        other => Err(el.attr_error(
            "key",
            format!("{other:?} is not rightCtrl, capsLock or B00"),
        )),
    }
}

fn hardware(ctx: &mut Ctx, root: &El, layers: &El) -> Result<()> {
    let form_id = layers.attr("formId").unwrap_or("");
    let form = form(ctx, root, layers, form_id)?;
    let min_device_width = layers.attr("minDeviceWidth").and_then(|w| w.parse().ok());
    let mut out = Vec::new();
    for layer in layers.children_named("layer") {
        let built = hardware_layer(ctx, layer, None, "row")?;
        check_fits(layer, &form, &built.rows)?;
        out.push(built);
    }
    let prefix = ctx.kbdgen.clone();
    for el in kbdgen_children(layers, prefix.as_deref()) {
        match el.name.as_str() {
            "extraModifier" => {
                let key = extra_key(el)?;
                ctx.kb.windows.extra_modifiers.push(key);
            }
            "layer" => {
                check_attrs(el, &["id", "modifiers"], &["modifiers"])?;
                let built = hardware_layer(ctx, el, prefix.as_deref(), "row")?;
                check_fits(el, &form, &built.rows)?;
                out.push(built);
            }
            _ => {
                return Err(el.error(format!(
                    "{} is not allowed in hardware layers",
                    el.qualified()
                )));
            }
        }
    }
    ctx.kb.hardware = Some(Hardware {
        form,
        min_device_width,
        layers: out,
    });
    Ok(())
}

fn touch(ctx: &mut Ctx, layers: &El) -> Result<TouchSet> {
    let mut set = TouchSet {
        name: None,
        bottom_row: BottomRow::Authored,
        min_device_width: layers.attr("minDeviceWidth").and_then(|w| w.parse().ok()),
        layers: Vec::new(),
        base: 0,
    };
    for layer in layers.children_named("layer") {
        set.layers.push(TouchLayer {
            id: layer.attr("id").unwrap_or("").to_string(),
            rows: rows(ctx, layer, "row", None)?,
        });
    }
    set.base = set
        .layer_index("base")
        .and_then(|b| u16::try_from(b).ok())
        .ok_or_else(|| layers.error("a touch layers element needs a layer with id base"))?;
    let prefix = ctx.kbdgen.clone();
    for el in kbdgen_children(layers, prefix.as_deref()) {
        if el.name != "touchSet" {
            return Err(el.error(format!("{} is not allowed in touch layers", el.qualified())));
        }
        check_attrs(el, &["name", "bottomRow"], &[])?;
        if el.attr("name").is_some() {
            set.name = Some(plain_attr(el, "name")?);
        }
        set.bottom_row = match el.attr("bottomRow") {
            None | Some("host") => BottomRow::Host,
            Some("authored") => BottomRow::Authored,
            Some(other) => {
                let message = format!("{other:?} is not host or authored");
                return Err(el.attr_error("bottomRow", message));
            }
        };
    }
    Ok(set)
}

/// Builds the hardware set and the touch sets, the latter in ascending
/// `minDeviceWidth` with a set that has none first. A keyboard without
/// hardware layers may still bind extra modifiers in its own `special`.
pub(super) fn layers(ctx: &mut Ctx, root: &El) -> Result<()> {
    for layers in root.children_named("layers") {
        if layers.attr("formId") == Some("touch") {
            let set = touch(ctx, layers)?;
            ctx.kb.touch.push(set);
        } else {
            hardware(ctx, root, layers)?;
        }
    }
    ctx.kb.touch.sort_by_key(|s| s.min_device_width);
    Ok(())
}
