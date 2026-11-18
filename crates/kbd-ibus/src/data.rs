//! The keyboards' data (`linux.ibus.lister`): the engine descriptions and
//! models that keyboard packages install under the data directories, the
//! per-user one first, and the `<engines>` list that ibus-daemon reads from
//! `--xml`.
//!
//! A description is read strictly, as the shape kbdgen writes
//! (`linux.ibus.data`): an optional XML declaration, then an `<engine>`
//! whose children are elements holding text alone. Attributes, comments
//! and nested elements, which kbdgen never writes, make it unusable. Only
//! the elements of `linux.ibus.data` are listed again, so no description
//! can hand IBus a `setup` command or an `icon` path.

use std::collections::BTreeSet;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use kbd_engine::{Error, Model};
use kbd_model::DecodeError;

/// The children of an `<engine>` that `linux.ibus.data` writes, in order.
pub const ELEMENTS: [&str; 8] = [
    "name",
    "longname",
    "description",
    "language",
    "author",
    "layout",
    "symbol",
    "version",
];

/// The per-user data directory. IBus expands only a leading `~` in the
/// component's observed paths, so this is not under `XDG_DATA_HOME`.
// [spec:kbdgen:def:linux.ibus.component]
pub fn user_root() -> Option<PathBuf> {
    std::env::home_dir()
        .filter(|home| !home.as_os_str().is_empty())
        .map(|home| home.join(".local/share/divvun/keyboards"))
}

/// The data directories in the order descriptions are read: the user's,
/// then each `--data-dir` in argument order.
// [spec:kbdgen:req:linux.ibus.lister]
pub fn roots(data_dirs: &[PathBuf]) -> Vec<PathBuf> {
    user_root()
        .into_iter()
        .chain(data_dirs.iter().cloned())
        .collect()
}

/// An engine description: the text of each of [`ELEMENTS`], unescaped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Description {
    values: [String; 8],
}

/// What is left of a description's text while it is read.
struct Reader<'a> {
    rest: &'a str,
}

impl<'a> Reader<'a> {
    fn skip_space(&mut self) {
        self.rest = self.rest.trim_start_matches([' ', '\t', '\r', '\n']);
    }

    fn eat(&mut self, token: &str) -> bool {
        match self.rest.strip_prefix(token) {
            Some(rest) => {
                self.rest = rest;
                true
            }
            None => false,
        }
    }

    fn until(&mut self, token: &str) -> Option<&'a str> {
        let (before, after) = self.rest.split_once(token)?;
        self.rest = after;
        Some(before)
    }

    fn name(&mut self) -> Option<&'a str> {
        let end = self
            .rest
            .find(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.')))
            .unwrap_or(self.rest.len());
        let (name, rest) = self.rest.split_at_checked(end)?;
        if !name.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_') {
            return None;
        }
        self.rest = rest;
        Some(name)
    }
}

/// Element text with its references resolved; `None` for a `<` or a
/// reference XML does not define.
fn unescape(raw: &str) -> Option<String> {
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some((text, after)) = rest.split_once('&') {
        if text.contains('<') {
            return None;
        }
        out.push_str(text);
        let (reference, after) = after.split_once(';')?;
        let c = match reference {
            "amp" => '&',
            "lt" => '<',
            "gt" => '>',
            "quot" => '"',
            "apos" => '\'',
            _ => {
                let code = match reference.strip_prefix("#x") {
                    Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                    None => reference.strip_prefix('#')?.parse().ok()?,
                };
                char::from_u32(code).filter(|c| *c != '\0')?
            }
        };
        out.push(c);
        rest = after;
    }
    if rest.contains('<') {
        return None;
    }
    out.push_str(rest);
    Some(out)
}

/// `text` escaped for XML element content.
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(c),
        }
    }
    out
}

impl Description {
    /// Reads a description; `None` unless it has the shape kbdgen writes,
    /// each of [`ELEMENTS`] exactly once and a non-empty `name`. Other
    /// elements are skipped.
    // [spec:kbdgen:req:linux.ibus.lister]
    pub fn parse(text: &str) -> Option<Description> {
        let mut reader = Reader {
            rest: text.strip_prefix('\u{feff}').unwrap_or(text),
        };
        reader.skip_space();
        if reader.eat("<?xml") {
            reader.until("?>")?;
            reader.skip_space();
        }
        if !reader.eat("<engine>") {
            return None;
        }
        let mut values: [Option<String>; 8] = Default::default();
        loop {
            reader.skip_space();
            if reader.eat("</engine>") {
                break;
            }
            if !reader.eat("<") {
                return None;
            }
            let element = reader.name()?;
            let text = if reader.eat("/>") {
                String::new()
            } else {
                if !reader.eat(">") {
                    return None;
                }
                let text = unescape(reader.until("</")?)?;
                if reader.name()? != element || !reader.eat(">") {
                    return None;
                }
                text
            };
            if let Some(slot) = ELEMENTS
                .iter()
                .position(|known| *known == element)
                .and_then(|i| values.get_mut(i))
                && slot.replace(text).is_some()
            {
                return None;
            }
        }
        reader.skip_space();
        if !reader.rest.is_empty() {
            return None;
        }
        let [
            name,
            longname,
            description,
            language,
            author,
            layout,
            symbol,
            version,
        ] = values;
        let values = [
            name.filter(|name| !name.is_empty())?,
            longname?,
            description?,
            language?,
            author?,
            layout?,
            symbol?,
            version?,
        ];
        Some(Description { values })
    }

    pub fn name(&self) -> &str {
        let [name, ..] = &self.values;
        name
    }

    /// The `<engine>` element as the lister prints it: indented by two
    /// spaces, its children by four.
    // [spec:kbdgen:req:linux.ibus.lister]
    pub fn write(&self, out: &mut String) {
        out.push_str("  <engine>\n");
        for (element, text) in ELEMENTS.iter().zip(&self.values) {
            out.push_str(&format!("    <{element}>{}</{element}>\n", escape(text)));
        }
        out.push_str("  </engine>\n");
    }
}

/// A description whose `name` is its file's stem, and the path of the
/// model beside it, which may not exist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    pub description: Description,
    pub model: PathBuf,
}

/// The entries of `dir`, in byte order of their names; none when it
/// cannot be read.
fn sorted(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut entries: Vec<(OsString, PathBuf)> = entries
        .flatten()
        .map(|entry| (entry.file_name(), entry.path()))
        .collect();
    entries.sort();
    entries.into_iter().map(|(_, path)| path).collect()
}

/// Every description in `<root>/*/` for each of `roots`, in order, that
/// parses and is named by its file stem.
// [spec:kbdgen:req:linux.ibus.lister]
pub fn found(roots: &[PathBuf]) -> Vec<Found> {
    let mut found = Vec::new();
    for package in roots.iter().flat_map(|root| sorted(root)) {
        if !package.is_dir() {
            continue;
        }
        for file in sorted(&package) {
            if file.extension().is_none_or(|extension| extension != "xml") || !file.is_file() {
                continue;
            }
            let Some(stem) = file.file_stem().and_then(|stem| stem.to_str()) else {
                continue;
            };
            let Some(description) = fs::read_to_string(&file)
                .ok()
                .and_then(|text| Description::parse(&text))
            else {
                continue;
            };
            if description.name() == stem {
                let model = file.with_extension("dvkb");
                found.push(Found { description, model });
            }
        }
    }
    found
}

/// Why `CreateEngine` cannot serve an engine (`linux.ibus.inert`).
// [spec:kbdgen:req:linux.ibus.inert]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    /// No usable description has the name.
    NoDescription,
    ModelUnreadable,
    /// `Model::from_bytes` refused the model's major version.
    ModelVersion,
    /// The file is not an encoded model.
    ModelMalformed,
    /// The model violates an invariant or needs an engine feature this
    /// build lacks.
    ModelInvalid,
}

impl Reason {
    /// The reason as `CreateEngine`'s error message and the diagnostic
    /// give it.
    // [spec:kbdgen:req:linux.ibus.inert]
    pub const fn code(self) -> &'static str {
        match self {
            Reason::NoDescription => "no-description",
            Reason::ModelUnreadable => "model-unreadable",
            Reason::ModelVersion => "model-version",
            Reason::ModelMalformed => "model-malformed",
            Reason::ModelInvalid => "model-invalid",
        }
    }
}

/// Loads the model at `path`.
// [spec:kbdgen:req:linux.ibus.inert]
pub fn load(path: &Path) -> Result<Model, Reason> {
    let bytes = fs::read(path).map_err(|_| Reason::ModelUnreadable)?;
    Model::from_bytes(&bytes).map_err(|error| match error {
        Error::Decode(DecodeError::UnsupportedVersion(_)) => Reason::ModelVersion,
        Error::Decode(_) => Reason::ModelMalformed,
        Error::Invalid(_) | Error::NormalizationUnsupported => Reason::ModelInvalid,
    })
}

/// Why the engine `name` cannot be served, and the file that says so.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    pub reason: Reason,
    pub file: PathBuf,
}

/// The model of the engine `--xml` lists as `name`: that of the first
/// description of the name whose model loads. Without one, the refusal is
/// that of the first such description's model, or `no-description` for
/// `<name>.xml` when no description has the name, so `--ibus` serves
/// exactly the engines `--xml` lists.
// [spec:kbdgen:req:linux.ibus.lister]
// [spec:kbdgen:req:linux.ibus.inert]
pub fn lookup(roots: &[PathBuf], name: &str) -> Result<Model, Refusal> {
    let mut refusal = None;
    for found in found(roots) {
        if found.description.name() != name {
            continue;
        }
        match load(&found.model) {
            Ok(model) => return Ok(model),
            Err(reason) => {
                refusal.get_or_insert(Refusal {
                    reason,
                    file: found.model,
                });
            }
        }
    }
    Err(refusal.unwrap_or_else(|| Refusal {
        reason: Reason::NoDescription,
        file: PathBuf::from(format!("{name}.xml")),
    }))
}

/// The usable descriptions under `roots`, the first of each name.
// [spec:kbdgen:req:linux.ibus.lister]
pub fn usable(roots: &[PathBuf]) -> Vec<Found> {
    let mut names = BTreeSet::new();
    let mut usable = Vec::new();
    for found in found(roots) {
        if !names.contains(found.description.name()) && load(&found.model).is_ok() {
            names.insert(found.description.name().to_owned());
            usable.push(found);
        }
    }
    usable
}

/// What `--xml` prints: one `<engines>` holding the usable descriptions.
// [spec:kbdgen:req:linux.ibus.lister]
pub fn engines_xml(roots: &[PathBuf]) -> String {
    let mut out = String::from("<engines>\n");
    for found in usable(roots) {
        found.description.write(&mut out);
    }
    out.push_str("</engines>\n");
    out
}
