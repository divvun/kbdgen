//! The key, flick and display tables (`ldml.xml.resolve` step 6).

use kbd_model::{
    DEFAULT_WIDTH, Direction, Display, DisplayTarget, Flick, FlickIndex, FlickSegment, Key,
    KeyIndex, Role, Text,
};

use super::Ctx;
use super::layers::parse_width;
use crate::diag::Result;
use crate::special::{check_attrs, kbdgen_children};
use crate::tree::El;

/// A display whose output target is interned but whose key target waits
/// for the key table.
pub(super) struct PendingDisplay<'a> {
    el: &'a El,
    output: Option<Text>,
}

/// Interns the markers of display outputs, which come first in document
/// order.
pub(super) fn intern_displays<'a>(ctx: &mut Ctx, root: &'a El) -> Result<Vec<PendingDisplay<'a>>> {
    let mut pending = Vec::new();
    for el in root
        .child("displays")
        .iter()
        .flat_map(|d| d.children_named("display"))
    {
        let output = match el.attr("output") {
            Some(_) => Some(ctx.text_attr(el, "output")?),
            None => None,
        };
        pending.push(PendingDisplay { el, output });
    }
    Ok(pending)
}

fn key_index(ctx: &Ctx, el: &El, attribute: &str, id: &str) -> Result<KeyIndex> {
    ctx.kb
        .key_index(id)
        .ok_or_else(|| el.attr_error(attribute, format!("no key has id {id}")))
}

fn key_list(ctx: &Ctx, el: &El, attribute: &str) -> Result<Vec<KeyIndex>> {
    el.attr(attribute)
        .unwrap_or("")
        .split_ascii_whitespace()
        .map(|id| key_index(ctx, el, attribute, id))
        .collect()
}

fn flicks(ctx: &mut Ctx, root: &El) -> Result<()> {
    for el in root
        .child("flicks")
        .iter()
        .flat_map(|f| f.children_named("flick"))
    {
        let mut segments = Vec::new();
        for segment in el.children_named("flickSegment") {
            let directions = segment
                .attr("directions")
                .unwrap_or("")
                .split_ascii_whitespace()
                .map(|d| {
                    Direction::from_name(d).ok_or_else(|| {
                        segment.attr_error("directions", format!("unknown direction {d}"))
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            let key = key_index(ctx, segment, "keyId", segment.attr("keyId").unwrap_or(""))?;
            segments.push(FlickSegment { directions, key });
        }
        ctx.kb.flicks.push(Flick {
            id: el.attr("id").unwrap_or("").to_string(),
            segments,
        });
    }
    Ok(())
}

/// Builds the key table: implied keys first, then the rest in document
/// order, with overrides in place; then flicks, and the references
/// between keys, which may point forwards.
pub(super) fn keys(ctx: &mut Ctx, root: &El) -> Result<()> {
    let Some(keys_el) = root.child("keys") else {
        return Ok(());
    };
    let elements: Vec<&El> = keys_el.children_named("key").collect();
    for el in &elements {
        let width = match el.attr("width") {
            Some(w) => parse_width(w).ok_or_else(|| el.attr_error("width", "not a width"))?,
            None => DEFAULT_WIDTH,
        };
        let mut key = Key::new(el.attr("id").unwrap_or(""), ctx.text_attr(el, "output")?);
        key.gap = el.attr("gap") == Some("true");
        key.stretch = el.attr("stretch") == Some("true");
        key.layer_id = el.attr("layerId").map(str::to_string);
        key.width = width;
        ctx.kb.keys.push(key);
    }
    if KeyIndex::try_from(ctx.kb.keys.len()).is_err() {
        return Err(keys_el.error("more than 65536 keys"));
    }
    flicks(ctx, root)?;
    for (i, el) in elements.iter().enumerate() {
        let long_press = key_list(ctx, el, "longPressKeyIds")?;
        let default = match el.attr("longPressDefaultKeyId") {
            Some(id) => Some(key_index(ctx, el, "longPressDefaultKeyId", id)?),
            None => None,
        };
        if default.is_some_and(|d| !long_press.contains(&d)) {
            return Err(el.attr_error(
                "longPressDefaultKeyId",
                "the default long-press key is not in longPressKeyIds",
            ));
        }
        let multi_tap = key_list(ctx, el, "multiTapKeyIds")?;
        let flick = match el.attr("flickId") {
            Some(id) => {
                let f = ctx
                    .kb
                    .flicks
                    .iter()
                    .position(|f| f.id == id)
                    .ok_or_else(|| el.attr_error("flickId", format!("no flick has id {id}")))?;
                Some(FlickIndex::try_from(f).map_err(|_| el.error("too many flicks"))?)
            }
            None => None,
        };
        if let Some(key) = ctx.kb.keys.get_mut(i) {
            key.long_press = long_press;
            key.long_press_default = default;
            key.multi_tap = multi_tap;
            key.flick = flick;
        }
    }
    let prefix = ctx.kbdgen.clone();
    for role in kbdgen_children(keys_el, prefix.as_deref()) {
        if role.name != "role" {
            return Err(role.error(format!("{} is not allowed in keys", role.qualified())));
        }
        check_attrs(role, &["keyId", "role"], &["keyId", "role"])?;
        let index = key_index(ctx, role, "keyId", role.attr("keyId").unwrap_or(""))?;
        let name = role.attr("role").unwrap_or("");
        let value = Role::from_name(name)
            .ok_or_else(|| role.attr_error("role", format!("unknown role {name}")))?;
        if let Some(key) = ctx.kb.keys.get_mut(usize::from(index)) {
            key.role = Some(value);
        }
    }
    Ok(())
}

/// Builds the display table once keys exist. A later display for the same
/// target replaces the earlier one in place. A `keyId` naming no key is
/// dropped with a warning, since displays may be shared across keyboards.
pub(super) fn displays(ctx: &mut Ctx, root: &El, pending: Vec<PendingDisplay>) -> Result<()> {
    for PendingDisplay { el, output } in pending {
        let display = ctx.display_attr(el, "display")?;
        let target = match output {
            Some(text) => DisplayTarget::Output(text),
            None => {
                let id = el.attr("keyId").unwrap_or("");
                match ctx.kb.key_index(id) {
                    Some(k) => DisplayTarget::Key(k),
                    None => {
                        ctx.warn(
                            el.warning(format!("no key has id {id}; display dropped"))
                                .at("keyId"),
                        );
                        continue;
                    }
                }
            }
        };
        let entries = &mut ctx.kb.displays.entries;
        match entries.iter_mut().find(|d| d.target == target) {
            Some(existing) => existing.display = display,
            None => entries.push(Display { target, display }),
        }
    }
    if let Some(options) = root
        .child("displays")
        .and_then(|d| d.child("displayOptions"))
        && options.attr("baseCharacter").is_some()
    {
        ctx.kb.displays.display_base = Some(ctx.display_attr(options, "baseCharacter")?);
    }
    Ok(())
}

/// The implied keys of `keys-Latn-implied.xml` at the version a keyboard
/// conforming to `version` uses, as resolution builds them.
pub(crate) fn implied_keys(version: u8) -> Vec<Key> {
    let path = format!(
        "{}/keys-Latn-implied.xml",
        crate::cldr::implied_version(version)
    );
    let Some(text) = crate::cldr::file(&path) else {
        return Vec::new();
    };
    let Ok(source) = crate::read::read_import(&path, None, text.as_bytes()) else {
        return Vec::new();
    };
    let Ok(implied) = El::from_document(&source.name, &source.document) else {
        return Vec::new();
    };
    implied
        .children_named("key")
        .map(|el| {
            let output =
                crate::escape::decode_plain(el.attr("output").unwrap_or("")).unwrap_or_default();
            let mut key = Key::new(el.attr("id").unwrap_or(""), Text::from(output.as_str()));
            key.gap = el.attr("gap") == Some("true");
            key.stretch = el.attr("stretch") == Some("true");
            key.width = el
                .attr("width")
                .and_then(parse_width)
                .unwrap_or(DEFAULT_WIDTH);
            key
        })
        .collect()
}
