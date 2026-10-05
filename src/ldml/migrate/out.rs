//! The v4 layout the migrator writes, and its text (`ldml.migrate.output`):
//! fresh YAML with top-level fields in `ldml.yaml.schema` order, variants in
//! host order, rows as literal blocks of tokens joined by single spaces,
//! and scalars quoted only where YAML needs it.

use serde_yaml::Value;

use super::text::printable;

/// A `deadKeys` node: the standalone output when it is not the identity,
/// and the compose entries in order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeadOut {
    pub standalone: Option<String>,
    pub compose: Vec<(String, ComposeOut)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComposeOut {
    Output(String),
    Node(DeadOut),
}

/// A hardware variant: layers by modifier-set key, and `space` entries.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HardwareOut {
    pub name: String,
    pub layers: Vec<(String, Vec<Vec<String>>)>,
    pub space: Vec<(String, String)>,
}

/// A touch layer: its rows, and the rows of its south flicks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TouchLayerOut {
    pub id: String,
    pub rows: Vec<Vec<String>>,
    pub flicks: Option<Vec<Vec<String>>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SizeOut {
    pub name: String,
    pub layers: Vec<TouchLayerOut>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TouchOut {
    pub name: String,
    pub sizes: Vec<SizeOut>,
}

/// A migrated layout. Every string is already in its v4 spelling.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Out4 {
    pub display_names: Vec<(String, String)>,
    pub decimal: Option<String>,
    pub key_names: Vec<(String, String)>,
    pub dead_keys: Vec<(String, DeadOut)>,
    pub long_press: Vec<(String, String)>,
    pub hardware: Vec<HardwareOut>,
    pub touch: Vec<TouchOut>,
    pub targets: Vec<(String, Vec<(String, String)>)>,
}

/// Words that YAML 1.1 readers take for booleans or null, quoted although
/// YAML 1.2 reads them as strings.
const YAML11_WORDS: [&str; 11] = [
    "y", "n", "yes", "no", "on", "off", "true", "false", "null", "~", "",
];

const INDICATORS: &str = "-?:,[]{}#&*!|>'\"%@`";

fn reads_back(document: &str, key: &str, value: &str) -> bool {
    let Ok(Value::Mapping(map)) = serde_yaml::from_str::<Value>(document) else {
        return false;
    };
    map.len() == 1
        && map
            .iter()
            .next()
            .is_some_and(|(k, v)| k.as_str() == Some(key) && v.as_str() == Some(value))
}

/// Whether `s` may be written as a plain scalar, as a key and as a value.
fn plain_ok(s: &str) -> bool {
    let Some(first) = s.chars().next() else {
        return false;
    };
    !INDICATORS.contains(first)
        && s.chars().all(|c| printable(c) && c != '\t')
        && s.trim() == s
        && !s.contains(": ")
        && !s.contains(" #")
        && !s.ends_with(':')
        && !YAML11_WORDS.contains(&s.to_ascii_lowercase().as_str())
        && reads_back(&format!("k: {s}\n"), "k", s)
        && reads_back(&format!("{s}: v\n"), s, "v")
}

fn double_quoted(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if printable(c) => out.push(c),
            c if u32::from(c) <= 0xFFFF => out.push_str(&format!("\\u{:04X}", u32::from(c))),
            c => out.push_str(&format!("\\U{:08X}", u32::from(c))),
        }
    }
    out.push('"');
    out
}

/// `s` as a YAML scalar: plain where YAML allows, else single-quoted, else
/// double-quoted with escapes for what YAML cannot hold as written.
pub fn scalar(s: &str) -> String {
    if plain_ok(s) {
        s.to_string()
    } else if s.chars().all(printable) {
        format!("'{}'", s.replace('\'', "''"))
    } else {
        double_quoted(s)
    }
}

/// Lines of YAML text at two spaces per level.
#[derive(Default)]
struct Emitter {
    text: String,
}

impl Emitter {
    fn line(&mut self, level: usize, text: &str) {
        for _ in 0..level {
            self.text.push_str("  ");
        }
        self.text.push_str(text);
        self.text.push('\n');
    }

    fn entry(&mut self, level: usize, key: &str, value: &str) {
        self.line(level, &format!("{}: {}", scalar(key), scalar(value)));
    }

    fn open(&mut self, level: usize, key: &str) {
        self.line(level, &format!("{}:", scalar(key)));
    }

    fn entries(&mut self, level: usize, key: &str, values: &[(String, String)]) {
        if values.is_empty() {
            return;
        }
        self.open(level, key);
        for (k, v) in values {
            self.entry(level + 1, k, v);
        }
    }

    /// Rows as a literal block, one row per line.
    fn rows(&mut self, level: usize, key: &str, rows: &[Vec<String>]) {
        self.line(level, &format!("{}: |", scalar(key)));
        for row in rows {
            self.line(level + 1, &row.join(" "));
        }
    }

    fn dead(&mut self, level: usize, key: &str, node: &DeadOut) {
        if node.standalone.is_none() && node.compose.is_empty() {
            self.line(level, &format!("{}: {{}}", scalar(key)));
            return;
        }
        self.open(level, key);
        if let Some(s) = &node.standalone {
            self.entry(level + 1, "standalone", s);
        }
        if node.compose.is_empty() {
            return;
        }
        self.open(level + 1, "compose");
        for (input, value) in &node.compose {
            match value {
                ComposeOut::Output(o) => self.entry(level + 2, input, o),
                ComposeOut::Node(child) => self.dead(level + 2, input, child),
            }
        }
    }
}

// [spec:kbdgen:req:ldml.migrate.output]
/// The text of a migrated layout. Equal layouts give equal bytes.
pub fn write(layout: &Out4) -> String {
    let mut e = Emitter::default();
    e.line(0, "format: 4");
    e.entries(0, "displayNames", &layout.display_names);
    if let Some(decimal) = &layout.decimal {
        e.entry(0, "decimal", decimal);
    }
    e.entries(0, "keyNames", &layout.key_names);
    if !layout.dead_keys.is_empty() {
        e.open(0, "deadKeys");
        for (identity, node) in &layout.dead_keys {
            e.dead(1, identity, node);
        }
    }
    e.entries(0, "longPress", &layout.long_press);
    if !layout.hardware.is_empty() {
        e.open(0, "hardware");
        for variant in &layout.hardware {
            e.open(1, &variant.name);
            e.open(2, "layers");
            for (key, rows) in &variant.layers {
                e.rows(3, key, rows);
            }
            e.entries(2, "space", &variant.space);
        }
    }
    if !layout.touch.is_empty() {
        e.open(0, "touch");
        for variant in &layout.touch {
            e.open(1, &variant.name);
            e.open(2, "sizes");
            for size in &variant.sizes {
                e.open(3, &size.name);
                e.open(4, "layers");
                for layer in &size.layers {
                    match &layer.flicks {
                        None => e.rows(5, &layer.id, &layer.rows),
                        Some(flicks) => {
                            e.open(5, &layer.id);
                            e.rows(6, "rows", &layer.rows);
                            e.open(6, "flicks");
                            e.rows(7, "s", flicks);
                        }
                    }
                }
            }
        }
    }
    if !layout.targets.is_empty() {
        e.open(0, "targets");
        for (host, fields) in &layout.targets {
            e.entries(1, host, fields);
        }
    }
    e.text
}
