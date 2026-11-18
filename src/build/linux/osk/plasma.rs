//! A layout's plasma-keyboard package (`linux.osk.plasma`): a KPackage
//! that plasma-keyboard 6.9 and later reads from
//! `plasma/keyboard/keyboardpackages/<id>/`, with `metadata.json` and the
//! presented set as `contents/layouts/desktop.qml`
//! (`ldml.linux.touch.plasma`).

use std::fmt::Write as _;

use anyhow::{Context, Result};
use kbd_model::DEFAULT_WIDTH;
use serde_json::{Map, Value, json};

use super::display_name;
use super::presented::{ADDED_KEY_WIDTH, Item, Level, LevelKind, Presented, TextKey};
use crate::build::linux::ids::Ids;
use crate::build::linux::source::LinuxLayout;

#[cfg(test)]
mod tests;

/// The package's metadata, relative to its directory.
pub const METADATA_FILE: &str = "metadata.json";
/// The package's one layout, relative to its directory.
pub const LAYOUT_FILE: &str = "contents/layouts/desktop.qml";

/// The layout's file name in `contents/layouts/`, and its id, which
/// plasma-keyboard joins to the package id as `<id>/desktop`.
const LAYOUT_NAME: &str = "desktop.qml";
const LAYOUT_ID: &str = "desktop";

/// Every form factor plasma-keyboard's own packages list, so that its
/// settings offer the layout whatever the device (`kcm/plasmakeyboardkcm.cpp`).
const FORM_FACTORS: [&str; 3] = ["phone", "desktop", "mediacenter"];

/// The widths of nb_NO's bottom row, in thousandths of a key, but for its
/// space bar: `SymbolModeKey`, `ChangeLanguageKey`, `,`, `.`,
/// `HideKeyboardKey` and `EnterKey`.
const BOTTOM_KEYS_WIDTH: u32 = 1500 + 1000 + 1000 + 1000 + 1000 + 1500;
/// The narrowest space bar, nb_NO's.
const MIN_SPACE_WIDTH: u32 = 4000;

const INDENT: &str = "    ";

/// plasma-keyboard's `Keyboard.qml` caps the layout's width at
/// `item.maxWidthToHeightRatio * height` unless that is -1, and a loader
/// has no such property, so without it the width is NaN and no tap lands
/// on a key.
const LOADER_RATIO: &str = "readonly property real maxWidthToHeightRatio: -1";

/// One layout's package, in memory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Package {
    /// The package id, which names its directory (`linux.ids`).
    pub id: String,
    /// The contents of `METADATA_FILE`.
    pub metadata: String,
    /// The contents of `LAYOUT_FILE`.
    pub layout: String,
}

/// `tag` with `-` replaced by `_`, as KDE names locales.
fn kde_locale(tag: &str) -> String {
    tag.replace('-', "_")
}

/// `metadata.json`: pretty JSON with `KPlugin` and `X-Plasma-Keyboard`.
// [spec:kbdgen:def:linux.osk.plasma]
fn metadata(id: &str, version: &str, layout: &LinuxLayout) -> Result<String> {
    let name = display_name(layout)?;
    let primary = layout.tag.primary_language();
    let mut plugin = Map::new();
    plugin.insert("Id".to_owned(), json!(id));
    plugin.insert("Name".to_owned(), json!(name));
    for (tag, translated) in &layout.display_names {
        if !tag.eq_ignore_ascii_case(primary) {
            plugin.insert(format!("Name[{}]", kde_locale(tag)), json!(translated));
        }
    }
    plugin.insert("Version".to_owned(), json!(version));
    let metadata = json!({
        "KPlugin": Value::Object(plugin),
        "X-Plasma-Keyboard": {
            "KeyboardLayouts": [{
                "File": LAYOUT_NAME,
                "FormFactors": FORM_FACTORS,
                "Id": LAYOUT_ID,
                "Name": name,
            }],
            "Locales": [kde_locale(layout.tag.as_str())],
        },
    });
    let mut text = serde_json::to_string_pretty(&metadata)
        .with_context(|| format!("{}: serialise {METADATA_FILE}", layout.tag))?;
    text.push('\n');
    Ok(text)
}

/// `text` as a QML string literal. Control characters, the line and
/// paragraph separators, which JavaScript reads as line ends, and the byte
/// order mark are escaped; everything else is written as itself.
// [spec:kbdgen:sem:ldml.linux.touch.plasma]
fn string(text: &str) -> String {
    let mut literal = String::from("\"");
    for c in text.chars() {
        match c {
            '"' => literal.push_str("\\\""),
            '\\' => literal.push_str("\\\\"),
            '\n' => literal.push_str("\\n"),
            '\t' => literal.push_str("\\t"),
            c if c.is_control() || matches!(c, '\u{2028}' | '\u{2029}' | '\u{FEFF}') => {
                let _ = write!(literal, "\\u{:04X}", u32::from(c));
            }
            c => literal.push(c),
        }
    }
    literal.push('"');
    literal
}

/// A width in thousandths of a key as a plasma-keyboard weight, 100 a key.
fn weight(thousandths: u32) -> String {
    match thousandths % 10 {
        0 => format!("{}", thousandths / 10),
        tenths => format!("{}.{tenths}", thousandths / 10),
    }
}

/// `Key { … }` with `properties`, each `name: value`.
fn element(name: &str, properties: &[(&str, String)]) -> String {
    if properties.is_empty() {
        return format!("{name} {{}}");
    }
    let body: Vec<String> = properties
        .iter()
        .map(|(property, value)| format!("{property}: {value}"))
        .collect();
    format!("{name} {{ {} }}", body.join("; "))
}

/// A key's `weight` property, when its width is not one key.
fn weighted(properties: &mut Vec<(&str, String)>, width: u32) {
    if width != DEFAULT_WIDTH {
        properties.push(("weight", weight(width)));
    }
}

/// A `Key` committing `text`, shown as `label`, with `candidates`.
// [spec:kbdgen:sem:ldml.linux.touch.plasma]
fn key(text: &str, label: Option<&str>, candidates: &[String], width: u32, shift: bool) -> String {
    let mut properties = vec![("text", string(text))];
    if let Some(label) = label {
        properties.push(("displayText", string(label)));
    }
    if !candidates.is_empty() {
        let list: Vec<String> = candidates.iter().map(|c| string(c)).collect();
        properties.push(("alternativeKeys", format!("[{}]", list.join(", "))));
    }
    weighted(&mut properties, width);
    if shift {
        properties.push(("noModifier", "true".to_owned()));
    }
    element("Key", &properties)
}

/// The QML element of `item`, none for a dead key without a standalone,
/// which is left out.
// [spec:kbdgen:sem:ldml.linux.touch.plasma]
fn item(item: &Item, shift: bool) -> Option<String> {
    Some(match item {
        Item::Text(TextKey {
            output,
            label,
            candidates,
            width,
        }) => key(output, label.as_deref(), candidates, *width, shift),
        Item::Dead(dead) if dead.standalone.is_empty() => return None,
        Item::Dead(dead) => key(
            &dead.standalone,
            Some(&dead.display),
            &dead.candidates,
            dead.width,
            shift,
        ),
        Item::Gap(width) => {
            let mut properties = Vec::new();
            weighted(&mut properties, *width);
            element("FillerKey", &properties)
        }
        Item::Shift => element("ShiftKey", &[("weight", weight(ADDED_KEY_WIDTH))]),
        Item::Backspace => element("BackspaceKey", &[("weight", weight(ADDED_KEY_WIDTH))]),
        Item::SymbolSwitch => element("SymbolModeKey", &[("weight", weight(ADDED_KEY_WIDTH))]),
    })
}

/// The width of a row as written: its items but the dead keys left out.
fn row_width(row: &[Item]) -> u32 {
    row.iter()
        .filter(|item| !matches!(item, Item::Dead(dead) if dead.standalone.is_empty()))
        .map(Item::width)
        .sum()
}

fn push_line(out: &mut String, depth: usize, line: &str) {
    for _ in 0..depth {
        out.push_str(INDENT);
    }
    out.push_str(line);
    out.push('\n');
}

/// A level's `KeyboardLayout`, every row `width` wide, at `depth`.
/// plasma-keyboard stretches each row to the keyboard's width and sizes
/// its keys by weight (`KeyboardRow.qml`), so a narrower row gets filler
/// of half the difference at each end, and the space bar fills the bottom
/// row.
// [spec:kbdgen:sem:ldml.linux.touch.plasma]
fn keyboard_layout(
    out: &mut String,
    depth: usize,
    level: &Level,
    width: u32,
    space_label: Option<&str>,
) {
    let shift = level.kind == LevelKind::Shift;
    push_line(out, depth, "KeyboardLayout {");
    for row in &level.rows {
        push_line(out, depth + 1, "KeyboardRow {");
        let slack = width.saturating_sub(row_width(row));
        let left = slack / 2;
        if left > 0 {
            push_line(
                out,
                depth + 2,
                &element("FillerKey", &[("weight", weight(left))]),
            );
        }
        for written in row.iter().filter_map(|i| item(i, shift)) {
            push_line(out, depth + 2, &written);
        }
        if slack - left > 0 {
            push_line(
                out,
                depth + 2,
                &element("FillerKey", &[("weight", weight(slack - left))]),
            );
        }
        push_line(out, depth + 1, "}");
        out.push('\n');
    }
    let mut space = vec![("weight", weight(width - BOTTOM_KEYS_WIDTH))];
    if let Some(label) = space_label {
        space.push(("displayText", string(label)));
    }
    let bottom = [
        element("SymbolModeKey", &[("weight", weight(1500))]),
        "ChangeLanguageKey {}".to_owned(),
        "Key { key: Qt.Key_Comma; text: \",\"; secondaryStyle: true }".to_owned(),
        element("SpaceKey", &space),
        "Key { key: Qt.Key_Period; text: \".\"; alternativeKeys: \"!.?\"; smallText: \"!?\"; secondaryStyle: true }"
            .to_owned(),
        "HideKeyboardKey {}".to_owned(),
        element("EnterKey", &[("weight", weight(1500))]),
    ];
    push_line(out, depth + 1, "KeyboardRow {");
    for key in bottom {
        push_line(out, depth + 2, &key);
    }
    push_line(out, depth + 1, "}");
    push_line(out, depth, "}");
}

/// `contents/layouts/desktop.qml`: a `KeyboardLayoutDefinition` with
/// plasma-keyboard's own symbol, number, digit and dial pages, and the
/// default level, or a loader choosing between it and the shift level.
// [spec:kbdgen:sem:ldml.linux.touch.plasma]
fn layout_qml(presented: &Presented) -> String {
    let width = presented
        .hardware_levels()
        .flat_map(|level| level.rows.iter().map(|row| row_width(row)))
        .max()
        .unwrap_or(0)
        .max(BOTTOM_KEYS_WIDTH + MIN_SPACE_WIDTH);
    let space = presented.space_label.as_deref();
    let mut out = String::from(
        "import QtQuick\n\n\
         import org.kde.plasma.keyboard.virtualkeyboard\n\
         import org.kde.plasma.keyboard.virtualkeyboard.components\n\
         import org.kde.plasma.keyboard.layouts.defaultplugin as SharedLayouts\n\n\
         KeyboardLayoutDefinition {\n\
         \x20   symbols: SharedLayouts.Symbols {}\n\
         \x20   numbers: SharedLayouts.Numbers {}\n\
         \x20   digits: SharedLayouts.Digits {}\n\
         \x20   dialpad: SharedLayouts.Dialpad {}\n\n",
    );
    match &presented.shift {
        None => keyboard_layout(&mut out, 1, &presented.default, width, space),
        Some(shift) => {
            push_line(&mut out, 1, "KeyboardLayoutLoader {");
            push_line(&mut out, 2, LOADER_RATIO);
            push_line(
                &mut out,
                2,
                "sourceComponent: inputEngine && inputEngine.uppercase ? shiftLevel : defaultLevel",
            );
            for (id, level) in [("defaultLevel", &presented.default), ("shiftLevel", shift)] {
                out.push('\n');
                push_line(&mut out, 2, "Component {");
                push_line(&mut out, 3, &format!("id: {id}"));
                out.push('\n');
                keyboard_layout(&mut out, 3, level, width, space);
                push_line(&mut out, 2, "}");
            }
            push_line(&mut out, 1, "}");
        }
    }
    out.push_str("}\n");
    out
}

/// The plasma-keyboard package of `layout`, whose presented set is
/// `presented`, at `version` (`targets/linux.yaml`).
// [spec:kbdgen:def:linux.osk.plasma]
pub fn package(
    ids: &Ids,
    version: &str,
    layout: &LinuxLayout,
    presented: &Presented,
) -> Result<Package> {
    let id = ids.kpackage_id(&layout.tag);
    Ok(Package {
        metadata: metadata(&id, version, layout)?,
        layout: layout_qml(presented),
        id,
    })
}
