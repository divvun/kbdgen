//! The IBus engine's data for each layout (`linux.ibus.data`): its engine
//! description `<L>.xml` and its model `<L>.dvkb`, built in memory for the
//! output step to write under `ibus/`. The engine program itself is the
//! workspace crate `crates/kbd-ibus`, released separately; its `--xml`
//! lister reads these descriptions back.

#[cfg(test)]
mod tests;

use std::fmt::Write as _;

use anyhow::Result;
use language_tags::LanguageTag;

use super::ids::Ids;
use super::source::LinuxLayout;
use super::xkb::RegistryEntry;

/// One layout's `<engine>`: what IBus lists the engine by, and the XKB
/// layout GNOME Shell applies beneath it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineDescription {
    /// The layout id L, which also names the files.
    pub name: String,
    /// The registry entry's description, which is also the `description`.
    pub longname: String,
    pub language: String,
    pub author: String,
    /// L, the layout's own XKB layout.
    pub layout: String,
    pub symbol: String,
    pub version: String,
}

/// The IBus name of a tag's language: its language subtag, with its region
/// subtag after `_` when it has one, as iso-codes names locales.
fn ibus_language(tag: &LanguageTag) -> String {
    let language = tag.primary_language().to_ascii_lowercase();
    match tag.region() {
        Some(region) => format!("{language}_{region}"),
        None => language,
    }
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

impl EngineDescription {
    // [spec:kbdgen:syn:linux.ibus.data]
    pub fn new(
        entry: &RegistryEntry,
        layout: &LinuxLayout,
        author: &str,
        version: &str,
    ) -> EngineDescription {
        EngineDescription {
            name: entry.name.clone(),
            longname: entry.description.clone(),
            language: ibus_language(&layout.tag),
            author: author.to_owned(),
            layout: entry.name.clone(),
            symbol: entry.short_description.clone(),
            version: version.to_owned(),
        }
    }

    /// The text of `<L>.xml`. There is no `icon`, `setup` or `rank`: IBus
    /// shows the symbol.
    // [spec:kbdgen:syn:linux.ibus.data]
    pub fn xml(&self) -> String {
        let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<engine>\n");
        let elements = [
            ("name", &self.name),
            ("longname", &self.longname),
            ("description", &self.longname),
            ("language", &self.language),
            ("author", &self.author),
            ("layout", &self.layout),
            ("symbol", &self.symbol),
            ("version", &self.version),
        ];
        for (element, text) in elements {
            let _ = writeln!(out, "  <{element}>{}</{element}>", escape(text));
        }
        out.push_str("</engine>\n");
        out
    }
}

/// The bundle's `ibus/` tree: file names, and contents.
#[derive(Debug, Clone)]
pub struct Tree {
    pub descriptions: Vec<EngineDescription>,
    pub files: Vec<(String, Vec<u8>)>,
}

impl Tree {
    /// The tree of `layouts`, in bundle layout order: `<L>.xml` then
    /// `<L>.dvkb` for each. `author` is `project.yaml`'s, `version`
    /// `targets/linux.yaml`'s.
    // [spec:kbdgen:syn:linux.ibus.data]
    pub fn new(ids: &Ids, author: &str, version: &str, layouts: &[LinuxLayout]) -> Result<Tree> {
        let mut descriptions = Vec::new();
        let mut files = Vec::new();
        for layout in layouts {
            let entry = RegistryEntry::new(ids, layout)?;
            let description = EngineDescription::new(&entry, layout, author, version);
            files.push((
                format!("{}.xml", description.name),
                description.xml().into_bytes(),
            ));
            files.push((format!("{}.dvkb", description.name), layout.model.clone()));
            descriptions.push(description);
        }
        Ok(Tree {
            descriptions,
            files,
        })
    }
}
