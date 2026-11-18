//! The bundle's GNOME Shell extension (`linux.osk.gnome`): `metadata.json`,
//! `extension.js`, and `osk-layouts.gresource` holding each layout's
//! presented set in GNOME Shell's format (`ldml.linux.touch.gnome`).

use std::collections::BTreeMap;

use anyhow::{Context, Result};
use gvdb::write::{FileWriter, HashTableBuilder};
use kbd_model::MarkerIndex;
use serde::{Serialize, Serializer};

use super::display_name;
use super::presented::{Item, Level, LevelKind, Presented};
use crate::build::linux::ids::Ids;
use crate::build::linux::source::LinuxLayout;
use crate::build::linux::xkb::{self, keysym::Keysym};
use crate::bundle::project::LocaleProjectDescription;

#[cfg(test)]
mod tests;

/// The extension's files, relative to its directory.
pub const METADATA_FILE: &str = "metadata.json";
pub const EXTENSION_FILE: &str = "extension.js";
pub const BUNDLE_FILE: &str = "osk-layouts.gresource";

/// Where GNOME Shell looks up an on-screen layout `<id>.json` in the
/// registered resource bundles (`js/ui/keyboard.js` `KeyboardModel`).
pub const RESOURCE_PREFIX: &str = "/org/gnome/shell/osk-layouts/";

/// `extension.js`, the same for every bundle.
pub const EXTENSION_JS: &str = include_str!("gnome/extension.js");

/// The GNOME Shell major versions the extension declares; the shell
/// refuses it under any other.
const SHELL_VERSIONS: [&str; 2] = ["50", "51"];
/// The lock screen is `unlock-dialog`, so its keyboard gets the layouts.
const SESSION_MODES: [&str; 2] = ["user", "unlock-dialog"];

/// The keyval of Return, which the bottom row's return key presses.
const RETURN_KEYVAL: &str = "0xff0d";

/// Widths in half keys, the grid GNOME Shell places keys on (`KEY_SIZE`).
const ONE_KEY: u32 = 2;
const ADDED_KEY: u32 = 3;
/// The narrowest space bar.
const MIN_SPACE: u32 = 6;
/// The bottom row's keys but the space bar: the level switch and return,
/// 1.5 keys each, and the emoji, language and hide keys.
const BOTTOM_KEYS: u32 = ADDED_KEY + 3 * ONE_KEY + ADDED_KEY;

/// One layout's file in the bundle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    /// The layout id L, which names the file (`linux.ids`).
    pub id: String,
    /// The JSON, without insignificant whitespace.
    pub json: String,
}

impl Layout {
    /// The file's path in the bundle.
    // [spec:kbdgen:sem:linux.osk.gnome.selection]
    pub fn resource_path(&self) -> String {
        format!("{RESOURCE_PREFIX}{}.json", self.id)
    }
}

/// The bundle's extension, in memory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Extension {
    /// The UUID, which names its directory (`linux.ids`).
    pub uuid: String,
    /// The contents of `METADATA_FILE`.
    pub metadata: String,
    /// The bundle's layouts, in bundle layout order.
    pub layouts: Vec<Layout>,
    /// The contents of `BUNDLE_FILE`.
    pub bundle: Vec<u8>,
}

impl Extension {
    /// Each file of the extension, relative to its directory, and its
    /// contents.
    pub fn files(&self) -> [(&'static str, &[u8]); 3] {
        [
            (METADATA_FILE, self.metadata.as_bytes()),
            (EXTENSION_FILE, EXTENSION_JS.as_bytes()),
            (BUNDLE_FILE, &self.bundle),
        ]
    }
}

/// The dead keys that GNOME Shell's layout sends as keyvals, with their
/// dead keysyms. mutter presses a keyval at the lowest level of the first
/// key code that has it, holding Shift for Level2 and Level3 Shift for
/// Level3; at a higher level it types Level1. So a dead keysym D is sent
/// only when it is at Level1, 2 or 3 of the first key carrying D.
// [spec:kbdgen:sem:ldml.linux.touch.gnome]
pub fn keyvals(adapted: &xkb::Adapted) -> BTreeMap<MarkerIndex, Keysym> {
    let mut keys: Vec<&xkb::SymbolsKey> = adapted.symbols.keys.iter().collect();
    keys.sort_by_key(|key| key.key.keycode());
    adapted
        .dead
        .keysyms
        .iter()
        .filter(|(_, keysym)| {
            keys.iter()
                .find_map(|key| key.levels.iter().position(|level| level == *keysym))
                .is_some_and(|level| level < 3)
        })
        .map(|(marker, keysym)| (*marker, *keysym))
        .collect()
}

/// A length in half keys, written as a number of keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Halves(u32);

impl Halves {
    fn is_zero(&self) -> bool {
        self.0 == 0
    }

    fn is_one_key(&self) -> bool {
        self.0 == ONE_KEY
    }
}

impl Serialize for Halves {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.0 % 2 {
            0 => serializer.serialize_u32(self.0 / 2),
            _ => serializer.serialize_f64(f64::from(self.0) / 2.0),
        }
    }
}

/// `thousandths` of a key in half keys, to the nearest.
fn halves(thousandths: u32) -> u32 {
    (thousandths + 250) / 500
}

/// A key of GNOME Shell's format, one key wide unless set otherwise. The
/// fields are in the order GNOME Shell's own layouts write them.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct Key {
    #[serde(skip_serializing_if = "Option::is_none")]
    action: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    icon_name: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    keyval: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    label: Option<String>,
    #[serde(skip_serializing_if = "Halves::is_zero")]
    left_offset: Halves,
    #[serde(skip_serializing_if = "Option::is_none")]
    level: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    strings: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Halves::is_one_key")]
    width: Halves,
}

impl Default for Key {
    fn default() -> Self {
        Key {
            action: None,
            icon_name: None,
            keyval: None,
            label: None,
            left_offset: Halves(0),
            level: None,
            strings: None,
            width: Halves(ONE_KEY),
        }
    }
}

impl Key {
    fn spans(&self) -> u32 {
        self.left_offset.0 + self.width.0
    }
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

/// The name a level has in GNOME Shell's format.
fn level_name(kind: LevelKind) -> &'static str {
    match kind {
        LevelKind::Default => "default",
        LevelKind::Shift => "shift",
        LevelKind::Symbols => "opt",
        LevelKind::MoreSymbols => "opt+shift",
    }
}

/// A level's `mode`, which Stevia's format shares: the shift level is left
/// after a key, the symbol levels are not.
// [spec:kbdgen:sem:ldml.linux.touch.gnome]
pub(super) fn mode(kind: LevelKind) -> &'static str {
    match kind {
        LevelKind::Default => "default",
        LevelKind::Shift => "latched",
        LevelKind::Symbols | LevelKind::MoreSymbols => "locked",
    }
}

fn level_switch(level: &'static str) -> Key {
    Key {
        action: Some("levelSwitch"),
        level: Some(level),
        width: Halves(ADDED_KEY),
        ..Key::default()
    }
}

fn strings(first: &str, candidates: &[String]) -> Option<Vec<String>> {
    Some(
        std::iter::once(first.to_owned())
            .chain(candidates.iter().cloned())
            .collect(),
    )
}

/// The key of `item` on a level of `kind`; none for a gap or a dead key
/// left out.
// [spec:kbdgen:sem:ldml.linux.touch.gnome]
fn key(item: &Item, kind: LevelKind, keyvals: &BTreeMap<MarkerIndex, Keysym>) -> Option<Key> {
    let width = |thousandths: u32| Halves(halves(thousandths).max(1));
    Some(match item {
        Item::Gap(_) => return None,
        Item::Text(text) => Key {
            label: text.label.clone(),
            strings: strings(&text.output, &text.candidates),
            width: width(text.width),
            ..Key::default()
        },
        Item::Dead(dead) => match keyvals.get(&dead.marker) {
            Some(keysym) => Key {
                keyval: Some(format!("{:#x}", keysym.0)),
                label: Some(dead.display.clone()),
                width: width(dead.width),
                ..Key::default()
            },
            None if dead.standalone.is_empty() => return None,
            None => Key {
                label: Some(dead.display.clone()),
                strings: strings(&dead.standalone, &dead.candidates),
                width: width(dead.width),
                ..Key::default()
            },
        },
        Item::Shift => Key {
            icon_name: Some("osk-shift-symbolic"),
            ..level_switch(match kind {
                LevelKind::Shift => "default",
                _ => "shift",
            })
        },
        Item::Backspace => Key {
            action: Some("delete"),
            icon_name: Some("osk-delete-symbolic"),
            width: Halves(ADDED_KEY),
            ..Key::default()
        },
        Item::SymbolSwitch => match kind {
            LevelKind::Symbols => Key {
                label: Some("=/<".to_owned()),
                ..level_switch("opt+shift")
            },
            _ => Key {
                label: Some("?123".to_owned()),
                ..level_switch("opt")
            },
        },
    })
}

/// A row's keys, each gap and dead key left out adding nothing, a gap
/// adding its width to the next key's `leftOffset`.
// [spec:kbdgen:sem:ldml.linux.touch.gnome]
fn row(items: &[Item], kind: LevelKind, keyvals: &BTreeMap<MarkerIndex, Keysym>) -> Vec<Key> {
    let mut keys = Vec::new();
    let mut gap = 0;
    for item in items {
        if let Item::Gap(width) = item {
            gap += width;
            continue;
        }
        if let Some(mut key) = key(item, kind, keyvals) {
            key.left_offset = Halves(halves(gap));
            gap = 0;
            keys.push(key);
        }
    }
    keys
}

/// The bottom row, its space bar `space` half keys wide.
// [spec:kbdgen:sem:ldml.linux.touch.gnome]
fn bottom_row(kind: LevelKind, space: u32, space_label: Option<&str>) -> Vec<Key> {
    let switch = match kind {
        LevelKind::Default | LevelKind::Shift => Key {
            label: Some("?123".to_owned()),
            ..level_switch("opt")
        },
        LevelKind::Symbols | LevelKind::MoreSymbols => Key {
            label: Some("ABC".to_owned()),
            ..level_switch("default")
        },
    };
    let icon = |action, icon| Key {
        action: Some(action),
        icon_name: Some(icon),
        ..Key::default()
    };
    vec![
        switch,
        icon("emoji", "osk-emoji-picker-symbolic"),
        Key {
            label: space_label.map(str::to_owned),
            strings: Some(vec![" ".to_owned()]),
            width: Halves(space),
            ..Key::default()
        },
        icon("languageMenu", "osk-layout-symbolic"),
        icon("hide", "osk-hide-symbolic"),
        Key {
            icon_name: Some("osk-enter-symbolic"),
            keyval: Some(RETURN_KEYVAL.to_owned()),
            width: Halves(ADDED_KEY),
            ..Key::default()
        },
    ]
}

/// A level in GNOME Shell's format. The shell aligns rows left, so each
/// row narrower than the widest, the bottom row with a 3-key space bar
/// included, starts half the difference in, and the space bar fills the
/// bottom row.
// [spec:kbdgen:sem:ldml.linux.touch.gnome]
fn level(
    level: &Level,
    keyvals: &BTreeMap<MarkerIndex, Keysym>,
    space_label: Option<&str>,
) -> LevelDoc {
    let mut rows: Vec<Vec<Key>> = level
        .rows
        .iter()
        .map(|items| row(items, level.kind, keyvals))
        .collect();
    let widest = rows
        .iter()
        .map(|keys| keys.iter().map(Key::spans).sum::<u32>())
        .max()
        .unwrap_or(0)
        .max(BOTTOM_KEYS + MIN_SPACE);
    for keys in &mut rows {
        let width: u32 = keys.iter().map(Key::spans).sum();
        if let Some(first) = keys.first_mut() {
            first.left_offset.0 += (widest - width) / 2;
        }
    }
    rows.push(bottom_row(level.kind, widest - BOTTOM_KEYS, space_label));
    LevelDoc {
        level: level_name(level.kind),
        mode: mode(level.kind),
        rows,
    }
}

/// The file of `layout`, whose layout id is `id`, in GNOME Shell's format:
/// its presented set `presented`, with the dead keys of `keyvals` sent as
/// their dead keysyms.
// [spec:kbdgen:sem:ldml.linux.touch.gnome]
pub fn layout(
    id: &str,
    layout: &LinuxLayout,
    presented: &Presented,
    keyvals: &BTreeMap<MarkerIndex, Keysym>,
) -> Result<Layout> {
    let space = presented.space_label.as_deref();
    let document = Document {
        levels: presented
            .levels()
            .map(|shown| level(shown, keyvals, space))
            .collect(),
        locale: layout.tag.as_str(),
        name: display_name(layout)?,
    };
    let json = serde_json::to_string(&document)
        .with_context(|| format!("{}: serialise the GNOME Shell layout", layout.tag))?;
    Ok(Layout {
        id: id.to_owned(),
        json,
    })
}

#[derive(Serialize)]
#[serde(rename_all = "kebab-case")]
struct Metadata<'a> {
    uuid: &'a str,
    name: &'a str,
    description: &'a str,
    shell_version: [&'a str; 2],
    session_modes: [&'a str; 2],
    version_name: &'a str,
}

/// `metadata.json`, pretty JSON.
// [spec:kbdgen:def:linux.osk.gnome]
fn metadata(uuid: &str, version: &str, locale: &LocaleProjectDescription) -> Result<String> {
    let metadata = Metadata {
        uuid,
        name: &locale.name,
        description: &locale.description,
        shell_version: SHELL_VERSIONS,
        session_modes: SESSION_MODES,
        version_name: version,
    };
    let mut text = serde_json::to_string_pretty(&metadata)
        .with_context(|| format!("{uuid}: serialise {METADATA_FILE}"))?;
    text.push('\n');
    Ok(text)
}

/// `osk-layouts.gresource`: GLib's GVDB format, as
/// `glib-compile-resources` writes it. The hash table is keyed by path,
/// with an entry for each directory listing its children, and each file's
/// value is the variant `(uuay)`: its size, its flags, 0 for uncompressed,
/// and its data with a terminating zero.
// [spec:kbdgen:def:linux.osk.gnome]
fn bundle(layouts: &[Layout]) -> Result<Vec<u8>> {
    let mut table = HashTableBuilder::new();
    for layout in layouts {
        let path = layout.resource_path();
        let size = u32::try_from(layout.json.len())
            .with_context(|| format!("{BUNDLE_FILE}: {path} is too large"))?;
        let mut data = layout.json.as_bytes().to_vec();
        data.push(0);
        table
            .insert(&path, (size, 0u32, data))
            .with_context(|| format!("{BUNDLE_FILE}: add {path}"))?;
    }
    FileWriter::new()
        .write_to_vec_with_table(table)
        .with_context(|| format!("write {BUNDLE_FILE}"))
}

/// The bundle's extension, holding `layouts`, at `version`
/// (`targets/linux.yaml`), named and described by `locale`, the project's
/// `en` locale.
// [spec:kbdgen:def:linux.osk.gnome]
pub fn extension(
    ids: &Ids,
    version: &str,
    locale: &LocaleProjectDescription,
    layouts: Vec<Layout>,
) -> Result<Extension> {
    let uuid = ids.extension_uuid();
    Ok(Extension {
        metadata: metadata(&uuid, version, locale)?,
        bundle: bundle(&layouts)?,
        layouts,
        uuid,
    })
}
