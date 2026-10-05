//! The v4 layout as loaded (`ldml.yaml.schema`): every field checked
//! strictly, every string that LDML escape-decodes decoded once, and
//! hardware and touch inheritance resolved.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use kbd_ldml::Target;
use kbd_ldml::escape::Piece;
use kbd_model::{
    BottomRow, Direction, Emoji, ExtraModifierKey, Form, ModifierSet, Normalization, Role,
    WINDOWS_KEY_NAMES,
};
use serde_yaml::Value;

use super::error::{At, Result, YamlProblem};
use super::node::{Fields, boolean, entries, list, number_text, string};
use super::text::{Strings, Syntax, check_escapes, hex_name, output, plain};
use super::tokens::{Token, candidates, width};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Info4 {
    pub name: Option<String>,
    pub author: Option<String>,
    pub layout: Option<String>,
    pub indicator: Option<String>,
    pub attribution: Option<String>,
}

/// `ldml:`, with paths already joined to the layout file's directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LdmlRef {
    All(PathBuf),
    /// Host name or `default` → path, in file order.
    PerHost(Vec<(String, PathBuf)>),
}

/// A `deadKeys` node. A nested node's identity is its parent's identity
/// followed by its input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeadNode {
    pub identity: String,
    pub marker: String,
    pub display: String,
    pub standalone: String,
    pub name: Option<String>,
    pub compose: Vec<ComposeEntry>,
    pub at: At,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComposeEntry {
    pub input: String,
    pub value: ComposeTo,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComposeTo {
    Output(String),
    Node(DeadNode),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VarKind {
    String,
    Set,
    Uset,
}

/// A `variables` entry, as written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Variable {
    pub kind: VarKind,
    pub id: String,
    pub value: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Reorder4 {
    pub from: String,
    pub before: Option<String>,
    pub order: Option<String>,
    pub tertiary: Option<String>,
    pub tertiary_base: Option<String>,
    pub pre_base: Option<String>,
}

/// A transform group, as written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Group {
    Rules(Vec<(String, Option<String>)>),
    Reorder(Vec<Reorder4>),
}

/// A `keys` entry: LDML key attributes as written, with the output also
/// decoded.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExplicitKey {
    pub id: String,
    pub output: Option<String>,
    pub decoded: Vec<Piece>,
    pub gap: bool,
    pub layer: Option<String>,
    pub width: Option<String>,
    pub stretch: bool,
    pub long_press: Option<String>,
    pub long_press_default: Option<String>,
    pub multi_tap: Option<String>,
    pub flick: Option<String>,
    pub role: Option<Role>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlickDef {
    pub id: String,
    /// (directions, key id), as written.
    pub segments: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DisplayTarget4 {
    /// The output as written, and decoded.
    Output(String, Vec<Piece>),
    KeyId(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisplayDef {
    pub target: DisplayTarget4,
    pub display: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LongPressEntry {
    pub output: Vec<Piece>,
    pub candidates: Vec<Token>,
    pub at: At,
}

/// The parts of `targets` that reach a document (`ldml.yaml.targets`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Targets4 {
    pub entries: Vec<Target>,
    pub shift_lock: bool,
    pub lrm_rlm: bool,
    pub key_names: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormSpec {
    Implied(String),
    Custom(Form),
}

/// A hardware layer as written: its key, the sets it names, its rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayerSpec {
    pub key: String,
    pub sets: Vec<ModifierSet>,
    pub rows: Vec<Vec<Token>>,
    pub at: At,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpaceSpec {
    pub key: String,
    pub sets: Vec<ModifierSet>,
    pub token: Token,
    pub at: At,
}

/// A hardware variant with `inherits` resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HardwareVariant {
    pub name: String,
    pub at: At,
    pub form: FormSpec,
    /// `impliedLayers: macOS`.
    pub implied: bool,
    pub layers: Vec<LayerSpec>,
    pub space: Vec<SpaceSpec>,
    pub extra_modifiers: Vec<ExtraModifierKey>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlickRows {
    pub key: String,
    pub directions: Vec<Direction>,
    pub rows: Vec<Vec<Token>>,
    pub at: At,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TouchLayerSpec {
    pub id: String,
    pub rows: Vec<Vec<Token>>,
    pub flicks: Vec<FlickRows>,
    pub at: At,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Size {
    pub name: String,
    pub min_device_width: Option<u16>,
    pub bottom_row: BottomRow,
    pub layers: Vec<TouchLayerSpec>,
    pub at: At,
}

/// A touch variant with `inherits` resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TouchVariant {
    pub name: String,
    pub at: At,
    pub long_press: Vec<LongPressEntry>,
    pub sizes: Vec<Size>,
}

/// A loaded v4 layout file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout4 {
    pub file: String,
    pub dir: PathBuf,
    pub tag: String,
    pub display_names: BTreeMap<String, String>,
    pub info: Info4,
    pub version: Option<String>,
    pub locales: Vec<String>,
    pub normalization: Normalization,
    pub decimal: Option<Vec<Piece>>,
    pub space_label: Option<String>,
    pub return_label: Option<String>,
    pub ldml: Option<LdmlRef>,
    pub dead_keys: Vec<DeadNode>,
    pub strings: Strings,
    pub variables: Vec<Variable>,
    pub transforms: Vec<Group>,
    pub backspace: Vec<Group>,
    pub keys: Vec<ExplicitKey>,
    pub flicks: Vec<FlickDef>,
    pub displays: Vec<DisplayDef>,
    pub display_base: Option<String>,
    pub long_press: Vec<LongPressEntry>,
    pub hardware: Vec<HardwareVariant>,
    pub touch: Vec<TouchVariant>,
    pub emoji: Emoji,
    pub targets: Targets4,
    pub warnings: Vec<YamlProblem>,
}

/// Fields that a layout with `ldml:` may not have (`ldml.yaml.ldml-ref`).
const NOT_WITH_LDML: [&str; 14] = [
    "deadKeys",
    "variables",
    "transforms",
    "backspace",
    "keys",
    "flicks",
    "displays",
    "longPress",
    "hardware",
    "touch",
    "normalization",
    "info",
    "version",
    "locales",
];

fn optional_string(fields: &mut Fields, name: &str) -> Result<Option<String>> {
    fields
        .take(name)
        .map(|(v, at)| string(v, &at).map(str::to_string))
        .transpose()
}

fn info(value: &Value, at: &At) -> Result<Info4> {
    let mut f = Fields::new(value, at)?;
    let info = Info4 {
        name: optional_string(&mut f, "name")?,
        author: optional_string(&mut f, "author")?,
        layout: optional_string(&mut f, "layout")?,
        indicator: optional_string(&mut f, "indicator")?,
        attribution: optional_string(&mut f, "attribution")?,
    };
    f.finish()?;
    Ok(info)
}

/// Letters, digits, `_` and `-`: a marker that also names the key
/// `dk-<marker>`.
fn check_marker(marker: &str, at: &At) -> Result<()> {
    let ok = !marker.is_empty()
        && marker
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    if ok {
        Ok(())
    } else {
        Err(at.error(format!(
            "marker {marker:?} must be letters, digits, _ and -, since it names the key dk-{marker}"
        )))
    }
}

struct Parent<'a> {
    node: &'a DeadNode,
    input: &'a str,
}

// [spec:kbdgen:def:ldml.yaml.dead-keys]
// [spec:kbdgen:sem:ldml.yaml.dead-keys.keys]
fn dead_node(identity: String, value: &Value, at: &At, parent: Option<Parent>) -> Result<DeadNode> {
    let mut f = Fields::new(value, at)?;
    let marker = match f.take("marker") {
        Some((v, a)) => {
            let m = string(v, &a)?.to_string();
            check_marker(&m, &a)?;
            m
        }
        None => match &parent {
            None => format!("dk_{}", hex_name(identity.chars(), "_")),
            Some(p) => format!("{}-{}", p.node.marker, hex_name(p.input.chars(), "_")),
        },
    };
    let display = match f.take("display") {
        Some((v, a)) => plain(string(v, &a)?, &a)?,
        None => identity.clone(),
    };
    let standalone = match f.take("standalone") {
        Some((v, a)) => plain(string(v, &a)?, &a)?,
        None => identity.clone(),
    };
    let name = match f.take("name") {
        Some((_, a)) if parent.is_some() => {
            return Err(a.error("only a top-level dead key has a name"));
        }
        Some((v, a)) => Some(plain(string(v, &a)?, &a)?),
        None => None,
    };
    let mut node = DeadNode {
        identity,
        marker,
        display,
        standalone,
        name,
        compose: Vec::new(),
        at: at.clone(),
    };
    if let Some((v, a)) = f.take("compose") {
        for (raw, value, entry_at) in entries(v, &a)? {
            let input = plain(raw, &entry_at)?;
            if input == " " {
                return Err(entry_at.error("the input \" \" is what standalone is for"));
            }
            if input.is_empty() {
                return Err(entry_at.error("an input is not empty"));
            }
            if node.compose.iter().any(|c| c.input == input) {
                return Err(entry_at.error(format!("the input {input:?} occurs twice")));
            }
            let value = match value {
                Value::String(s) => ComposeTo::Output(plain(s, &entry_at)?),
                _ => {
                    let child_identity = format!("{}{input}", node.identity);
                    let parent = Parent {
                        node: &node,
                        input: &input,
                    };
                    ComposeTo::Node(dead_node(child_identity, value, &entry_at, Some(parent))?)
                }
            };
            node.compose.push(ComposeEntry { input, value });
        }
    }
    f.finish()?;
    Ok(node)
}

fn collect_markers<'a>(node: &'a DeadNode, out: &mut Vec<(&'a str, &'a At)>) {
    out.push((&node.marker, &node.at));
    for entry in &node.compose {
        if let ComposeTo::Node(child) = &entry.value {
            collect_markers(child, out);
        }
    }
}

fn dead_keys(value: &Value, at: &At) -> Result<Vec<DeadNode>> {
    let mut nodes: Vec<DeadNode> = Vec::new();
    for (raw, v, a) in entries(value, at)? {
        let identity = plain(raw, &a)?;
        if identity.is_empty() {
            return Err(a.error("a dead-key identity is not empty"));
        }
        if nodes.iter().any(|n| n.identity == identity) {
            return Err(a.error(format!("the identity {identity:?} occurs twice")));
        }
        nodes.push(dead_node(identity, v, &a, None)?);
    }
    let mut markers = Vec::new();
    for node in &nodes {
        collect_markers(node, &mut markers);
    }
    for (i, (marker, a)) in markers.iter().enumerate() {
        if markers.iter().take(i).any(|(m, _)| m == marker) {
            return Err(a.error(format!("the marker {marker} is used twice")));
        }
    }
    Ok(nodes)
}

// [spec:kbdgen:def:ldml.yaml.verbatim]
fn variables(value: &Value, at: &At, strings: &mut Strings) -> Result<Vec<Variable>> {
    let mut f = Fields::new(value, at)?;
    let mut out = Vec::new();
    for (field, kind, syntax) in [
        ("strings", VarKind::String, Syntax::Text),
        ("sets", VarKind::Set, Syntax::Text),
        ("usets", VarKind::Uset, Syntax::Regex),
    ] {
        let Some((v, a)) = f.take(field) else {
            continue;
        };
        for (id, value, va) in entries(v, &a)? {
            let value = string(value, &va)?;
            check_escapes(value, syntax, &va)?;
            if kind == VarKind::String {
                strings.define(id, value, &va)?;
            }
            out.push(Variable {
                kind,
                id: id.to_string(),
                value: value.to_string(),
            });
        }
    }
    f.finish()?;
    Ok(out)
}

fn regex_field(fields: &mut Fields, name: &str) -> Result<Option<String>> {
    match fields.take(name) {
        Some((v, a)) => {
            let s = string(v, &a)?;
            check_escapes(s, Syntax::Regex, &a)?;
            Ok(Some(s.to_string()))
        }
        None => Ok(None),
    }
}

fn number_field(fields: &mut Fields, name: &str) -> Result<Option<String>> {
    fields
        .take(name)
        .map(|(v, a)| number_text(v, &a))
        .transpose()
}

fn reorder(value: &Value, at: &At) -> Result<Reorder4> {
    let mut f = Fields::new(value, at)?;
    let (from, from_at) = f.require("from")?;
    let from = string(from, &from_at)?.to_string();
    check_escapes(&from, Syntax::Regex, &from_at)?;
    let r = Reorder4 {
        from,
        before: regex_field(&mut f, "before")?,
        order: number_field(&mut f, "order")?,
        tertiary: number_field(&mut f, "tertiary")?,
        tertiary_base: number_field(&mut f, "tertiaryBase")?,
        pre_base: number_field(&mut f, "preBase")?,
    };
    f.finish()?;
    Ok(r)
}

// [spec:kbdgen:def:ldml.yaml.verbatim]
fn groups(value: &Value, at: &At) -> Result<Vec<Group>> {
    let mut out = Vec::new();
    for (group, ga) in list(value, at)? {
        if let Value::Mapping(_) = group {
            let mut f = Fields::new(group, &ga)?;
            let (items, ia) = f.require("reorder")?;
            let rules = list(items, &ia)?
                .into_iter()
                .map(|(v, a)| reorder(v, &a))
                .collect::<Result<Vec<_>>>()?;
            f.finish()?;
            out.push(Group::Reorder(rules));
            continue;
        }
        let mut rules = Vec::new();
        for (item, a) in list(group, &ga)? {
            let mut f = Fields::new(item, &a)?;
            let (from, from_at) = f.require("from")?;
            let from = string(from, &from_at)?.to_string();
            check_escapes(&from, Syntax::Regex, &from_at)?;
            let to = regex_field(&mut f, "to")?;
            f.finish()?;
            rules.push((from, to));
        }
        if rules.is_empty() {
            return Err(ga.error("a transform group has at least one transform"));
        }
        out.push(Group::Rules(rules));
    }
    Ok(out)
}

fn key_list_field(f: &mut Fields, name: &str) -> Result<Option<String>> {
    f.take(name)
        .map(|(v, a)| string(v, &a).map(str::to_string))
        .transpose()
}

// [spec:kbdgen:def:ldml.yaml.verbatim]
fn explicit_keys(value: &Value, at: &At, strings: &Strings) -> Result<Vec<ExplicitKey>> {
    let mut out = Vec::new();
    for (id, v, a) in entries(value, at)? {
        let mut f = Fields::new(v, &a)?;
        let mut key = ExplicitKey {
            id: id.to_string(),
            ..ExplicitKey::default()
        };
        if let Some((v, oa)) = f.take("output") {
            let raw = string(v, &oa)?;
            key.decoded = output(raw, strings, &oa)?;
            key.output = Some(raw.to_string());
        }
        if let Some((v, ga)) = f.take("gap") {
            key.gap = boolean(v, &ga)?;
        }
        if let Some((v, sa)) = f.take("stretch") {
            key.stretch = boolean(v, &sa)?;
        }
        if let Some((v, wa)) = f.take("width") {
            let text = number_text(v, &wa)?;
            width(&text, &wa)?;
            key.width = Some(text);
        }
        key.layer = key_list_field(&mut f, "layer")?;
        key.long_press = key_list_field(&mut f, "longPress")?;
        key.long_press_default = key_list_field(&mut f, "longPressDefault")?;
        key.multi_tap = key_list_field(&mut f, "multiTap")?;
        key.flick = key_list_field(&mut f, "flick")?;
        if let Some((v, ra)) = f.take("role") {
            let name = string(v, &ra)?;
            key.role = Some(
                Role::from_name(name).ok_or_else(|| ra.error(format!("{name} is not a role")))?,
            );
        }
        f.finish()?;
        out.push(key);
    }
    Ok(out)
}

fn flicks(value: &Value, at: &At) -> Result<Vec<FlickDef>> {
    let mut out = Vec::new();
    for (id, v, a) in entries(value, at)? {
        let mut segments = Vec::new();
        for (item, ia) in list(v, &a)? {
            let mut f = Fields::new(item, &ia)?;
            let (d, da) = f.require("directions")?;
            let (k, ka) = f.require("key")?;
            segments.push((string(d, &da)?.to_string(), string(k, &ka)?.to_string()));
            f.finish()?;
        }
        out.push(FlickDef {
            id: id.to_string(),
            segments,
        });
    }
    Ok(out)
}

fn displays(
    value: &Value,
    at: &At,
    strings: &Strings,
) -> Result<(Vec<DisplayDef>, Option<String>)> {
    let mut out = Vec::new();
    let mut base = None;
    for (item, a) in list(value, at)? {
        let mut f = Fields::new(item, &a)?;
        if let Some((v, ba)) = f.take("displayBase") {
            if base.is_some() {
                return Err(ba.error("displayBase occurs twice"));
            }
            let raw = string(v, &ba)?;
            plain(raw, &ba)?;
            base = Some(raw.to_string());
            f.finish()?;
            continue;
        }
        let target = match (f.take("output"), f.take("keyId")) {
            (Some((v, oa)), None) => {
                let raw = string(v, &oa)?;
                DisplayTarget4::Output(raw.to_string(), output(raw, strings, &oa)?)
            }
            (None, Some((v, ka))) => DisplayTarget4::KeyId(string(v, &ka)?.to_string()),
            _ => return Err(a.error("a display has exactly one of output and keyId")),
        };
        let (d, da) = f.require("display")?;
        let display = string(d, &da)?;
        check_escapes(display, Syntax::Text, &da)?;
        f.finish()?;
        out.push(DisplayDef {
            target,
            display: display.to_string(),
        });
    }
    Ok((out, base))
}

// [spec:kbdgen:def:ldml.yaml.long-press]
/// `longPress`: decoded output → candidate tokens.
pub fn long_press(value: &Value, at: &At, strings: &Strings) -> Result<Vec<LongPressEntry>> {
    let mut out: Vec<LongPressEntry> = Vec::new();
    for (raw, v, a) in entries(value, at)? {
        let output = output(raw, strings, &a)?;
        if out.iter().any(|e| e.output == output) {
            return Err(a.error(format!("the output {raw} occurs twice")));
        }
        let tokens = candidates(string(v, &a)?, strings, &a)?;
        out.push(LongPressEntry {
            output,
            candidates: tokens,
            at: a,
        });
    }
    Ok(out)
}

fn ldml_ref(value: &Value, at: &At, dir: &Path) -> Result<LdmlRef> {
    if let Value::String(path) = value {
        return Ok(LdmlRef::All(dir.join(path)));
    }
    let mut out = Vec::new();
    for (host, v, a) in entries(value, at)? {
        if host != "default" && kbd_model::Host::from_name(host).is_none() {
            return Err(a.error(format!("{host} is neither a host nor default")));
        }
        out.push((host.to_string(), dir.join(string(v, &a)?)));
    }
    Ok(LdmlRef::PerHost(out))
}

const TARGET_FIELDS: [(&str, &[&str]); 4] = [
    ("windows", &["locale", "id"]),
    ("chromeOS", &["locale", "xkbLayout"]),
    ("iOS", &["spellerPackageKey", "spellerPath"]),
    ("android", &["spellerPackageKey", "spellerPath"]),
];

fn targets(value: &Value, at: &At) -> Result<Targets4> {
    let mut f = Fields::new(value, at)?;
    let mut out = Targets4::default();
    for (host, names) in TARGET_FIELDS {
        let Some((v, a)) = f.take(host) else {
            continue;
        };
        let mut t = Fields::new(v, &a)?;
        for name in names {
            if let Some(value) = optional_string(&mut t, name)? {
                out.entries.push(Target {
                    host: host.to_string(),
                    name: name.to_string(),
                    value,
                });
            }
        }
        if host == "windows" {
            if let Some((v, ba)) = t.take("shiftLock") {
                out.shift_lock = boolean(v, &ba)?;
            }
            if let Some((v, ba)) = t.take("lrmRlm") {
                out.lrm_rlm = boolean(v, &ba)?;
            }
            if let Some((v, ka)) = t.take("keyNames") {
                for (name, n, na) in entries(v, &ka)? {
                    if !WINDOWS_KEY_NAMES.contains(&name) {
                        return Err(na.error(format!("{name} is not a Windows key name")));
                    }
                    out.key_names
                        .push((name.to_string(), string(n, &na)?.to_string()));
                }
            }
        }
        t.finish()?;
    }
    f.finish()?;
    Ok(out)
}

fn display_names(value: &Value, at: &At) -> Result<BTreeMap<String, String>> {
    entries(value, at)?
        .into_iter()
        .map(|(k, v, a)| Ok((k.to_string(), string(v, &a)?.to_string())))
        .collect()
}

/// The autonym requirement (`bundle.layouts.autonym`): a display name for
/// the tag's primary language.
fn check_autonym(names: &BTreeMap<String, String>, tag: &str, at: &At) -> Result<()> {
    let primary = tag.split('-').next().unwrap_or(tag).to_ascii_lowercase();
    if names.keys().any(|k| k.to_ascii_lowercase() == primary) {
        Ok(())
    } else {
        Err(at.error(format!("displayNames has no autonym, a name for {primary}")))
    }
}

/// Whether `token` or any token of `rows` uses a `\d{}` identity, for the
/// warning about unused dead keys.
fn uses_dead(rows: &[Vec<Token>], identity: &str) -> bool {
    rows.iter()
        .flatten()
        .any(|t| matches!(t, Token::Dead(i) if i == identity))
}

fn unused_dead_keys(layout: &Layout4) -> Vec<YamlProblem> {
    let used = |identity: &str| {
        let lp = |entries: &[LongPressEntry]| {
            entries
                .iter()
                .any(|e| uses_dead(std::slice::from_ref(&e.candidates), identity))
        };
        lp(&layout.long_press)
            || layout.hardware.iter().any(|h| {
                h.layers.iter().any(|l| uses_dead(&l.rows, identity))
                    || h.space
                        .iter()
                        .any(|s| matches!(&s.token, Token::Dead(i) if i == identity))
            })
            || layout.touch.iter().any(|t| {
                lp(&t.long_press)
                    || t.sizes.iter().flat_map(|s| &s.layers).any(|l| {
                        uses_dead(&l.rows, identity)
                            || l.flicks.iter().any(|f| uses_dead(&f.rows, identity))
                    })
            })
    };
    layout
        .dead_keys
        .iter()
        .filter(|n| !used(&n.identity))
        .map(|n| {
            n.at.warning(format!(
                "no \\d{{{}}} token uses this dead key, so no host gets it",
                n.identity
            ))
        })
        .collect()
}

// [spec:kbdgen:def:ldml.yaml.schema]
// [spec:kbdgen:req:ldml.yaml.strict]
// [spec:kbdgen:req:ldml.yaml.ldml-ref]
/// Parses the top-level mapping of a v4 file, `format: 4` already
/// detected. Unknown fields fail at any depth; a layout with `ldml:` may
/// carry only the fields that override the LDML file's kbdgen data.
pub fn parse(file: &str, path: &Path, tag: &str, value: &Value) -> Result<Layout4> {
    let root = At::file(file);
    let dir = path.parent().map(Path::to_path_buf).unwrap_or_default();
    let mut f = Fields::new(value, &root)?;
    f.require("format")?;
    let present = f.names();
    let ldml = match f.take("ldml") {
        Some((v, a)) => {
            if let Some(field) = NOT_WITH_LDML.iter().find(|n| present.contains(n)) {
                return Err(root.key(field).error(format!(
                    "a layout with ldml: may not have {field}; the LDML file defines it"
                )));
            }
            Some(ldml_ref(v, &a, &dir)?)
        }
        None => None,
    };
    let (names, names_at) = f.require("displayNames")?;
    let display_names = display_names(names, &names_at)?;
    check_autonym(&display_names, tag, &names_at)?;
    let mut strings = Strings::default();
    let variables = match f.take("variables") {
        Some((v, a)) => variables(v, &a, &mut strings)?,
        None => Vec::new(),
    };
    let mut layout = Layout4 {
        file: file.to_string(),
        dir: dir.clone(),
        tag: tag.to_string(),
        display_names,
        info: f
            .take("info")
            .map(|(v, a)| info(v, &a))
            .transpose()?
            .unwrap_or_default(),
        version: optional_string(&mut f, "version")?,
        locales: match f.take("locales") {
            Some((v, a)) => list(v, &a)?
                .into_iter()
                .map(|(v, a)| string(v, &a).map(str::to_string))
                .collect::<Result<_>>()?,
            None => Vec::new(),
        },
        // [spec:kbdgen:sem:ldml.yaml.normalization]
        normalization: match f.take("normalization") {
            None => Normalization::Disabled,
            Some((v, a)) => match string(v, &a)? {
                "disabled" => Normalization::Disabled,
                "enabled" => Normalization::Enabled,
                other => {
                    return Err(a.error(format!("{other} is neither disabled nor enabled")));
                }
            },
        },
        decimal: match f.take("decimal") {
            Some((v, a)) => Some(output(string(v, &a)?, &strings, &a)?),
            None => None,
        },
        space_label: None,
        return_label: None,
        ldml,
        dead_keys: match f.take("deadKeys") {
            Some((v, a)) => dead_keys(v, &a)?,
            None => Vec::new(),
        },
        strings: Strings::default(),
        variables,
        transforms: match f.take("transforms") {
            Some((v, a)) => groups(v, &a)?,
            None => Vec::new(),
        },
        backspace: match f.take("backspace") {
            Some((v, a)) => groups(v, &a)?,
            None => Vec::new(),
        },
        keys: match f.take("keys") {
            Some((v, a)) => explicit_keys(v, &a, &strings)?,
            None => Vec::new(),
        },
        flicks: match f.take("flicks") {
            Some((v, a)) => flicks(v, &a)?,
            None => Vec::new(),
        },
        displays: Vec::new(),
        display_base: None,
        long_press: match f.take("longPress") {
            Some((v, a)) => long_press(v, &a, &strings)?,
            None => Vec::new(),
        },
        hardware: match f.take("hardware") {
            Some((v, a)) => super::variants::hardware(v, &a, &strings)?,
            None => Vec::new(),
        },
        touch: match f.take("touch") {
            Some((v, a)) => super::variants::touch(v, &a, &strings)?,
            None => Vec::new(),
        },
        emoji: match f.take("emoji") {
            Some((v, a)) => super::emoji::emoji(v, &a, &dir)?,
            None => Emoji::default(),
        },
        targets: match f.take("targets") {
            Some((v, a)) => targets(v, &a)?,
            None => Targets4::default(),
        },
        warnings: Vec::new(),
    };
    if let Some((v, a)) = f.take("displays") {
        (layout.displays, layout.display_base) = displays(v, &a, &strings)?;
    }
    if let Some((v, a)) = f.take("keyNames") {
        let mut k = Fields::new(v, &a)?;
        for (name, slot) in [
            ("space", &mut layout.space_label),
            ("return", &mut layout.return_label),
        ] {
            if let Some((v, la)) = k.take(name) {
                *slot = Some(plain(string(v, &la)?, &la)?);
            }
        }
        k.finish()?;
    }
    f.finish()?;
    layout.strings = strings;
    layout.warnings = unused_dead_keys(&layout);
    Ok(layout)
}
