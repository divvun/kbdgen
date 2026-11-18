//! A layout's Stevia layout (`linux.osk.stevia`): the presented set in
//! the format of Stevia's `src/layouts/*.json` (`ldml.linux.touch.stevia`),
//! for contributing upstream.

use anyhow::{Context, Result};
use kbd_model::DEFAULT_WIDTH;
use serde::Serialize;

use super::display_name;
use super::gnome::mode;
use super::presented::{Item, Level, LevelKind, Presented};
use crate::build::linux::source::LinuxLayout;

#[cfg(test)]
mod tests;

/// The range of Stevia's key `width` property (`src/pos-osk-key.c`), in
/// thousandths of a key. GLib warns about a width outside it and keeps 1.
const MIN_WIDTH: u32 = 1000;
const MAX_WIDTH: u32 = 10_000;

/// One layout's file, in memory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    /// The layout id L, which names the file (`linux.ids`).
    pub id: String,
    /// Pretty JSON, as Stevia's own layouts are written.
    pub json: String,
}

impl Layout {
    /// The file's path, relative to `stevia/`, and to Stevia's `src/`.
    pub fn path(&self) -> String {
        format!("layouts/{}.json", self.id)
    }
}

/// A key: its text then its candidates, or an object where it needs more.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
enum Key {
    Plain(Vec<String>),
    Object(KeyObject),
}

/// The properties of Stevia's `PosOskKey` a layout sets, in the order
/// Stevia's own layouts write them.
#[derive(Debug, Clone, PartialEq, Serialize)]
struct KeyObject {
    symbol: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    symbols: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    width: Option<f64>,
}

#[derive(Debug, Serialize)]
struct LevelDoc {
    level: &'static str,
    mode: &'static str,
    rows: Vec<Vec<Key>>,
}

#[derive(Debug, Serialize)]
struct Document<'a> {
    levels: Vec<LevelDoc>,
    locale: &'a str,
    name: &'a str,
}

/// The name a level has in Stevia's format; Stevia knows no others.
fn level_name(kind: LevelKind) -> &'static str {
    match kind {
        LevelKind::Default => "",
        LevelKind::Shift => "shift",
        LevelKind::Symbols => "opt",
        LevelKind::MoreSymbols => "opt+shift",
    }
}

/// A key committing `symbol`, with long-press `candidates`, an object
/// when it has a label or a width other than 1.
// [spec:kbdgen:sem:ldml.linux.touch.stevia]
fn key(symbol: &str, candidates: &[String], label: Option<&str>, width: u32) -> Key {
    let width = width.clamp(MIN_WIDTH, MAX_WIDTH);
    if label.is_none() && width == DEFAULT_WIDTH {
        return Key::Plain(
            std::iter::once(symbol.to_owned())
                .chain(candidates.iter().cloned())
                .collect(),
        );
    }
    Key::Object(KeyObject {
        symbol: symbol.to_owned(),
        symbols: candidates.to_vec(),
        label: label.map(str::to_owned),
        width: (width != DEFAULT_WIDTH).then(|| f64::from(width) / 1000.0),
    })
}

/// The key of `item`, none for what Stevia adds itself, a gap, which
/// Stevia's centred rows have no use for, and a dead key without a
/// standalone. Stevia has no dead keys, so a dead key commits its
/// standalone.
// [spec:kbdgen:sem:ldml.linux.touch.stevia]
fn item(item: &Item) -> Option<Key> {
    match item {
        Item::Text(text) => Some(key(
            &text.output,
            &text.candidates,
            text.label.as_deref(),
            text.width,
        )),
        Item::Dead(dead) if dead.standalone.is_empty() => None,
        Item::Dead(dead) => Some(key(
            &dead.standalone,
            &dead.candidates,
            Some(&dead.display),
            dead.width,
        )),
        Item::Gap(_) | Item::Shift | Item::Backspace | Item::SymbolSwitch => None,
    }
}

/// The last row of a level, around the space bar, as Stevia's own layouts
/// have it; Stevia adds its menu, symbol and return keys to it.
// [spec:kbdgen:sem:ldml.linux.touch.stevia]
fn bottom_row(kind: LevelKind) -> Vec<Key> {
    let symbols: &[&str] = match kind {
        LevelKind::Default | LevelKind::Shift => &[",", " ", "."],
        LevelKind::Symbols => &["_", "/", " ", ",", "."],
        LevelKind::MoreSymbols => &["<", ">", " ", ",", "."],
    };
    symbols
        .iter()
        .map(|symbol| Key::Plain(vec![(*symbol).to_owned()]))
        .collect()
}

/// A level in Stevia's format. A row left with no keys is left out, as
/// Stevia adds no keys to an empty row: it puts shift and backspace on a
/// level's second-to-last row.
// [spec:kbdgen:sem:ldml.linux.touch.stevia]
fn level(level: &Level) -> LevelDoc {
    let mut rows: Vec<Vec<Key>> = level
        .rows
        .iter()
        .map(|items| items.iter().filter_map(item).collect::<Vec<_>>())
        .filter(|row| !row.is_empty())
        .collect();
    rows.push(bottom_row(level.kind));
    LevelDoc {
        level: level_name(level.kind),
        mode: mode(level.kind),
        rows,
    }
}

/// The Stevia layout of `layout`, whose layout id is `id`, holding its
/// presented set `presented`.
// [spec:kbdgen:def:linux.osk.stevia]
// [spec:kbdgen:sem:ldml.linux.touch.stevia]
pub fn layout(id: &str, layout: &LinuxLayout, presented: &Presented) -> Result<Layout> {
    let document = Document {
        levels: presented.levels().map(level).collect(),
        locale: layout.tag.as_str(),
        name: display_name(layout)?,
    };
    let mut json = serde_json::to_string_pretty(&document)
        .with_context(|| format!("{}: serialise the Stevia layout", layout.tag))?;
    json.push('\n');
    Ok(Layout {
        id: id.to_owned(),
        json,
    })
}
