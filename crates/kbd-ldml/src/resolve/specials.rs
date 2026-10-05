//! kbdgen's elements in the `special` of `keyboard3` (`ldml.xml.special`,
//! `ldml.xml.resolve` step 11).

use kbd_model::{Annotation, EmojiKey, Host, ModifierSet, WINDOWS_KEY_NAMES};

use super::Ctx;
use super::layers::{extra_key, parse_modifiers};
use crate::diag::Result;
use crate::special::{Target, bool_attr, check_attrs, kbdgen_children, plain_attr, read_dead_key};
use crate::tree::El;

fn keyboard_element(ctx: &mut Ctx, el: &El) -> Result<()> {
    check_attrs(
        el,
        &[
            "tag",
            "host",
            "decimal",
            "spaceLabel",
            "returnLabel",
            "impliedLayers",
        ],
        &[],
    )?;
    if let Some(tag) = el.attr("tag") {
        ctx.extensions.tag = Some(tag.to_string());
    }
    if let Some(host) = el.attr("host") {
        ctx.kb.host = Some(
            Host::from_name(host)
                .ok_or_else(|| el.attr_error("host", format!("unknown host {host}")))?,
        );
    }
    if el.attr("decimal").is_some() {
        let decimal = ctx.text_attr(el, "decimal")?;
        ctx.kb.decimal = Some(decimal);
    }
    if el.attr("spaceLabel").is_some() {
        ctx.kb.displays.labels.space = Some(plain_attr(el, "spaceLabel")?);
    }
    if el.attr("returnLabel").is_some() {
        ctx.kb.displays.labels.r#return = Some(plain_attr(el, "returnLabel")?);
    }
    if let Some(implied) = el.attr("impliedLayers") {
        ctx.extensions.implied_layers = Some(implied.to_string());
    }
    Ok(())
}

fn emoji_key(ctx: &mut Ctx, el: &El) -> Result<()> {
    check_attrs(el, &["scanCode", "modifiers"], &["scanCode", "modifiers"])?;
    let code = el.attr("scanCode").unwrap_or("");
    let scan_code = u8::from_str_radix(code, 16)
        .ok()
        .filter(|_| code.len() == 2)
        .ok_or_else(|| {
            el.attr_error(
                "scanCode",
                format!("{code} is not a two-digit hex scan code"),
            )
        })?;
    let sets = parse_modifiers(el.attr("modifiers").unwrap_or(""))
        .map_err(|e| el.attr_error("modifiers", e))?;
    let modifiers = match sets.as_slice() {
        [ModifierSet::Set(m)] => *m,
        _ => {
            return Err(el.attr_error("modifiers", "the emoji key has one modifier set, not other"));
        }
    };
    ctx.kb.emoji.key = Some(EmojiKey {
        scan_code,
        modifiers,
    });
    Ok(())
}

fn element(ctx: &mut Ctx, el: &El, prefix: &str) -> Result<()> {
    match el.name.as_str() {
        "keyboard" => keyboard_element(ctx, el)?,
        "displayName" => {
            check_attrs(el, &["lang", "name"], &["lang", "name"])?;
            ctx.extensions
                .display_names
                .insert(plain_attr(el, "lang")?, plain_attr(el, "name")?);
        }
        "flush" => {
            check_attrs(el, &["marker", "output"], &["marker", "output"])?;
            let marker = ctx.marker(el, el.attr("marker").unwrap_or(""))?;
            let output = plain_attr(el, "output")?;
            ctx.kb.flush.insert(marker, output);
        }
        "deadKeyName" => {
            check_attrs(el, &["marker", "name"], &["marker", "name"])?;
            let marker = ctx.marker(el, el.attr("marker").unwrap_or(""))?;
            ctx.kb
                .dead_key_names
                .insert(marker, plain_attr(el, "name")?);
        }
        "windows" => {
            check_attrs(el, &["shiftLock", "lrmRlm"], &[])?;
            ctx.kb.windows.shift_lock = bool_attr(el, "shiftLock")?;
            ctx.kb.windows.lrm_rlm = bool_attr(el, "lrmRlm")?;
        }
        "windowsKeyName" => {
            check_attrs(el, &["key", "name"], &["key", "name"])?;
            let key = plain_attr(el, "key")?;
            if !WINDOWS_KEY_NAMES.contains(&key.as_str()) {
                return Err(el.attr_error("key", format!("{key} is not a Windows key name")));
            }
            ctx.kb
                .windows
                .key_names
                .insert(key, plain_attr(el, "name")?);
        }
        "target" => {
            check_attrs(el, &["host", "name", "value"], &["host", "name", "value"])?;
            ctx.extensions.targets.push(Target {
                host: plain_attr(el, "host")?,
                name: plain_attr(el, "name")?,
                value: plain_attr(el, "value")?,
            });
        }
        "emojiKey" => emoji_key(ctx, el)?,
        "emoji" => {
            check_attrs(el, &["emoji", "name", "keywords"], &["emoji", "name"])?;
            let keywords = plain_attr(el, "keywords")?;
            ctx.kb.emoji.annotations.push(Annotation {
                emoji: plain_attr(el, "emoji")?,
                name: plain_attr(el, "name")?,
                keywords: keywords
                    .split('|')
                    .map(str::trim)
                    .filter(|k| !k.is_empty())
                    .map(str::to_string)
                    .collect(),
            });
        }
        "deadKey" => {
            let dead_key = read_dead_key(el, prefix)?;
            ctx.extensions.dead_keys.push(dead_key);
        }
        "extraModifier" => {
            let key = extra_key(el)?;
            ctx.kb.windows.extra_modifiers.push(key);
        }
        _ => {
            return Err(el.error(format!(
                "{} is not a kbdgen element of keyboard3",
                el.qualified()
            )));
        }
    }
    Ok(())
}

// [spec:kbdgen:def:ldml.xml.special+1]
/// Reads kbdgen's elements of the keyboard's own `special`. Elements of
/// other namespaces are kept in the document and ignored here.
pub(super) fn keyboard(ctx: &mut Ctx, root: &El) -> Result<()> {
    let Some(prefix) = ctx.kbdgen.clone() else {
        return Ok(());
    };
    let elements = kbdgen_children(root, Some(&prefix));
    if elements.iter().filter(|e| e.name == "keyboard").count() > 1 {
        return Err(root.error("kbdgen:keyboard may occur once"));
    }
    for el in elements {
        element(ctx, el, &prefix)?;
    }
    Ok(())
}
