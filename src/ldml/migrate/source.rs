//! A v3 layout file (`layout.schema`) as the migrator reads it: every field
//! kept with its raw strings and in file order, the fields v3 never read
//! reported (M10), comments counted (M11), and v2 files recognised (M07).

use serde_yaml::Value;

use super::defect::{Code, Defects};

/// A desktop platform section.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Desktop {
    Windows,
    MacOs,
    ChromeOs,
}

impl Desktop {
    pub const ALL: [Desktop; 3] = [Desktop::Windows, Desktop::MacOs, Desktop::ChromeOs];

    /// The section name in v3, which is also the v4 variant name.
    pub fn name(self) -> &'static str {
        match self {
            Desktop::Windows => "windows",
            Desktop::MacOs => "macOS",
            Desktop::ChromeOs => "chromeOS",
        }
    }

    /// The layer names of `layout.schema.targets`.
    pub fn layer_names(self) -> &'static [&'static str] {
        match self {
            Desktop::Windows => &[
                "default",
                "shift",
                "caps",
                "caps+shift",
                "alt",
                "alt+shift",
                "alt+caps",
                "ctrl",
            ],
            Desktop::MacOs => &[
                "default",
                "shift",
                "caps",
                "caps+shift",
                "alt",
                "alt+shift",
                "alt+caps",
                "ctrl",
                "cmd",
                "cmd+shift",
                "cmd+alt",
                "cmd+alt+shift",
            ],
            Desktop::ChromeOs => &[
                "default",
                "shift",
                "caps",
                "caps+shift",
                "alt",
                "alt+shift",
                "ctrl",
            ],
        }
    }

    fn fields(self) -> &'static [&'static str] {
        match self {
            Desktop::Windows => &["config", "primary", "deadKeys"],
            Desktop::MacOs => &["primary", "deadKeys", "space"],
            Desktop::ChromeOs => &["config", "primary", "deadKeys"],
        }
    }

    fn config_fields(self) -> &'static [&'static str] {
        match self {
            Desktop::Windows => &["locale", "id"],
            Desktop::MacOs => &[],
            Desktop::ChromeOs => &["locale", "xkbLayout"],
        }
    }
}

/// A touch platform section.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Touch {
    Ios,
    Android,
}

impl Touch {
    pub const ALL: [Touch; 2] = [Touch::Ios, Touch::Android];

    pub fn name(self) -> &'static str {
        match self {
            Touch::Ios => "iOS",
            Touch::Android => "android",
        }
    }

    /// The platforms of the section with the v4 size each becomes.
    pub fn platforms(self) -> &'static [(&'static str, &'static str)] {
        match self {
            Touch::Ios => &[
                ("primary", "phone"),
                ("iPad-9in", "tablet"),
                ("iPad-12in", "tablet-large"),
            ],
            Touch::Android => &[("primary", "phone"), ("tablet-600", "tablet")],
        }
    }

    pub fn layer_names(self) -> &'static [&'static str] {
        match self {
            Touch::Ios => &[
                "default",
                "shift",
                "caps",
                "alt",
                "alt+shift",
                "symbols-1",
                "symbols-2",
            ],
            Touch::Android => &["default", "shift"],
        }
    }
}

/// A layer of a platform: its v3 name, its layer string as written, and
/// the YAML path of that string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layer3 {
    pub name: String,
    pub text: String,
    pub path: String,
}

/// A `deadKeys` entry: the layer it names and its raw list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeadList {
    pub layer: String,
    pub entries: Vec<String>,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopSection {
    pub platform: Desktop,
    pub layers: Vec<Layer3>,
    pub dead_keys: Vec<DeadList>,
    /// `macOS.space`: v3 layer name → token.
    pub space: Vec<(String, String, String)>,
    /// The carried-over `config` fields, in file order.
    pub config: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TouchPlatform {
    /// The v3 platform name, such as `iPad-9in`.
    pub name: String,
    pub size: &'static str,
    pub layers: Vec<Layer3>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TouchSection {
    pub platform: Touch,
    pub platforms: Vec<TouchPlatform>,
    pub dead_keys: Vec<DeadList>,
    pub config: Vec<(String, String)>,
}

/// A transform node: a leaf output or a branch from inputs to nodes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Node3 {
    Leaf(String),
    Branch(Vec<(String, Node3)>),
}

/// A v3 layout file as read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source3 {
    pub display_names: Vec<(String, String)>,
    pub decimal: Option<String>,
    pub key_names: Vec<(String, String)>,
    pub longpress: Vec<(String, String)>,
    pub transforms: Vec<(String, Node3)>,
    /// In `Desktop::ALL` order.
    pub desktop: Vec<DesktopSection>,
    /// In `Touch::ALL` order.
    pub touch: Vec<TouchSection>,
}

/// Why a v3 file cannot be read at all.
pub type ReadError = String;

fn describe(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "a boolean",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Sequence(_) => "a list",
        Value::Mapping(_) => "a mapping",
        Value::Tagged(_) => "a tagged value",
    }
}

fn join(path: &str, key: &str) -> String {
    if path.is_empty() {
        key.to_string()
    } else {
        format!("{path}.{key}")
    }
}

/// The string entries of a mapping in file order; null is empty, as v3
/// reads a field whose entries are all commented out.
fn mapping<'a>(value: &'a Value, path: &str) -> Result<Vec<(&'a str, &'a Value)>, ReadError> {
    match value {
        Value::Null => Ok(Vec::new()),
        Value::Mapping(map) => map
            .iter()
            .map(|(k, v)| match k {
                Value::String(k) => Ok((k.as_str(), v)),
                other => Err(format!(
                    "{path}: a key is {}, not a string; v3 requires string keys",
                    describe(other)
                )),
            })
            .collect(),
        other => Err(format!(
            "{path}: expected a mapping, found {}",
            describe(other)
        )),
    }
}

fn text(value: &Value, path: &str) -> Result<String, ReadError> {
    match value {
        Value::String(s) => Ok(s.clone()),
        other => Err(format!(
            "{path}: expected a string, found {}; v3 requires YAML strings",
            describe(other)
        )),
    }
}

fn strings(value: &Value, path: &str) -> Result<Vec<(String, String)>, ReadError> {
    mapping(value, path)?
        .into_iter()
        .map(|(k, v)| Ok((k.to_string(), text(v, &join(path, k))?)))
        .collect()
}

/// Reports every field of `entries` not in `known` as never read by v3.
fn unread(entries: &[(&str, &Value)], known: &[&str], path: &str, defects: &mut Defects) {
    for (name, _) in entries {
        if !known.contains(name) {
            defects.add(
                Code::M10,
                &join(path, name),
                format!("{name} is not a v3 field; v3 never read it"),
            );
        }
    }
}

fn layers(
    value: &Value,
    path: &str,
    names: &[&str],
    platform: &str,
) -> Result<Vec<Layer3>, ReadError> {
    let mut out = Vec::new();
    for (name, v) in mapping(value, path)? {
        if !names.contains(&name) {
            return Err(format!(
                "{}: {name} is not a {platform} layer; one of {}",
                join(path, name),
                names.join(", ")
            ));
        }
        out.push(Layer3 {
            name: name.to_string(),
            text: text(v, &join(path, name))?,
            path: join(path, name),
        });
    }
    Ok(out)
}

fn platform_layers(
    value: &Value,
    path: &str,
    names: &[&str],
    platform: &str,
    defects: &mut Defects,
) -> Result<Vec<Layer3>, ReadError> {
    let entries = mapping(value, path)?;
    unread(&entries, &["layers"], path, defects);
    match entries.iter().find(|(k, _)| *k == "layers") {
        Some((_, v)) => layers(v, &join(path, "layers"), names, platform),
        None => Ok(Vec::new()),
    }
}

fn dead_lists(value: &Value, path: &str) -> Result<Vec<DeadList>, ReadError> {
    let mut out = Vec::new();
    for (layer, v) in mapping(value, path)? {
        let at = join(path, layer);
        let entries = match v {
            Value::Null => Vec::new(),
            Value::Sequence(items) => items
                .iter()
                .enumerate()
                .map(|(i, item)| text(item, &format!("{at}[{}]", i + 1)))
                .collect::<Result<_, _>>()?,
            other => return Err(format!("{at}: expected a list, found {}", describe(other))),
        };
        out.push(DeadList {
            layer: layer.to_string(),
            entries,
            path: at,
        });
    }
    Ok(out)
}

/// The carried-over fields of a `config`, the others reported M10. A null
/// `config` is omitted silently.
fn config(
    value: &Value,
    path: &str,
    known: &[&str],
    defects: &mut Defects,
) -> Result<Vec<(String, String)>, ReadError> {
    let entries = mapping(value, path)?;
    let mut out = Vec::new();
    for (name, v) in entries {
        if known.contains(&name) {
            out.push((name.to_string(), text(v, &join(path, name))?));
        } else {
            defects.add(
                Code::M10,
                &join(path, name),
                format!("{name} is not carried over; v3 never read it"),
            );
        }
    }
    Ok(out)
}

fn desktop(
    platform: Desktop,
    value: &Value,
    defects: &mut Defects,
) -> Result<DesktopSection, ReadError> {
    let path = platform.name();
    let entries = mapping(value, path)?;
    unread(&entries, platform.fields(), path, defects);
    let mut section = DesktopSection {
        platform,
        layers: Vec::new(),
        dead_keys: Vec::new(),
        space: Vec::new(),
        config: Vec::new(),
    };
    for (name, v) in entries {
        let at = join(path, name);
        match name {
            "primary" => {
                section.layers = platform_layers(v, &at, platform.layer_names(), path, defects)?;
            }
            "deadKeys" => section.dead_keys = dead_lists(v, &at)?,
            "space" if platform == Desktop::MacOs => {
                section.space = strings(v, &at)?
                    .into_iter()
                    .map(|(k, t)| {
                        let p = join(&at, &k);
                        (k, t, p)
                    })
                    .collect();
            }
            "config" => section.config = config(v, &at, platform.config_fields(), defects)?,
            _ => {}
        }
    }
    Ok(section)
}

fn touch(platform: Touch, value: &Value, defects: &mut Defects) -> Result<TouchSection, ReadError> {
    let path = platform.name();
    let entries = mapping(value, path)?;
    let mut known: Vec<&str> = platform.platforms().iter().map(|(p, _)| *p).collect();
    known.push("config");
    if platform == Touch::Ios {
        known.push("deadKeys");
    }
    unread(&entries, &known, path, defects);
    let mut section = TouchSection {
        platform,
        platforms: Vec::new(),
        dead_keys: Vec::new(),
        config: Vec::new(),
    };
    for (name, size) in platform.platforms() {
        if let Some((_, v)) = entries.iter().find(|(k, _)| k == name) {
            section.platforms.push(TouchPlatform {
                name: name.to_string(),
                size,
                layers: platform_layers(
                    v,
                    &join(path, name),
                    platform.layer_names(),
                    path,
                    defects,
                )?,
            });
        }
    }
    for (name, v) in &entries {
        let at = join(path, name);
        match *name {
            "deadKeys" => section.dead_keys = dead_lists(v, &at)?,
            "config" => {
                section.config = config(v, &at, &["spellerPackageKey", "spellerPath"], defects)?;
            }
            _ => {}
        }
    }
    Ok(section)
}

fn node(value: &Value, path: &str) -> Result<Node3, ReadError> {
    match value {
        Value::String(s) => Ok(Node3::Leaf(s.clone())),
        Value::Mapping(_) => Ok(Node3::Branch(
            mapping(value, path)?
                .into_iter()
                .map(|(k, v)| Ok((k.to_string(), node(v, &join(path, k))?)))
                .collect::<Result<_, ReadError>>()?,
        )),
        other => Err(format!(
            "{path}: a transform is a string or a mapping, not {}",
            describe(other)
        )),
    }
}

const TOP_LEVEL: [&str; 11] = [
    "displayNames",
    "decimal",
    "windows",
    "chromeOS",
    "macOS",
    "iOS",
    "android",
    "transforms",
    "keyNames",
    "longpress",
    "languageTag",
];

/// Reads the top-level mapping of a v3 file. A file with no platform
/// section is a v2 layout (M07); it is read no further.
pub fn read(value: &Value, defects: &mut Defects) -> Result<Option<Source3>, ReadError> {
    let entries = mapping(value, "")?;
    let has = |name: &str| entries.iter().any(|(k, _)| *k == name);
    let platforms = Desktop::ALL
        .iter()
        .map(|d| d.name())
        .chain(Touch::ALL.iter().map(|t| t.name()));
    if !platforms.clone().any(has) {
        let fields: Vec<&str> = entries.iter().map(|(k, _)| *k).collect();
        defects.add(
            Code::M07,
            "",
            format!(
                "a v2 layout: no v3 platform section ({}), only {}; re-migrate it from v2 or delete it",
                platforms.collect::<Vec<_>>().join(", "),
                fields.join(", ")
            ),
        );
        return Ok(None);
    }
    unread(&entries, &TOP_LEVEL, "", defects);
    let field = |name: &str| entries.iter().find(|(k, _)| *k == name).map(|(_, v)| *v);
    let display_names = match field("displayNames") {
        Some(v) => strings(v, "displayNames")?,
        None => return Err("displayNames: the field is required".to_string()),
    };
    let mut source = Source3 {
        display_names,
        decimal: field("decimal").map(|v| text(v, "decimal")).transpose()?,
        key_names: Vec::new(),
        longpress: match field("longpress") {
            Some(v) => strings(v, "longpress")?,
            None => Vec::new(),
        },
        transforms: Vec::new(),
        desktop: Vec::new(),
        touch: Vec::new(),
    };
    if let Some(v) = field("keyNames") {
        let names = strings(v, "keyNames")?;
        for (name, _) in &names {
            if name != "space" && name != "return" {
                defects.add(
                    Code::M10,
                    &join("keyNames", name),
                    format!("{name} is not a v3 key name; v3 never read it"),
                );
            }
        }
        source.key_names = names
            .into_iter()
            .filter(|(n, _)| n == "space" || n == "return")
            .collect();
    }
    if let Some(v) = field("transforms") {
        for (k, t) in mapping(v, "transforms")? {
            source
                .transforms
                .push((k.to_string(), node(t, &join("transforms", k))?));
        }
    }
    for platform in Desktop::ALL {
        if let Some(v) = field(platform.name()) {
            source.desktop.push(desktop(platform, v, defects)?);
        }
    }
    for platform in Touch::ALL {
        if let Some(v) = field(platform.name()) {
            source.touch.push(touch(platform, v, defects)?);
        }
    }
    Ok(Some(source))
}

/// Whether the block scalar opened on `line` (`key: |`, `key: >-`) starts
/// here.
fn opens_block(line: &str) -> bool {
    let body = strip_comment(line).trim_end();
    let Some((head, last)) = body.rsplit_once(' ') else {
        return false;
    };
    let indicator = last.starts_with(['|', '>'])
        && last[1..]
            .chars()
            .all(|c| c == '-' || c == '+' || c.is_ascii_digit());
    indicator && (head.ends_with(':') || head.trim() == "-")
}

/// The part of `line` before a comment: a `#` at the start or after a
/// blank, outside quoted scalars. A quote opens a quoted scalar only where
/// a scalar starts: at the start of the line, after `: `, `- `, `[`, `{`
/// or `,`; inside a plain scalar it is text.
fn strip_comment(line: &str) -> &str {
    let mut quote: Option<char> = None;
    let mut scalar_start = true;
    let mut previous = ' ';
    for (i, c) in line.char_indices() {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => {}
            None if c == '#' && previous.is_whitespace() => return &line[..i],
            None if (c == '\'' || c == '"') && scalar_start => quote = Some(c),
            None if c.is_whitespace() => {
                if previous == ':' || previous == '-' {
                    scalar_start = true;
                }
            }
            None => scalar_start = matches!(c, '[' | '{' | ','),
        }
        previous = c;
    }
    line
}

/// The 1-based numbers of the lines of `text` that hold a comment, block
/// scalar contents excluded (`ldml.migrate.defects` M11).
pub fn comment_lines(text: &str) -> Vec<usize> {
    let mut out = Vec::new();
    let mut block: Option<usize> = None;
    for (n, line) in text.lines().enumerate() {
        let indent = line.len() - line.trim_start().len();
        if let Some(parent) = block {
            if line.trim().is_empty() || indent > parent {
                continue;
            }
            block = None;
        }
        if strip_comment(line).len() != line.len() {
            out.push(n + 1);
        }
        if opens_block(line) {
            block = Some(indent);
        }
    }
    out
}
