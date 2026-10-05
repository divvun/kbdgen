//! Import of the layout-level fields: verbatim LDML taken from the
//! canonical export of the model, and kbdgen data from its extensions.

use kbd_ldml::escape::{encode_plain, encode_text};
use kbd_ldml::{ComposeValue, DeadKey, encode_modifiers, encode_width};
use kbd_model::{DEFAULT_WIDTH, DisplayTarget, Flick, Key, Keyboard, ModifierSet, TextElem};
use serde_yaml::{Mapping, Value};
use xmlem::{Document, Element};

use super::keys::pieces;
use super::layers::insert;
use crate::ldml::yaml::emoji::position_name;

fn children(doc: &Document, el: Element, name: &str) -> Vec<Element> {
    el.children(doc)
        .into_iter()
        .filter(|c| c.name(doc) == name)
        .collect()
}

/// `variables` as the canonical export writes them.
pub fn variables(canon: &Document) -> Value {
    let root = canon.root();
    let mut map = Mapping::new();
    for vars in children(canon, root, "variables") {
        for (element, field) in [("string", "strings"), ("set", "sets"), ("uset", "usets")] {
            let mut entries = Mapping::new();
            for v in children(canon, vars, element) {
                insert(
                    &mut entries,
                    v.attribute(canon, "id").unwrap_or(""),
                    Value::String(v.attribute(canon, "value").unwrap_or("").into()),
                );
            }
            if !entries.is_empty() {
                insert(&mut map, field, Value::Mapping(entries));
            }
        }
    }
    if map.is_empty() {
        Value::Null
    } else {
        Value::Mapping(map)
    }
}

fn is_generated(canon: &Document, group: Element) -> bool {
    children(canon, group, "special")
        .into_iter()
        .any(|s| !children(canon, s, "kbdgen:generated").is_empty())
}

/// The transform groups of one list (`simple` or `backspace`) as the
/// canonical export writes them, without the generated ones when
/// `drop_generated`.
pub fn groups(canon: &Document, list: &str, drop_generated: bool) -> Value {
    let root = canon.root();
    let mut out = Vec::new();
    for transforms in children(canon, root, "transforms") {
        if transforms.attribute(canon, "type") != Some(list) {
            continue;
        }
        for group in children(canon, transforms, "transformGroup") {
            if drop_generated && is_generated(canon, group) {
                continue;
            }
            let reorders = children(canon, group, "reorder");
            if !reorders.is_empty() {
                let items: Vec<Value> = reorders
                    .into_iter()
                    .map(|r| {
                        let mut m = Mapping::new();
                        for (attr, field) in [
                            ("from", "from"),
                            ("before", "before"),
                            ("order", "order"),
                            ("tertiary", "tertiary"),
                            ("tertiaryBase", "tertiaryBase"),
                            ("preBase", "preBase"),
                        ] {
                            if let Some(v) = r.attribute(canon, attr) {
                                insert(&mut m, field, Value::String(v.into()));
                            }
                        }
                        Value::Mapping(m)
                    })
                    .collect();
                let mut m = Mapping::new();
                insert(&mut m, "reorder", Value::Sequence(items));
                out.push(Value::Mapping(m));
                continue;
            }
            let rules: Vec<Value> = children(canon, group, "transform")
                .into_iter()
                .map(|t| {
                    let mut m = Mapping::new();
                    insert(
                        &mut m,
                        "from",
                        Value::String(t.attribute(canon, "from").unwrap_or("").into()),
                    );
                    if let Some(to) = t.attribute(canon, "to") {
                        insert(&mut m, "to", Value::String(to.into()));
                    }
                    Value::Mapping(m)
                })
                .collect();
            out.push(Value::Sequence(rules));
        }
    }
    if out.is_empty() {
        Value::Null
    } else {
        Value::Sequence(out)
    }
}

fn ids(kb: &Keyboard, keys: &[u16]) -> String {
    keys.iter()
        .filter_map(|k| kb.key(*k).map(|key| key.id.clone()))
        .collect::<Vec<_>>()
        .join(" ")
}

/// A `keys` entry with every attribute of the model key.
pub fn explicit_key(kb: &Keyboard, key: &Key) -> Value {
    let mut m = Mapping::new();
    if !key.output.is_empty() {
        insert(
            &mut m,
            "output",
            Value::String(encode_text(&pieces(kb, &key.output))),
        );
    }
    if key.gap {
        insert(&mut m, "gap", Value::Bool(true));
    }
    if let Some(layer) = &key.layer_id {
        insert(&mut m, "layer", Value::String(layer.clone()));
    }
    if key.width != DEFAULT_WIDTH {
        insert(&mut m, "width", Value::String(encode_width(key.width)));
    }
    if key.stretch {
        insert(&mut m, "stretch", Value::Bool(true));
    }
    if !key.long_press.is_empty() {
        insert(&mut m, "longPress", Value::String(ids(kb, &key.long_press)));
    }
    if let Some(d) = key.long_press_default {
        insert(&mut m, "longPressDefault", Value::String(ids(kb, &[d])));
    }
    if !key.multi_tap.is_empty() {
        insert(&mut m, "multiTap", Value::String(ids(kb, &key.multi_tap)));
    }
    if let Some(f) = key.flick.and_then(|f| kb.flicks.get(usize::from(f))) {
        insert(&mut m, "flick", Value::String(f.id.clone()));
    }
    if let Some(role) = key.role {
        insert(&mut m, "role", Value::String(role.name().into()));
    }
    Value::Mapping(m)
}

pub fn explicit_flick(kb: &Keyboard, flick: &Flick) -> Value {
    Value::Sequence(
        flick
            .segments
            .iter()
            .map(|s| {
                let mut m = Mapping::new();
                let names: Vec<&str> = s.directions.iter().map(|d| d.name()).collect();
                insert(&mut m, "directions", Value::String(names.join(" ")));
                insert(&mut m, "key", Value::String(ids(kb, &[s.key])));
                Value::Mapping(m)
            })
            .collect(),
    )
}

/// The model's displays, without the trailing entries for which `auto`
/// holds, which lowering adds again.
pub fn displays(kb: &Keyboard, auto: &dyn Fn(&kbd_model::Display) -> bool) -> Value {
    let entries = &kb.displays.entries;
    let keep = entries.iter().rposition(|d| !auto(d)).map_or(0, |i| i + 1);
    let mut out: Vec<Value> = entries
        .iter()
        .take(keep)
        .map(|d| {
            let mut m = Mapping::new();
            match &d.target {
                DisplayTarget::Output(text) => {
                    insert(
                        &mut m,
                        "output",
                        Value::String(encode_text(&pieces(kb, text))),
                    );
                }
                DisplayTarget::Key(k) => insert(&mut m, "keyId", Value::String(ids(kb, &[*k]))),
            }
            insert(&mut m, "display", Value::String(encode_plain(&d.display)));
            Value::Mapping(m)
        })
        .collect();
    if let Some(base) = &kb.displays.display_base {
        let mut m = Mapping::new();
        insert(&mut m, "displayBase", Value::String(encode_plain(base)));
        out.push(Value::Mapping(m));
    }
    if out.is_empty() {
        Value::Null
    } else {
        Value::Sequence(out)
    }
}

/// The display of marker `marker`, if the model has one.
pub fn marker_display(kb: &Keyboard, marker: &str) -> Option<String> {
    let index = kb.marker_index(marker)?;
    kb.displays
        .entries
        .iter()
        .rev()
        .find(|d| matches!(&d.target, DisplayTarget::Output(t) if t.elements() == [TextElem::Marker(index)]))
        .map(|d| d.display.clone())
}

fn dead_node(
    kb: &Keyboard,
    identity: &str,
    marker: &str,
    default_marker: &str,
    display: Option<String>,
    standalone: &str,
    compose: &[kbd_ldml::Compose],
) -> Mapping {
    let mut m = Mapping::new();
    if marker != default_marker {
        insert(&mut m, "marker", Value::String(marker.into()));
    }
    if let Some(display) = display.filter(|d| d != identity) {
        insert(&mut m, "display", Value::String(encode_plain(&display)));
    }
    if standalone != identity {
        insert(
            &mut m,
            "standalone",
            Value::String(encode_plain(standalone)),
        );
    }
    let mut entries = Mapping::new();
    for c in compose {
        let value = match &c.value {
            ComposeValue::Output(output) => Value::String(encode_plain(output)),
            ComposeValue::Node {
                marker: child,
                standalone,
                compose,
            } => {
                let child_identity = format!("{identity}{}", c.input);
                let default = format!(
                    "{marker}-{}",
                    crate::ldml::yaml::text::hex_name(c.input.chars(), "_")
                );
                Value::Mapping(dead_node(
                    kb,
                    &child_identity,
                    child,
                    &default,
                    marker_display(kb, child),
                    standalone,
                    compose,
                ))
            }
        };
        insert(&mut entries, &encode_plain(&c.input), value);
    }
    if !entries.is_empty() {
        insert(&mut m, "compose", Value::Mapping(entries));
    }
    m
}

/// `deadKeys` from the `kbdgen:deadKey` metadata, leaving out every field
/// that has its default.
pub fn dead_keys(kb: &Keyboard, metadata: &[DeadKey]) -> Value {
    let mut out = Mapping::new();
    for dk in metadata {
        let default = format!(
            "dk_{}",
            crate::ldml::yaml::text::hex_name(dk.identity.chars(), "_")
        );
        let mut node = dead_node(
            kb,
            &dk.identity,
            &dk.marker,
            &default,
            Some(dk.display.clone()),
            &dk.standalone,
            &dk.compose,
        );
        if let Some(name) = &dk.name {
            insert(&mut node, "name", Value::String(encode_plain(name)));
        }
        insert(&mut out, &encode_plain(&dk.identity), Value::Mapping(node));
    }
    Value::Mapping(out)
}

/// `targets`, from `kbdgen:target` and the model's Windows options. A
/// target the schema has no field for is reported.
pub fn targets(kb: &Keyboard, entries: &[kbd_ldml::Target], warnings: &mut Vec<String>) -> Value {
    let known = [
        ("windows", ["locale", "id"]),
        ("chromeOS", ["locale", "xkbLayout"]),
        ("iOS", ["spellerPackageKey", "spellerPath"]),
        ("android", ["spellerPackageKey", "spellerPath"]),
    ];
    let mut hosts: Vec<(&str, Mapping)> = Vec::new();
    let slot = |host: &'static str, hosts: &mut Vec<(&str, Mapping)>| -> usize {
        match hosts.iter().position(|(h, _)| *h == host) {
            Some(i) => i,
            None => {
                hosts.push((host, Mapping::new()));
                hosts.len() - 1
            }
        }
    };
    for t in entries {
        let Some((host, _)) = known
            .iter()
            .find(|(h, names)| *h == t.host && names.contains(&t.name.as_str()))
        else {
            warnings.push(format!(
                "the target {} {} has no v4 field and is dropped",
                t.host, t.name
            ));
            continue;
        };
        let i = slot(host, &mut hosts);
        if let Some((_, m)) = hosts.get_mut(i) {
            insert(m, &t.name, Value::String(t.value.clone()));
        }
    }
    let w = &kb.windows;
    if w.shift_lock || w.lrm_rlm || !w.key_names.is_empty() {
        let i = slot("windows", &mut hosts);
        if let Some((_, m)) = hosts.get_mut(i) {
            if w.shift_lock {
                insert(m, "shiftLock", Value::Bool(true));
            }
            if w.lrm_rlm {
                insert(m, "lrmRlm", Value::Bool(true));
            }
            if !w.key_names.is_empty() {
                let mut names = Mapping::new();
                for (k, v) in &w.key_names {
                    insert(&mut names, k, Value::String(v.clone()));
                }
                insert(m, "keyNames", Value::Mapping(names));
            }
        }
    }
    if hosts.is_empty() {
        return Value::Null;
    }
    let mut out = Mapping::new();
    for (host, m) in hosts {
        insert(&mut out, host, Value::Mapping(m));
    }
    Value::Mapping(out)
}

/// `emoji`: the key by ISO position. Annotations need a file of their own,
/// which import does not write, so they are reported instead.
pub fn emoji(kb: &Keyboard, warnings: &mut Vec<String>) -> Value {
    if !kb.emoji.annotations.is_empty() {
        warnings.push(format!(
            "{} emoji annotations are dropped; v4 reads them from a CLDR annotations file",
            kb.emoji.annotations.len()
        ));
    }
    let Some(key) = kb.emoji.key else {
        return Value::Null;
    };
    let Some(position) = position_name(key.scan_code) else {
        warnings.push(format!(
            "the emoji key's scan code {:02X} has no ISO position and is dropped",
            key.scan_code
        ));
        return Value::Null;
    };
    let mut k = Mapping::new();
    insert(&mut k, "position", Value::String(position.into()));
    insert(
        &mut k,
        "modifiers",
        Value::String(encode_modifiers(&[ModifierSet::Set(key.modifiers)])),
    );
    let mut m = Mapping::new();
    insert(&mut m, "key", Value::Mapping(k));
    Value::Mapping(m)
}
