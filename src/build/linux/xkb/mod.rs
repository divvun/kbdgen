//! The XKB tier of the Linux target (`docs/spec/linux.md`, "XKB layout";
//! `docs/spec/ldml/linux.md`): each layout's symbols file and registry
//! entry, derived from its `linux` keyboard, and the bundle's
//! `rules/evdev.xml`, built in memory for the output step to write.

mod classify;
pub mod compose;
mod dead_keys;
pub mod keycodes;
pub mod keysym;
mod levels;
mod registry;
mod symbols;
#[cfg(test)]
mod tests;
pub mod types;

use anyhow::{Result, anyhow};

pub use classify::{Classification, Rewrites, rewrites};
pub use dead_keys::{DeadKeys, Difference};
pub use levels::{Decimal, KeyValues, Levels, Value};
pub use registry::{RegistryEntry, evdev_xml};
pub use symbols::{Symbols, SymbolsKey};
pub use types::{KeyType, Typed, XkbState};

use super::ids::Ids;
use super::source::LinuxLayout;
use crate::build::diag::{Diagnostics, Platform};

/// One layout's XKB layout: what it was derived from, and its files.
#[derive(Debug, Clone)]
pub struct Adapted {
    pub entry: RegistryEntry,
    pub levels: Levels,
    /// Indexed like `levels.keys`.
    pub typed: Vec<Typed>,
    pub decimal: Option<Decimal>,
    pub dead: DeadKeys,
    pub symbols: Symbols,
    /// The text of `symbols/<L>`.
    pub text: String,
    pub classification: Classification,
    pub diag: Diagnostics,
}

/// The XKB layout of one compiled layout. It fails, naming the layout,
/// when the layout lacks what the registry needs, or the marker of a dead
/// key that does not leave it pending in the engine.
// [spec:kbdgen:def:ldml.linux.adapter]
pub fn adapt(ids: &Ids, layout: &LinuxLayout) -> Result<Adapted> {
    let entry = RegistryEntry::new(ids, layout)?;
    let diag = Diagnostics::new(Platform::Linux, layout.tag.to_string());
    let keyboard = &layout.keyboard;
    let hardware = keyboard
        .hardware
        .as_ref()
        .ok_or_else(|| anyhow!("{}: the linux keyboard has no hardware layers", layout.tag))?;
    let levels = levels::derive(keyboard, hardware);
    let dead = dead_keys::derive(keyboard, &levels, layout.tag.as_str())?;
    let typed: Vec<Typed> = levels
        .keys
        .iter()
        .map(|key| {
            let values = key
                .values
                .clone()
                .map(|value| levels::keysym(keyboard, &dead.keysyms, &value));
            types::choose(&values, levels.uses_level3)
        })
        .collect();
    let decimal = levels::decimal(keyboard);
    let symbols = Symbols {
        id: entry.name.clone(),
        file: layout.file.clone(),
        description: entry.description.clone(),
        keys: levels
            .keys
            .iter()
            .zip(&typed)
            .map(|(key, typed)| SymbolsKey {
                key: key.key,
                key_type: typed.key_type,
                levels: typed.levels.clone(),
            })
            .collect(),
        decimal: decimal.as_ref().map(|decimal| decimal.keysym),
        uses_level3: levels.uses_level3,
    };
    let text = symbols::write(&symbols);
    let mut adapted = Adapted {
        entry,
        levels,
        typed,
        decimal,
        dead,
        symbols,
        text,
        classification: Classification::default(),
        diag,
    };
    adapted.classification = classify::classify(&adapted, keyboard, hardware);
    classify::report(&adapted.classification, &mut adapted.diag);
    Ok(adapted)
}

/// The bundle's `xkb/` tree: file paths relative to it, and contents.
#[derive(Debug, Clone)]
pub struct Tree {
    pub layouts: Vec<Adapted>,
    pub files: Vec<(String, String)>,
}

impl Tree {
    /// The tree of `layouts`, in bundle layout order: `symbols/<L>` for
    /// each, then `rules/evdev.xml`.
    // [spec:kbdgen:syn:linux.xkb.registry]
    // [spec:kbdgen:syn:linux.xkb.symbols]
    pub fn new(ids: &Ids, layouts: &[LinuxLayout]) -> Result<Tree> {
        let adapted = layouts
            .iter()
            .map(|layout| adapt(ids, layout))
            .collect::<Result<Vec<_>>>()?;
        let mut files: Vec<(String, String)> = adapted
            .iter()
            .map(|layout| {
                (
                    format!("symbols/{}", layout.entry.name),
                    layout.text.clone(),
                )
            })
            .collect();
        let entries: Vec<RegistryEntry> =
            adapted.iter().map(|layout| layout.entry.clone()).collect();
        files.push(("rules/evdev.xml".to_owned(), evdev_xml(&entries)));
        Ok(Tree {
            layouts: adapted,
            files,
        })
    }
}
