//! The key maps of a model keyboard: one per hardware layer macOS can
//! select, its modifier terms, and the value of each key it places.

use kbd_model::{
    Component, ExtraModifierKey, Hardware, HardwareLayer, KeyIndex, Keyboard, MarkerIndex,
    ModifierSet, Modifiers, TextElem,
};

use super::modifiers::{Policy, terms};
use crate::build::macos::keymap::MACOS_KEYS;
use crate::build::windows::kbdl::diag::Diagnostics;
use crate::build::windows::kbdl::input::POSITION_NAMES;
use crate::build::windows::kbdl::tables::POSITION_KEYS;

/// The space bar: scan code and macOS key code.
pub const SPACE: (u8, u16) = (0x39, 49);

/// The numpad decimal key's macOS key code.
pub const DECIMAL_CODE: u16 = 65;

/// What a key types: text, or a dead key's marker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    Text(String),
    Dead(MarkerIndex),
}

/// How the engine is made to type a value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Press {
    Key(KeyIndex),
    Decimal,
}

/// A distinct value, with the first key giving it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValueEntry {
    pub value: Value,
    pub press: Press,
}

/// One placed key: its macOS key code and its value's index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cell {
    pub code: u16,
    pub value: usize,
}

/// A key map to write: the hardware layer it comes from, its terms, its
/// cells at the 48 ISO positions in table order, and its space bar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapPlan {
    pub layer: usize,
    pub modifiers: Vec<String>,
    pub cells: Vec<Cell>,
    pub space: Option<Cell>,
}

/// Everything the dead keys, the classification and the key maps need.
#[derive(Debug, Clone, Default)]
pub struct Derived {
    pub maps: Vec<MapPlan>,
    pub default_index: usize,
    /// Distinct values in first-occurrence order: maps, positions, the
    /// space bar, then the decimal key.
    pub values: Vec<ValueEntry>,
    pub decimal: Option<usize>,
    /// `<position> in <layer>` for each output mixing text and markers.
    pub mixed: Vec<String>,
}

/// The 48 ISO positions: name, scan code and macOS key code.
pub fn positions() -> impl Iterator<Item = (&'static str, u8, u16)> {
    POSITION_NAMES
        .iter()
        .zip(POSITION_KEYS.iter())
        .zip(MACOS_KEYS.values())
        .map(|((name, (scan, _)), code)| (*name, *scan, *code as u16))
}

/// A layer as v4 YAML names it: its sets joined by `, `.
pub fn layer_name(layer: &HardwareLayer) -> String {
    let names: Vec<String> = layer
        .modifiers
        .iter()
        .map(|set| match set {
            ModifierSet::Other => "other".to_owned(),
            ModifierSet::Set(s) if s.is_empty() => "none".to_owned(),
            ModifierSet::Set(s) => s
                .components()
                .map(Component::name)
                .collect::<Vec<_>>()
                .join(" "),
        })
        .collect();
    names.join(", ")
}

fn has_extra(set: Modifiers) -> bool {
    set.components().any(|c| c.extra_number().is_some())
}

fn is_sided(set: Modifiers) -> bool {
    use Component::*;
    [AltL, AltR, CtrlL, CtrlR].iter().any(|c| set.contains(*c))
}

/// The policy of a keyboard's terms.
// [spec:kbdgen:sem:ldml.macos.modifiers]
fn policy(keyboard: &Keyboard, hardware: &Hardware) -> Policy {
    let typing_ctrl = hardware.layers.iter().any(|layer| {
        !layer.is_native()
            && layer.modifiers.iter().any(|set| match set {
                ModifierSet::Set(s) => s.components().any(|c| c.name().starts_with("ctrl")),
                ModifierSet::Other => true,
            })
    });
    Policy {
        caps_ignored: keyboard
            .windows
            .extra_modifiers
            .contains(&ExtraModifierKey::CapsLock),
        loose_ctrl: !typing_ctrl,
    }
}

/// The sets of a layer that macOS can select. Sets with an extra modifier
/// or, when Caps Lock is rebound, with `caps`, never match on macOS.
// [spec:kbdgen:sem:ldml.macos.modifiers]
fn selectable_sets(
    layer: &HardwareLayer,
    policy: Policy,
    diag: &mut Diagnostics,
) -> Vec<Modifiers> {
    let name = layer_name(layer);
    let mut sets = Vec::new();
    for set in &layer.modifiers {
        let ModifierSet::Set(set) = set else {
            continue;
        };
        if has_extra(*set) {
            diag.warn(format!(
                "layer {name}: macOS has no extra modifiers, so its set {} is dropped",
                set.components()
                    .map(Component::name)
                    .collect::<Vec<_>>()
                    .join(" ")
            ));
        } else if policy.caps_ignored && set.contains(Component::Caps) {
            diag.warn(format!(
                "layer {name}: Caps Lock is bound as an extra modifier, so no Caps Lock state selects its caps set; dropped"
            ));
        } else {
            sets.push(*set);
        }
    }
    if sets.iter().any(|set| is_sided(*set)) {
        diag.warn(format!(
            "layer {name}: it names a left or right Option or Control key; the .keylayout keeps the side, which macOS may not tell apart"
        ));
    }
    sets
}

/// The value of `key`, if it types anything, recording it as distinct.
// [spec:kbdgen:sem:ldml.macos.keys]
fn cell_value(
    keyboard: &Keyboard,
    key: Option<KeyIndex>,
    at: &str,
    derived: &mut Derived,
    diag: &mut Diagnostics,
) -> Option<usize> {
    let index = key?;
    let key = keyboard.key(index)?;
    if key.gap || key.output.is_empty() {
        return None;
    }
    let value = match key.output.elements() {
        [TextElem::Marker(marker)] => Value::Dead(*marker),
        _ if key.output.is_plain() => Value::Text(key.output.plain()),
        _ => {
            diag.warn(format!(
                "key {at}: its output mixes text and markers or holds several markers, which a .keylayout cannot express; treated as no key"
            ));
            derived.mixed.push(at.to_owned());
            return None;
        }
    };
    Some(intern(derived, value, Press::Key(index)))
}

fn intern(derived: &mut Derived, value: Value, press: Press) -> usize {
    match derived.values.iter().position(|entry| entry.value == value) {
        Some(index) => index,
        None => {
            derived.values.push(ValueEntry { value, press });
            derived.values.len() - 1
        }
    }
}

/// Warns about keys of layer `index` that no key code holds.
// [spec:kbdgen:sem:ldml.macos.keys]
fn check_unplaced_keys(
    keyboard: &Keyboard,
    hardware: &Hardware,
    index: usize,
    diag: &mut Diagnostics,
) {
    let name = layer_name(&hardware.layers[index]);
    for (row, codes) in hardware.form.rows.iter().enumerate() {
        for (col, scan) in codes.iter().enumerate() {
            let placed = *scan == SPACE.0 || positions().any(|(_, s, _)| s == *scan);
            let types = hardware.layers[index]
                .key_at(row, col)
                .and_then(|key| keyboard.key(key))
                .is_some_and(|key| !key.gap && !key.output.is_empty());
            if types && !placed {
                diag.warn(format!(
                    "key at scan code {scan:02X} in layer {name}: macOS has no key code for it here; dropped"
                ));
            }
        }
    }
}

/// One key map per hardware layer with a set macOS can select, or that is
/// `Other`, which becomes the default index.
// [spec:kbdgen:sem:ldml.macos.modifiers]
// [spec:kbdgen:sem:ldml.macos.keys]
pub fn derive(keyboard: &Keyboard, hardware: &Hardware, diag: &mut Diagnostics) -> Derived {
    let policy = policy(keyboard, hardware);
    let mut derived = Derived::default();
    for (index, layer) in hardware.layers.iter().enumerate() {
        let sets = selectable_sets(layer, policy, diag);
        let other = layer.modifiers.contains(&ModifierSet::Other);
        if sets.is_empty() && !other {
            diag.warn(format!(
                "layer {}: no modifier state on macOS selects it; dropped",
                layer_name(layer)
            ));
            continue;
        }
        if other {
            derived.default_index = derived.maps.len();
        }
        check_unplaced_keys(keyboard, hardware, index, diag);
        let name = layer_name(layer);
        let mut cells = Vec::new();
        for (position, scan, code) in positions() {
            let key = hardware.key_for_scan_code(index, scan);
            let at = format!("{position} in layer {name}");
            if let Some(value) = cell_value(keyboard, key, &at, &mut derived, diag) {
                cells.push(Cell { code, value });
            }
        }
        let key = hardware.key_for_scan_code(index, SPACE.0);
        let at = format!("space in layer {name}");
        let space = cell_value(keyboard, key, &at, &mut derived, diag).map(|value| Cell {
            code: SPACE.1,
            value,
        });
        derived.maps.push(MapPlan {
            layer: index,
            modifiers: terms(&sets, policy),
            cells,
            space,
        });
    }
    derived.decimal = decimal_value(keyboard, &mut derived, diag);
    derived
}

/// The decimal key's value, when the keyboard has a plain `decimal`.
// [spec:kbdgen:sem:ldml.macos.keys]
fn decimal_value(
    keyboard: &Keyboard,
    derived: &mut Derived,
    diag: &mut Diagnostics,
) -> Option<usize> {
    let decimal = keyboard.decimal.as_ref()?;
    if !decimal.is_plain() || decimal.is_empty() {
        diag.warn("the decimal key's output holds markers or nothing; the keypad types \".\"");
        return None;
    }
    Some(intern(
        derived,
        Value::Text(decimal.plain()),
        Press::Decimal,
    ))
}
