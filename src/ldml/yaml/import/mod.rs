//! Import (`ldml.yaml.import`): keyboard3 documents of one layout tag to a
//! v4 layout.
//!
//! Each document's model is first exported canonically
//! (`kbd_ldml::export`), which splices imports in and spells every LDML
//! string the way export does. Import then writes the layout twice: with
//! sugar (tokens, `deadKeys`, `impliedLayers: macOS`) and fully verbatim.
//! Both are lowered and resolved again and compared with the documents'
//! models; the sugared layout is written when every difference it leaves
//! is one the verbatim layout leaves too, and every difference that
//! remains is reported (`ldml.yaml.roundtrip`).

mod fields;
mod keys;
mod layers;

use std::path::Path;

use kbd_ldml::escape::{Piece, encode_text};
use kbd_ldml::{Extensions, is_mark};
use kbd_model::{DisplayTarget, Host, Keyboard, Normalization};
use serde_yaml::{Mapping, Value};
use xmlem::Document;

use self::keys::{KeyTok, classify, pieces, token_of, usage};
use self::layers::insert;
use super::lower::lower;
use super::schema::parse;
use crate::ldml::LdmlError;
use crate::ldml::import::ImportedFile;

/// How much sugar a layout is written with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Sugar,
    Verbatim,
}

/// One document being imported, with its canonical export.
pub struct Doc<'a> {
    pub name: String,
    pub host: Option<Host>,
    pub kb: &'a Keyboard,
    pub ext: &'a Extensions,
    pub canon: Document,
}

/// An imported layout: the YAML text and what it could not carry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Imported {
    pub yaml: String,
    pub warnings: Vec<String>,
}

/// The hardware and touch variant a document's host writes to.
fn variant_names(host: Option<Host>) -> (Option<&'static str>, Option<&'static str>) {
    match host {
        None | Some(Host::Web) => (Some("default"), Some("default")),
        Some(Host::Windows) => (Some("windows"), None),
        Some(Host::MacOs) => (Some("macOS"), None),
        Some(Host::ChromeOs) => (Some("chromeOS"), None),
        Some(Host::Linux) => (Some("linux"), None),
        Some(Host::Ios) => (None, Some("iOS")),
        Some(Host::Android) => (Some("android"), Some("android")),
    }
}

/// A layout-level field as one document gives it.
type Field<'f> = dyn Fn(&Doc) -> Value + 'f;

/// The value every document must share, or the reason they cannot.
fn shared(docs: &[Doc], field: &str, value: &Field) -> Result<Value, String> {
    let mut iter = docs.iter();
    let first = iter.next().map(value).unwrap_or(Value::Null);
    for doc in iter {
        if value(doc) != first {
            return Err(format!("{} differs in {field}", doc.name));
        }
    }
    Ok(first)
}

fn info(kb: &Keyboard, autonym: &str) -> Value {
    let mut m = Mapping::new();
    if kb.info.name != autonym {
        insert(&mut m, "name", Value::String(kb.info.name.clone()));
    }
    for (k, v) in [
        ("author", &kb.info.author),
        ("layout", &kb.info.layout),
        ("indicator", &kb.info.indicator),
        ("attribution", &kb.info.attribution),
    ] {
        if let Some(v) = v {
            insert(&mut m, k, Value::String(v.clone()));
        }
    }
    if m.is_empty() {
        Value::Null
    } else {
        Value::Mapping(m)
    }
}

/// The dead-key metadata every document agrees on, or none when a
/// document has none or they differ.
fn dead_sugar<'a>(docs: &[Doc<'a>]) -> Option<&'a [kbd_ldml::DeadKey]> {
    let first = docs.first()?.ext.dead_keys.as_slice();
    if first.is_empty() || docs.iter().any(|d| d.ext.dead_keys.as_slice() != first) {
        return None;
    }
    Some(first)
}

struct Plan {
    classes: Vec<Vec<KeyTok>>,
    long_press: Vec<(Vec<Piece>, Vec<String>, usize, u16)>,
    touch_long_press: Vec<Mapping>,
}

fn candidate_ids(kb: &Keyboard, index: u16) -> Vec<String> {
    kb.key(index)
        .map(|k| {
            k.long_press
                .iter()
                .filter_map(|c| kb.key(*c).map(|c| c.id.clone()))
                .collect()
        })
        .unwrap_or_default()
}

fn tokenized(class: Option<&KeyTok>) -> bool {
    matches!(class, Some(KeyTok::Made(_) | KeyTok::Implied(_)))
}

fn demote(class: &mut KeyTok) {
    *class = match class {
        KeyTok::Implied(_) => KeyTok::Ref,
        _ => KeyTok::Explicit,
    };
}

/// Classifies every key and derives `longPress`: the global entries from
/// keys used in hardware rows, then per touch variant the entries that
/// differ, an empty one where a variant's keys have no long press. A key
/// whose long press its entry would not give is written `\k{id}`.
fn plan(docs: &[Doc], level: Level, dead: &dyn Fn(&str) -> Option<String>) -> Plan {
    let usages: Vec<Vec<(bool, bool)>> = docs.iter().map(|d| usage(d.kb)).collect();
    let mut classes: Vec<Vec<KeyTok>> = docs
        .iter()
        .zip(&usages)
        .map(|(d, used)| {
            d.kb.keys
                .iter()
                .zip(used)
                .map(|(k, u)| classify(d.kb, k, *u, level, dead))
                .collect()
        })
        .collect();
    let mut global: Vec<(Vec<Piece>, Vec<String>, usize, u16)> = Vec::new();
    for (di, doc) in docs.iter().enumerate() {
        for (ki, key) in doc.kb.keys.iter().enumerate() {
            let k = u16::try_from(ki).unwrap_or(u16::MAX);
            let used = usages[di][ki].0;
            if !used || key.long_press.is_empty() || !tokenized(classes[di].get(ki)) {
                continue;
            }
            let output = pieces(doc.kb, &key.output);
            if !global.iter().any(|(o, ..)| *o == output) {
                global.push((output, candidate_ids(doc.kb, k), di, k));
            }
        }
    }
    let mut touch_long_press = Vec::new();
    for (di, doc) in docs.iter().enumerate() {
        let mut variant: Vec<(Vec<Piece>, Vec<String>, u16)> = Vec::new();
        for (ki, key) in doc.kb.keys.iter().enumerate() {
            let k = u16::try_from(ki).unwrap_or(u16::MAX);
            let (hw, touch) = usages[di][ki];
            if !tokenized(classes[di].get(ki)) || key.output.is_empty() {
                continue;
            }
            let output = pieces(doc.kb, &key.output);
            let desired = candidate_ids(doc.kb, k);
            let entry = global
                .iter()
                .find(|(o, ..)| *o == output)
                .map(|(_, ids, ..)| ids.clone())
                .unwrap_or_default();
            if hw && desired != entry {
                if let Some(c) = classes[di].get_mut(ki) {
                    demote(c);
                }
                continue;
            }
            if !touch || hw || desired == entry {
                continue;
            }
            match variant.iter().find(|(o, ..)| *o == output) {
                Some((_, ids, _)) if *ids != desired => {
                    if let Some(c) = classes[di].get_mut(ki) {
                        demote(c);
                    }
                }
                Some(_) => {}
                None => variant.push((output, desired, k)),
            }
        }
        let mut map = Mapping::new();
        for (output, _, k) in variant {
            let tokens: Vec<String> = doc
                .kb
                .key(k)
                .map(|key| {
                    key.long_press
                        .iter()
                        .map(|c| token_of(doc.kb, &classes[di], *c))
                        .collect()
                })
                .unwrap_or_default();
            insert(
                &mut map,
                &encode_text(&output),
                Value::String(tokens.join(" ")),
            );
        }
        touch_long_press.push(map);
    }
    Plan {
        classes,
        long_press: global,
        touch_long_press,
    }
}

/// Whether lowering adds display `d` again (`ldml.yaml.displays.auto`).
fn auto_display(kb: &Keyboard, d: &kbd_model::Display, markers: &[(String, String)]) -> bool {
    match &d.target {
        DisplayTarget::Output(text) => {
            let p = pieces(kb, text);
            if let [Piece::Marker(m)] = p.as_slice() {
                return markers
                    .iter()
                    .any(|(mk, shown)| mk == m && *shown == d.display);
            }
            let marks =
                !p.is_empty() && p.iter().all(|x| matches!(x, Piece::Char(c) if is_mark(*c)));
            let mut shown = String::from('\u{25CC}');
            shown.extend(text.chars());
            marks && d.display == shown
        }
        DisplayTarget::Key(k) => {
            kb.key(*k).is_some_and(|key| key.id == "space")
                && kb.displays.labels.space.as_deref() == Some(d.display.as_str())
        }
    }
}

fn dead_markers(metadata: &[kbd_ldml::DeadKey], kb: &Keyboard) -> Vec<(String, String)> {
    fn walk(
        c: &[kbd_ldml::Compose],
        kb: &Keyboard,
        identity: &str,
        out: &mut Vec<(String, String)>,
    ) {
        for entry in c {
            if let kbd_ldml::ComposeValue::Node {
                marker, compose, ..
            } = &entry.value
            {
                let id = format!("{identity}{}", entry.input);
                let shown = fields::marker_display(kb, marker).unwrap_or_else(|| id.clone());
                out.push((marker.clone(), shown));
                walk(compose, kb, &id, out);
            }
        }
    }
    let mut out = Vec::new();
    for dk in metadata {
        out.push((dk.marker.clone(), dk.display.clone()));
        walk(&dk.compose, kb, &dk.identity, &mut out);
    }
    out
}

/// The layout written at `level`, or why that level cannot write it.
fn build(
    tag: &str,
    docs: &[Doc],
    level: Level,
    warnings: &mut Vec<String>,
) -> Result<Mapping, String> {
    let metadata = match level {
        Level::Sugar => dead_sugar(docs),
        Level::Verbatim => None,
    };
    let dead = |m: &str| {
        metadata.and_then(|md| {
            md.iter()
                .find(|d| d.marker == m)
                .map(|d| d.identity.clone())
        })
    };
    let plan = plan(docs, level, &dead);
    let mut out = Mapping::new();
    insert(&mut out, "format", Value::Number(4.into()));
    let mut names = std::collections::BTreeMap::new();
    for doc in docs {
        for (k, v) in &doc.ext.display_names {
            if names
                .insert(k.clone(), v.clone())
                .is_some_and(|old| old != *v)
            {
                return Err(format!("{} differs in the display name for {k}", doc.name));
            }
        }
    }
    let primary = tag.split('-').next().unwrap_or(tag).to_string();
    let first = docs.first().ok_or("no documents")?;
    if !names.keys().any(|k| k.eq_ignore_ascii_case(&primary)) {
        warnings.push(format!(
            "no display name for {primary}; the autonym is taken from info@name"
        ));
        names.insert(primary.clone(), first.kb.info.name.clone());
    }
    let autonym = names
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(&primary))
        .map(|(_, v)| v.clone())
        .unwrap_or_default();
    let mut dn = Mapping::new();
    for (k, v) in &names {
        insert(&mut dn, k, Value::String(v.clone()));
    }
    insert(&mut out, "displayNames", Value::Mapping(dn));
    let fields: [(&str, &Field); 6] = [
        ("info", &|d| info(d.kb, &autonym)),
        ("version", &|d| {
            d.kb.version.clone().map_or(Value::Null, Value::String)
        }),
        ("locales", &|d| {
            if d.kb.locales.is_empty() {
                Value::Null
            } else {
                Value::Sequence(d.kb.locales.iter().cloned().map(Value::String).collect())
            }
        }),
        ("normalization", &|d| match d.kb.normalization {
            Normalization::Enabled => Value::String("enabled".to_string()),
            Normalization::Disabled => Value::Null,
        }),
        ("decimal", &|d| {
            d.kb.decimal.as_ref().map_or(Value::Null, |t| {
                Value::String(encode_text(&pieces(d.kb, t)))
            })
        }),
        ("keyNames", &|d| {
            let mut m = Mapping::new();
            let labels = &d.kb.displays.labels;
            for (k, v) in [("space", &labels.space), ("return", &labels.r#return)] {
                if let Some(v) = v {
                    insert(&mut m, k, Value::String(kbd_ldml::escape::encode_plain(v)));
                }
            }
            if m.is_empty() {
                Value::Null
            } else {
                Value::Mapping(m)
            }
        }),
    ];
    for (name, value) in fields {
        let v = shared(docs, name, value)?;
        if !v.is_null() {
            insert(&mut out, name, v);
        }
    }
    if let Some(md) = metadata {
        insert(&mut out, "deadKeys", fields::dead_keys(first.kb, md));
    }
    let drop_generated = metadata.is_some();
    if !drop_generated && docs.iter().any(|d| !d.ext.generated.is_empty()) {
        warnings.push(
            "groups generated for dead keys are kept verbatim: re-deriving them from deadKeys does not reproduce them"
                .to_string(),
        );
    }
    let verbatim: [(&str, &Field); 3] = [
        ("variables", &|d| fields::variables(&d.canon)),
        ("transforms", &|d| {
            fields::groups(&d.canon, "simple", drop_generated)
        }),
        ("backspace", &|d| {
            fields::groups(&d.canon, "backspace", drop_generated)
        }),
    ];
    for (name, value) in verbatim {
        let v = shared(docs, name, value)?;
        if !v.is_null() {
            insert(&mut out, name, v);
        }
    }
    let mut keys_map = Mapping::new();
    let mut flicks_map = Mapping::new();
    for (di, doc) in docs.iter().enumerate() {
        let classes = &plan.classes[di];
        for (key, class) in doc.kb.keys.iter().zip(classes) {
            if *class != KeyTok::Explicit {
                continue;
            }
            let value = fields::explicit_key(doc.kb, key);
            match keys_map.get(key.id.as_str()) {
                Some(v) if *v != value => {
                    return Err(format!("{} differs in key {}", doc.name, key.id));
                }
                Some(_) => {}
                None => insert(&mut keys_map, &key.id, value),
            }
        }
        for flick in &doc.kb.flicks {
            let owned = doc.kb.keys.iter().zip(classes).any(|(k, c)| {
                matches!(c, KeyTok::Made(_))
                    && k.flick.and_then(|f| doc.kb.flicks.get(usize::from(f))) == Some(flick)
            });
            if owned {
                continue;
            }
            let value = fields::explicit_flick(doc.kb, flick);
            match flicks_map.get(flick.id.as_str()) {
                Some(v) if *v != value => {
                    return Err(format!("{} differs in flick {}", doc.name, flick.id));
                }
                Some(_) => {}
                None => insert(&mut flicks_map, &flick.id, value),
            }
        }
    }
    if !keys_map.is_empty() {
        insert(&mut out, "keys", Value::Mapping(keys_map));
    }
    if !flicks_map.is_empty() {
        insert(&mut out, "flicks", Value::Mapping(flicks_map));
    }
    let markers = metadata
        .map(|md| dead_markers(md, first.kb))
        .unwrap_or_default();
    let displays = shared(docs, "displays", &|d| {
        fields::displays(d.kb, &|x| {
            level == Level::Sugar && auto_display(d.kb, x, &markers)
        })
    })?;
    if !displays.is_null() {
        insert(&mut out, "displays", displays);
    }
    let mut lp = Mapping::new();
    for (output, _, di, k) in &plan.long_press {
        let doc = &docs[*di];
        let tokens: Vec<String> = doc
            .kb
            .key(*k)
            .map(|key| {
                key.long_press
                    .iter()
                    .map(|c| token_of(doc.kb, &plan.classes[*di], *c))
                    .collect()
            })
            .unwrap_or_default();
        insert(
            &mut lp,
            &encode_text(output),
            Value::String(tokens.join(" ")),
        );
    }
    if !lp.is_empty() {
        insert(&mut out, "longPress", Value::Mapping(lp));
    }
    let (hardware, touch) = variants(docs, &plan, level, warnings)?;
    if !hardware.is_empty() {
        insert(&mut out, "hardware", Value::Mapping(hardware));
    }
    if !touch.is_empty() {
        insert(&mut out, "touch", Value::Mapping(touch));
    }
    let emoji = shared(docs, "emoji", &|d| fields::emoji(d.kb, &mut Vec::new()))?;
    fields::emoji(first.kb, warnings);
    if !emoji.is_null() {
        insert(&mut out, "emoji", emoji);
    }
    let targets = shared(docs, "targets", &|d| {
        fields::targets(d.kb, &d.ext.targets, &mut Vec::new())
    })?;
    fields::targets(first.kb, &first.ext.targets, warnings);
    if !targets.is_null() {
        insert(&mut out, "targets", targets);
    }
    Ok(out)
}

/// The hardware and touch variants, one per document and host, with
/// host variants equal to `default` left out, since hosts fall back to
/// it.
fn variants(
    docs: &[Doc],
    plan: &Plan,
    level: Level,
    warnings: &mut Vec<String>,
) -> Result<(Mapping, Mapping), String> {
    let mut hardware = Mapping::new();
    let mut touch = Mapping::new();
    for (di, doc) in docs.iter().enumerate() {
        let (hw_name, touch_name) = variant_names(doc.host);
        let classes = &plan.classes[di];
        match (hw_name, &doc.kb.hardware) {
            (Some(name), Some(_)) => {
                if hardware.contains_key(name) {
                    return Err(format!("two documents write hardware variant {name}"));
                }
                insert(&mut hardware, name, layers::hardware(doc, classes, level));
            }
            (None, Some(_)) => warnings.push(format!(
                "{}: the hardware layers of a {} document are dropped",
                doc.name,
                doc.host.map_or("default", Host::name)
            )),
            _ => {}
        }
        match (touch_name, doc.kb.touch.is_empty()) {
            (Some(name), false) => {
                if touch.contains_key(name) {
                    return Err(format!("two documents write touch variant {name}"));
                }
                let lp = plan.touch_long_press.get(di).cloned().unwrap_or_default();
                insert(&mut touch, name, layers::touch(doc, classes, lp));
            }
            (None, false) => warnings.push(format!(
                "{}: the touch layers of a {} document are dropped",
                doc.name,
                doc.host.map_or("default", Host::name)
            )),
            _ => {}
        }
    }
    for map in [&mut hardware, &mut touch] {
        if let Some(default) = map.get("default").cloned() {
            map.retain(|k, v| k.as_str() == Some("default") || *v != default);
        }
    }
    Ok((hardware, touch))
}

/// The names of the model fields in which `a` and `b` differ, hosts
/// aside.
fn differences(a: &Keyboard, b: &Keyboard) -> Vec<&'static str> {
    let mut out = Vec::new();
    macro_rules! field {
        ($($f:ident),*) => {
            $(if a.$f != b.$f { out.push(stringify!($f)); })*
        };
    }
    field!(
        locale,
        locales,
        conforms_to,
        version,
        info,
        normalization,
        markers,
        keys,
        flicks,
        displays,
        hardware,
        touch,
        sets,
        classes,
        simple,
        backspace,
        context_len,
        decimal,
        flush,
        dead_key_names,
        windows,
        emoji
    );
    out
}

/// What lowering `layout` gives for each document, as differences from
/// its model; an error when the layout does not load, lower or resolve.
fn check(tag: &str, dest: &Path, layout: &Mapping, docs: &[Doc]) -> Result<Vec<String>, String> {
    let value = Value::Mapping(layout.clone());
    let file = dest.display().to_string();
    let loaded = parse(&file, dest, tag, &value).map_err(|e| e.to_string())?;
    let lowered = lower(&loaded).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for doc in docs {
        let host = doc.host.unwrap_or(Host::Web);
        let Some((_, source)) = lowered.iter().find(|(h, _)| *h == host) else {
            out.push(format!("{}: no {} document", doc.name, host.name()));
            continue;
        };
        let resolved = kbd_ldml::resolve(source).map_err(|e| e.to_string())?;
        let mut keyboard = resolved.keyboard;
        keyboard.host = doc.kb.host;
        for field in differences(doc.kb, &keyboard) {
            out.push(format!("{}: {field} differs", doc.name));
        }
    }
    Ok(out)
}

// [spec:kbdgen:sem:ldml.yaml.import+1]
// [spec:kbdgen:thm:ldml.yaml.roundtrip+1]
/// Writes the documents of one layout tag as a v4 layout destined for
/// `dest`. The sugared layout is chosen when every difference lowering it
/// leaves is one the verbatim layout leaves too; otherwise the verbatim
/// layout, with a warning naming what the sugar did not reproduce. Each
/// remaining difference is a warning: a model field v4 cannot write.
pub fn import(tag: &str, dest: &Path, files: &[ImportedFile]) -> Result<Imported, LdmlError> {
    let docs: Vec<Doc> = files
        .iter()
        .map(|f| Doc {
            name: f.path.display().to_string(),
            host: f.host,
            kb: &f.resolved.keyboard,
            ext: &f.resolved.extensions,
            canon: kbd_ldml::export(&f.resolved.keyboard, &f.resolved.extensions),
        })
        .collect();
    let mut outcomes = Vec::new();
    for level in [Level::Sugar, Level::Verbatim] {
        let mut warnings = Vec::new();
        let outcome = build(tag, &docs, level, &mut warnings)
            .and_then(|m| check(tag, dest, &m, &docs).map(|d| (m, d)));
        outcomes.push((outcome, warnings));
    }
    let mut outcomes = outcomes.into_iter();
    let (sugar, mut sugar_warnings) = outcomes.next().unwrap_or((Err(String::new()), Vec::new()));
    let (verbatim, mut verbatim_warnings) =
        outcomes.next().unwrap_or((Err(String::new()), Vec::new()));
    let (layout, diffs, mut warnings) = match (sugar, verbatim) {
        (Ok((layout, diffs)), Ok((_, plain))) if diffs.iter().all(|d| plain.contains(d)) => {
            (layout, diffs, sugar_warnings)
        }
        (Ok((_, diffs)), Ok((layout, plain))) => {
            let extra: Vec<&String> = diffs.iter().filter(|d| !plain.contains(d)).collect();
            verbatim_warnings.push(format!(
                "{tag}: written without sugar, since re-deriving it does not reproduce: {}",
                extra
                    .iter()
                    .map(|d| d.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
            (layout, plain, verbatim_warnings)
        }
        (Ok((layout, diffs)), Err(reason)) => {
            if !diffs.is_empty() {
                sugar_warnings.push(format!("{tag}: no verbatim layout: {reason}"));
            }
            (layout, diffs, sugar_warnings)
        }
        (Err(reason), Ok((layout, plain))) => {
            verbatim_warnings.push(format!("{tag}: written without sugar: {reason}"));
            (layout, plain, verbatim_warnings)
        }
        (Err(sugar), Err(verbatim)) => {
            return Err(LdmlError::Layout {
                tag: tag.to_string(),
                message: format!("{sugar}; {verbatim}"),
            });
        }
    };
    warnings.extend(
        diffs
            .into_iter()
            .map(|d| format!("{d}; v4 cannot write it")),
    );
    let yaml = serde_yaml::to_string(&Value::Mapping(layout)).map_err(|e| LdmlError::Layout {
        tag: tag.to_string(),
        message: e.to_string(),
    })?;
    Ok(Imported { yaml, warnings })
}
