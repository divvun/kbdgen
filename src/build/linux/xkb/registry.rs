//! The registry entry of each layout and the bundle's `rules/evdev.xml`
//! (`linux.xkb.registry`).

use std::fmt::Write as _;

use anyhow::{Result, anyhow};

use crate::build::linux::ids::Ids;
use crate::build::linux::source::LinuxLayout;

/// A layout's `<configItem>`: what GNOME, KDE and Stevia list it by.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryEntry {
    /// The layout id L.
    pub name: String,
    /// The tag's primary language subtag, which GNOME shows as the
    /// indicator.
    pub short_description: String,
    /// The layout's display name under its primary language subtag.
    pub description: String,
    /// The tag's region subtag, when it is two letters.
    pub country: Option<String>,
    /// The ISO 639-3 code of the primary language subtag, when it has one.
    pub language: Option<String>,
}

impl RegistryEntry {
    // [spec:kbdgen:syn:linux.xkb.registry]
    pub fn new(ids: &Ids, layout: &LinuxLayout) -> Result<RegistryEntry> {
        let tag = &layout.tag;
        let primary = tag.primary_language().to_ascii_lowercase();
        let description = layout
            .display_names
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(&primary))
            .map(|(_, name)| name.clone())
            .ok_or_else(|| anyhow!("{tag}: no display name under {primary}"))?;
        let country = tag
            .region()
            .filter(|region| region.len() == 2 && region.bytes().all(|b| b.is_ascii_alphabetic()))
            .map(str::to_ascii_uppercase);
        let language = iso639::autonym::get(&primary).map(|record| record.tag3.to_owned());
        Ok(RegistryEntry {
            name: ids.layout_id(tag),
            short_description: primary,
            description,
            country,
            language,
        })
    }
}

/// `text` with `&`, `<` and `>` escaped, for XML element content.
pub(crate) fn escape_text(text: &str) -> String {
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

fn element(out: &mut String, depth: usize, name: &str, text: &str) {
    let _ = writeln!(
        out,
        "{:indent$}<{name}>{}</{name}>",
        "",
        escape_text(text),
        indent = depth * 2
    );
}

fn open(out: &mut String, depth: usize, name: &str) {
    let _ = writeln!(out, "{:indent$}<{name}>", "", indent = depth * 2);
}

fn close(out: &mut String, depth: usize, name: &str) {
    let _ = writeln!(out, "{:indent$}</{name}>", "", indent = depth * 2);
}

/// `rules/evdev.xml` listing `entries`, in their order. libxkbregistry
/// validates it against a DTD of its own, which needs no `modelList` or
/// `optionList`, so there is no doctype.
// [spec:kbdgen:syn:linux.xkb.registry]
pub fn evdev_xml(entries: &[RegistryEntry]) -> String {
    let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str("<xkbConfigRegistry version=\"1.1\">\n");
    open(&mut out, 1, "layoutList");
    for entry in entries {
        open(&mut out, 2, "layout");
        open(&mut out, 3, "configItem");
        element(&mut out, 4, "name", &entry.name);
        element(&mut out, 4, "shortDescription", &entry.short_description);
        element(&mut out, 4, "description", &entry.description);
        if let Some(country) = &entry.country {
            open(&mut out, 4, "countryList");
            element(&mut out, 5, "iso3166Id", country);
            close(&mut out, 4, "countryList");
        }
        if let Some(language) = &entry.language {
            open(&mut out, 4, "languageList");
            element(&mut out, 5, "iso639Id", language);
            close(&mut out, 4, "languageList");
        }
        close(&mut out, 3, "configItem");
        close(&mut out, 2, "layout");
    }
    close(&mut out, 1, "layoutList");
    out.push_str("</xkbConfigRegistry>\n");
    out
}
