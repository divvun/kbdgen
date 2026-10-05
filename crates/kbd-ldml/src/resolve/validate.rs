//! Validation after imports are resolved (`ldml.xml.validate`): the DTD's
//! content models, attribute lists, enumerations and `@MATCH` patterns,
//! and the textual constraints of UTS #35 Part 7. The model invariants are
//! checked on the built model.

use kbd_model::is_nmtoken;

use crate::diag::{Diagnostic, Result};
use crate::escape::decode_plain;
use crate::gencat::is_mark;
use crate::syntax::is_variable_id;
use crate::tree::El;

#[derive(Clone, Copy)]
enum Occurs {
    Optional,
    One,
    Many,
}

#[derive(Clone, Copy)]
enum Check {
    Any,
    Enum(&'static [&'static str]),
    Nmtoken,
    Nmtokens,
    /// `[A-Za-z0-9][A-Za-z0-9_-]*`
    Id,
    /// `[0-9A-Za-z_]{1,32}`
    VariableId,
    Bcp47,
    Digits,
    /// A decimal from 0.01 to 100.0.
    Width,
    /// A whole number from 1 to 999.
    DeviceWidth,
    Directions,
    ScanCodes,
    Modifiers,
}

struct Attr {
    name: &'static str,
    required: bool,
    check: Check,
}

const fn opt(name: &'static str, check: Check) -> Attr {
    Attr {
        name,
        required: false,
        check,
    }
}

const fn req(name: &'static str, check: Check) -> Attr {
    Attr {
        name,
        required: true,
        check,
    }
}

enum Content {
    Empty,
    /// A sequence of (element, occurrence).
    Seq(&'static [(&'static str, Occurs)]),
    /// `( import*, ( transform* | reorder* ), special* )`
    Group,
}

struct ElementDecl {
    name: &'static str,
    attrs: &'static [Attr],
    content: Content,
}

use Check::*;
use Occurs::*;

const ELEMENTS: &[ElementDecl] = &[
    ElementDecl {
        name: "keyboard3",
        attrs: &[
            req("locale", Bcp47),
            req("conformsTo", Enum(&["45", "46", "47", "48", "49"])),
            opt("xmlns", Any),
            opt(
                "draft",
                Enum(&["approved", "contributed", "provisional", "unconfirmed"]),
            ),
        ],
        content: Content::Seq(&[
            ("import", Many),
            ("locales", Optional),
            ("version", Optional),
            ("info", One),
            ("settings", Optional),
            ("displays", Optional),
            ("keys", Optional),
            ("flicks", Optional),
            ("forms", Optional),
            ("layers", Many),
            ("variables", Optional),
            ("transforms", Many),
            ("special", Many),
        ]),
    },
    ElementDecl {
        name: "import",
        attrs: &[req("path", Any), opt("base", Enum(&["cldr"]))],
        content: Content::Empty,
    },
    ElementDecl {
        name: "locales",
        attrs: &[],
        content: Content::Seq(&[("locale", Many)]),
    },
    ElementDecl {
        name: "locale",
        attrs: &[req("id", Bcp47)],
        content: Content::Empty,
    },
    ElementDecl {
        name: "version",
        attrs: &[opt("number", Any), opt("cldrVersion", Digits)],
        content: Content::Empty,
    },
    ElementDecl {
        name: "info",
        attrs: &[
            req("name", Any),
            opt("author", Any),
            opt("layout", Any),
            opt("indicator", Any),
            opt("attribution", Any),
        ],
        content: Content::Empty,
    },
    ElementDecl {
        name: "settings",
        attrs: &[opt("normalization", Enum(&["disabled"]))],
        content: Content::Empty,
    },
    ElementDecl {
        name: "displays",
        attrs: &[],
        content: Content::Seq(&[
            ("import", Many),
            ("display", Many),
            ("displayOptions", Many),
            ("special", Many),
        ]),
    },
    ElementDecl {
        name: "display",
        attrs: &[opt("keyId", Id), opt("output", Any), req("display", Any)],
        content: Content::Empty,
    },
    ElementDecl {
        name: "displayOptions",
        attrs: &[opt("baseCharacter", Any)],
        content: Content::Empty,
    },
    ElementDecl {
        name: "keys",
        attrs: &[],
        content: Content::Seq(&[("import", Many), ("key", Many), ("special", Many)]),
    },
    ElementDecl {
        name: "key",
        attrs: &[
            req("id", Nmtoken),
            opt("flickId", Nmtoken),
            opt("gap", Enum(&["true"])),
            opt("output", Any),
            opt("longPressKeyIds", Nmtokens),
            opt("longPressDefaultKeyId", Nmtoken),
            opt("multiTapKeyIds", Nmtokens),
            opt("stretch", Enum(&["true"])),
            opt("layerId", Nmtoken),
            opt("width", Width),
        ],
        content: Content::Empty,
    },
    ElementDecl {
        name: "flicks",
        attrs: &[],
        content: Content::Seq(&[("import", Many), ("flick", Many), ("special", Many)]),
    },
    ElementDecl {
        name: "flick",
        attrs: &[req("id", Nmtoken)],
        content: Content::Seq(&[("flickSegment", Many), ("special", Many)]),
    },
    ElementDecl {
        name: "flickSegment",
        attrs: &[req("directions", Directions), req("keyId", Nmtoken)],
        content: Content::Empty,
    },
    ElementDecl {
        name: "forms",
        attrs: &[],
        content: Content::Seq(&[("import", Many), ("form", Many), ("special", Many)]),
    },
    ElementDecl {
        name: "form",
        attrs: &[opt("id", Id)],
        content: Content::Seq(&[("scanCodes", Many), ("special", Many)]),
    },
    ElementDecl {
        name: "scanCodes",
        attrs: &[req("codes", ScanCodes)],
        content: Content::Empty,
    },
    ElementDecl {
        name: "layers",
        attrs: &[req("formId", Id), opt("minDeviceWidth", DeviceWidth)],
        content: Content::Seq(&[("import", Many), ("layer", Many), ("special", Many)]),
    },
    ElementDecl {
        name: "layer",
        attrs: &[opt("id", Id), opt("modifiers", Modifiers)],
        content: Content::Seq(&[("row", Many), ("special", Many)]),
    },
    ElementDecl {
        name: "row",
        attrs: &[req("keys", Nmtokens)],
        content: Content::Empty,
    },
    ElementDecl {
        name: "variables",
        attrs: &[],
        content: Content::Seq(&[
            ("import", Many),
            ("string", Many),
            ("set", Many),
            ("uset", Many),
            ("special", Many),
        ]),
    },
    ElementDecl {
        name: "string",
        attrs: &[req("id", VariableId), req("value", Any)],
        content: Content::Empty,
    },
    ElementDecl {
        name: "set",
        attrs: &[req("id", VariableId), req("value", Any)],
        content: Content::Empty,
    },
    ElementDecl {
        name: "uset",
        attrs: &[req("id", VariableId), req("value", Any)],
        content: Content::Empty,
    },
    ElementDecl {
        name: "transforms",
        attrs: &[req("type", Enum(&["simple", "backspace"]))],
        content: Content::Seq(&[
            ("import", Many),
            ("transformGroup", Many),
            ("special", Many),
        ]),
    },
    ElementDecl {
        name: "transformGroup",
        attrs: &[],
        content: Content::Group,
    },
    ElementDecl {
        name: "transform",
        attrs: &[req("from", Any), opt("to", Any)],
        content: Content::Empty,
    },
    ElementDecl {
        name: "reorder",
        attrs: &[
            opt("before", Any),
            req("from", Any),
            opt("order", Any),
            opt("tertiary", Any),
            opt("tertiaryBase", Any),
            opt("preBase", Any),
        ],
        content: Content::Empty,
    },
];

fn is_id(s: &str) -> bool {
    let mut chars = s.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphanumeric())
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// BCP 47 well-formedness, structurally: alphanumeric subtags of 1–8
/// characters joined by `-`, the first alphabetic.
fn is_bcp47(s: &str) -> bool {
    let mut subtags = s.split('-');
    let first_ok = subtags
        .next()
        .is_some_and(|t| (1..=8).contains(&t.len()) && t.bytes().all(|b| b.is_ascii_alphabetic()));
    first_ok
        && subtags
            .all(|t| (1..=8).contains(&t.len()) && t.bytes().all(|b| b.is_ascii_alphanumeric()))
}

/// A width from 0.01 to 100.0, written as a plain decimal.
fn is_width(s: &str) -> bool {
    let (int, frac) = s.split_once('.').unwrap_or((s, ""));
    let digits = |t: &str| t.bytes().all(|b| b.is_ascii_digit());
    if int.is_empty() && frac.is_empty() || !digits(int) || !digits(frac) {
        return false;
    }
    let thousandths = super::layers::parse_width(s);
    thousandths.is_some_and(|w| (10..=100_000).contains(&w))
}

fn passes(check: Check, value: &str) -> bool {
    let tokens = || value.split_ascii_whitespace();
    match check {
        Any => true,
        Enum(values) => values.contains(&value),
        Nmtoken => is_nmtoken(value),
        Nmtokens => tokens().next().is_some() && tokens().all(is_nmtoken),
        Id => is_id(value),
        VariableId => is_variable_id(value),
        Bcp47 => is_bcp47(value),
        Digits => !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit()),
        Width => is_width(value),
        DeviceWidth => value.parse::<u16>().is_ok_and(|w| (1..=999).contains(&w)),
        Directions => {
            tokens().next().is_some()
                && tokens().all(|t| matches!(t, "n" | "e" | "s" | "w" | "ne" | "nw" | "se" | "sw"))
        }
        ScanCodes => {
            tokens().next().is_some()
                && tokens().all(|t| t.len() == 2 && t.bytes().all(|b| b.is_ascii_hexdigit()))
        }
        Modifiers => value.split(',').all(|set| {
            set.split_ascii_whitespace().next().is_some()
                && set
                    .split_ascii_whitespace()
                    .all(|c| !c.is_empty() && c.bytes().all(|b| b.is_ascii_alphanumeric()))
        }),
    }
}

fn check_attrs(el: &El, decl: &ElementDecl) -> Result<()> {
    for (name, value) in &el.attrs {
        if name == "xmlns" && decl.name != "keyboard3" || name.starts_with("xmlns:") {
            continue;
        }
        let Some(attr) = decl.attrs.iter().find(|a| a.name == name) else {
            return Err(el.attr_error(name, format!("{name} is not an attribute of {}", decl.name)));
        };
        if !passes(attr.check, value) {
            return Err(el.attr_error(name, format!("{value:?} is not a valid {name}")));
        }
    }
    for attr in decl.attrs.iter().filter(|a| a.required) {
        if el.attr(attr.name).is_none() {
            return Err(el.attr_error(attr.name, "required attribute is missing"));
        }
    }
    Ok(())
}

fn check_content(el: &El, decl: &ElementDecl) -> Result<()> {
    let names: Vec<&str> = el
        .children
        .iter()
        .map(|c| {
            if c.prefix.is_some() {
                ""
            } else {
                c.name.as_str()
            }
        })
        .collect();
    if let Some(i) = names.iter().position(|n| n.is_empty()) {
        let child = el.children.get(i).map_or(el, |c| c);
        return Err(child.error(format!(
            "{} is outside special; only special may hold other namespaces",
            child.qualified()
        )));
    }
    if let Some(child) = el
        .children
        .iter()
        .find(|c| c.name != "special" && !ELEMENTS.iter().any(|d| d.name == c.name))
    {
        return Err(child.error(format!("unknown element {}", child.name)));
    }
    match decl.content {
        Content::Empty => match el.children.first() {
            Some(child) => Err(child.error(format!("{} has no child elements", decl.name))),
            None => Ok(()),
        },
        Content::Group => {
            let kinds: Vec<&str> = names
                .iter()
                .copied()
                .filter(|n| *n != "import" && *n != "special")
                .collect();
            if let Some(bad) = kinds
                .iter()
                .find(|n| **n != "transform" && **n != "reorder")
            {
                return Err(el.error(format!("{bad} is not allowed in transformGroup")));
            }
            if kinds.contains(&"transform") && kinds.contains(&"reorder") {
                return Err(
                    el.error("a transformGroup holds transform or reorder elements, not both")
                );
            }
            if kinds.is_empty() {
                return Err(el.error("a transformGroup may not be empty"));
            }
            let special_at = names
                .iter()
                .position(|n| *n == "special")
                .unwrap_or(names.len());
            if names.iter().skip(special_at).any(|n| *n != "special") {
                return Err(el.error("special must come last in transformGroup"));
            }
            Ok(())
        }
        Content::Seq(particles) => {
            let mut i = 0;
            for (name, occurs) in particles {
                let mut count = 0;
                while names.get(i) == Some(name) {
                    i += 1;
                    count += 1;
                }
                let ok = match occurs {
                    Optional => count <= 1,
                    One => count == 1,
                    Many => true,
                };
                if !ok {
                    return Err(el.error(format!("{} must have exactly one {name}", decl.name)));
                }
            }
            match el.children.get(i) {
                Some(child) => Err(child.error(format!(
                    "{} is not allowed here in {} (order: DTD content model)",
                    child.name, decl.name
                ))),
                None => Ok(()),
            }
        }
    }
}

fn walk(el: &El) -> Result<()> {
    if el.prefix.is_none() && el.name == "special" {
        return Ok(());
    }
    let decl = ELEMENTS
        .iter()
        .find(|d| d.name == el.name && el.prefix.is_none())
        .ok_or_else(|| el.error(format!("unknown element {}", el.qualified())))?;
    check_attrs(el, decl)?;
    check_content(el, decl)?;
    el.children.iter().try_for_each(walk)
}

fn keys_constraints(root: &El) -> Result<()> {
    for key in root
        .child("keys")
        .iter()
        .flat_map(|k| k.children_named("key"))
    {
        if key.attr("gap").is_some() {
            for a in [
                "flickId",
                "longPressKeyIds",
                "longPressDefaultKeyId",
                "multiTapKeyIds",
                "layerId",
                "output",
            ] {
                if key.attr(a).is_some() {
                    return Err(key.attr_error(a, "a gap key may not have this attribute"));
                }
            }
        } else if key.attr("output").is_none() && key.attr("layerId").is_none() {
            return Err(key.error("a key needs output, layerId or gap"));
        }
    }
    Ok(())
}

fn display_constraints(root: &El, warnings: &mut Vec<Diagnostic>) -> Result<()> {
    let Some(displays) = root.child("displays") else {
        return Ok(());
    };
    if displays.children_named("displayOptions").count() > 1 {
        return Err(displays.error("displayOptions may occur once"));
    }
    for display in displays.children_named("display") {
        match (display.attr("output"), display.attr("keyId")) {
            (Some(_), Some(_)) | (None, None) => {
                return Err(display.error("a display has exactly one of output and keyId"));
            }
            (Some(output), None) if Some(output) == display.attr("display") => {
                return Err(
                    display.attr_error("display", "display equals output, an extraneous entry")
                );
            }
            _ => {}
        }
        let shown = display.attr("display").unwrap_or("");
        if !shown.contains("${")
            && decode_plain(shown).is_ok_and(|s| s.chars().next().is_some_and(is_mark))
        {
            warnings.push(
                display
                    .warning("a display starting with a combining mark needs a base such as U+25CC")
                    .at("display"),
            );
        }
    }
    Ok(())
}

fn layers_constraints(root: &El) -> Result<()> {
    let all: Vec<&El> = root.children_named("layers").collect();
    if all.is_empty() {
        return Err(root.error("a keyboard needs at least one layers element"));
    }
    let hardware: Vec<&&El> = all
        .iter()
        .filter(|l| l.attr("formId") != Some("touch"))
        .collect();
    if let Some(second) = hardware.get(1) {
        return Err(second.error("a keyboard has at most one hardware layers element"));
    }
    let mut widths: Vec<Option<&str>> = Vec::new();
    for layers in all.iter().filter(|l| l.attr("formId") == Some("touch")) {
        let width = layers.attr("minDeviceWidth");
        if widths.contains(&width) {
            return Err(layers.attr_error(
                "minDeviceWidth",
                "touch layers need distinct minDeviceWidth",
            ));
        }
        widths.push(width);
        for layer in layers.children_named("layer") {
            if layer.attr("id").is_none() {
                return Err(layer.attr_error("id", "a touch layer needs an id"));
            }
        }
        if !layers
            .children_named("layer")
            .any(|l| l.attr("id") == Some("base"))
        {
            return Err(layers.error("a touch layers element needs a layer with id base"));
        }
    }
    for layers in hardware {
        for layer in layers.children_named("layer") {
            if layer.attr("modifiers").is_none() {
                return Err(layer.attr_error("modifiers", "a hardware layer needs modifiers"));
            }
        }
    }
    for form in root
        .child("forms")
        .iter()
        .flat_map(|f| f.children_named("form"))
    {
        if form.attr("id") == Some("touch") {
            return Err(form.attr_error("id", "a form may not be named touch"));
        }
    }
    Ok(())
}

fn variable_constraints(root: &El) -> Result<()> {
    let mut seen: Vec<&str> = Vec::new();
    for var in root
        .child("variables")
        .iter()
        .flat_map(|v| v.children.iter())
    {
        if var.prefix.is_some() || var.name == "special" {
            continue;
        }
        let id = var.attr("id").unwrap_or("");
        if seen.contains(&id) {
            return Err(var.attr_error("id", format!("variable id {id} is used twice")));
        }
        seen.push(id);
    }
    let mut types: Vec<&str> = Vec::new();
    for transforms in root.children_named("transforms") {
        let t = transforms.attr("type").unwrap_or("");
        if types.contains(&t) {
            return Err(transforms.error(format!("a second transforms of type {t}")));
        }
        types.push(t);
    }
    Ok(())
}

// [spec:kbdgen:req:ldml.xml.validate+1]
/// Validates a keyboard after imports are resolved. An unknown element or
/// attribute outside `special` is an error; `special` content is left to
/// the extension reader, which interprets only kbdgen's namespace.
pub(super) fn validate(root: &El, warnings: &mut Vec<Diagnostic>) -> Result<()> {
    walk(root)?;
    keys_constraints(root)?;
    display_constraints(root, warnings)?;
    layers_constraints(root)?;
    variable_constraints(root)
}
