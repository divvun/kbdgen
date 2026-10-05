//! The `kbdl` layers of a model keyboard: which hardware layer serves each
//! layer's modifier state, which key sits at each of the 49 ISO positions,
//! and the value each key gives.

use anyhow::{Result, bail};
use indexmap::IndexMap;
use kbd_engine::ModifierState;
use kbd_model::{
    Component, ExtraModifierKey, Hardware, KeyIndex, Keyboard, MarkerIndex, ModifierSet, Modifiers,
    Text, TextElem, Windows,
};

use super::super::{
    diag::Diagnostics,
    input::{KeyValue, Layer, POSITION_COUNT, POSITION_NAMES},
    tables::POSITION_KEYS,
};

/// The scan code of the space bar, whose row `kbdl.vk-chars` fixes.
const SPACE_SCAN_CODE: u8 = 0x39;

/// A dead key of the layers: a key whose output is the single marker
/// `marker`, shown as `identity`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeadKey {
    pub marker: MarkerIndex,
    pub key: KeyIndex,
    pub identity: String,
}

/// A distinct value of the layers, with the first key giving it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Value {
    pub key: KeyIndex,
    pub value: KeyValue,
}

/// The layers of `kbdl.input` derived from a keyboard, with what the dead
/// tree and the classification need.
#[derive(Debug, Clone, Default)]
pub struct DerivedLayers {
    pub layers: IndexMap<Layer, Vec<Option<KeyValue>>>,
    /// Distinct values in first-occurrence order: layers in `kbdl.layers`
    /// order, then positions.
    pub values: Vec<Value>,
    /// Dead keys in first-occurrence order, one per marker.
    pub dead_keys: Vec<DeadKey>,
    /// `<position> in <layer>` for each output mixing text and markers.
    pub mixed: Vec<String>,
}

/// How a `kbdl` layer is selected: through the engine's selection, or, for
/// the native-only `ctrl` layer, by exact matching alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Selection {
    Engine,
    Native,
}

/// The `kbdl` layers in `kbdl.layers` order, each with its modifier state.
// [spec:kbdgen:sem:ldml.kbdl.layers]
pub(super) fn layer_states(extra_modifiers: usize) -> Vec<(Layer, ModifierState, Selection)> {
    let shift = ModifierState::shift();
    let caps = ModifierState {
        caps: true,
        ..ModifierState::default()
    };
    let altgr = ModifierState::altgr();
    let mut states = vec![
        (Layer::Default, ModifierState::default(), Selection::Engine),
        (Layer::Shift, shift, Selection::Engine),
        (
            Layer::Ctrl,
            ModifierState {
                ctrl_l: true,
                ..ModifierState::default()
            },
            Selection::Native,
        ),
        (Layer::Alt, altgr, Selection::Engine),
        (
            Layer::AltShift,
            ModifierState {
                shift_l: true,
                ..altgr
            },
            Selection::Engine,
        ),
        (Layer::Caps, caps, Selection::Engine),
        (
            Layer::CapsShift,
            ModifierState {
                shift_l: true,
                ..caps
            },
            Selection::Engine,
        ),
        (
            Layer::AltCaps,
            ModifierState {
                caps: true,
                ..altgr
            },
            Selection::Engine,
        ),
    ];
    for i in 0..extra_modifiers.min(3) {
        let mut extra = ModifierState::default();
        extra.extra[i] = true;
        let index = u8::try_from(i).unwrap_or(u8::MAX);
        states.push((Layer::Extra(index), extra, Selection::Engine));
        states.push((
            Layer::ExtraShift(index),
            ModifierState {
                shift_l: true,
                ..extra
            },
            Selection::Engine,
        ));
    }
    states
}

/// Whether one side pair (any, left, right) of `set` accepts the held
/// state `(l, r)`, as `ldml.engine.modifiers` defines.
fn side_matches(set: Modifiers, sides: [Component; 3], l: bool, r: bool) -> bool {
    let [any, left, right] = sides;
    if set.contains(any) {
        l || r
    } else if set.contains(left) {
        l && !r
    } else if set.contains(right) {
        r && !l
    } else {
        !l && !r
    }
}

/// Whether the component set `set` matches `m` exactly.
// [spec:kbdgen:sem:ldml.kbdl.layers]
fn set_matches(set: Modifiers, m: ModifierState) -> bool {
    use Component::*;
    set.contains(Shift) == (m.shift_l || m.shift_r)
        && set.contains(Caps) == m.caps
        && set.contains(Cmd) == m.cmd
        && [Extra1, Extra2, Extra3]
            .iter()
            .zip(m.extra)
            .all(|(c, held)| set.contains(*c) == held)
        && side_matches(set, [Alt, AltL, AltR], m.alt_l, m.alt_r)
        && side_matches(set, [Ctrl, CtrlL, CtrlR], m.ctrl_l, m.ctrl_r)
}

/// The first layer, native-only or not as `native` says, with a set
/// matching `m` exactly.
fn matching_layer(hardware: &Hardware, m: ModifierState, native: bool) -> Option<usize> {
    hardware.layers.iter().position(|layer| {
        layer.is_native() == native
            && layer.modifiers.iter().any(|set| match set {
                ModifierSet::Set(s) => set_matches(*s, m),
                ModifierSet::Other => false,
            })
    })
}

/// The hardware layer serving `m`: for the engine's selection, after the
/// extra-modifier bindings (`ldml.engine.extra`), an exact non-native match,
/// then AltGr's retry with Left Ctrl (`ldml.engine.altgr`), then `Other`;
/// for a native-only state, an exact native match.
// [spec:kbdgen:sem:ldml.kbdl.layers]
fn select(
    hardware: &Hardware,
    windows: &Windows,
    m: ModifierState,
    selection: Selection,
) -> Option<usize> {
    if selection == Selection::Native {
        return matching_layer(hardware, m, true);
    }
    let caps_rebound = windows
        .extra_modifiers
        .contains(&ExtraModifierKey::CapsLock);
    let m = ModifierState {
        caps: m.caps && !caps_rebound,
        ..m
    };
    matching_layer(hardware, m, false)
        .or_else(|| {
            (m.altgr && m.alt_r && !m.ctrl_l && !m.ctrl_r)
                .then(|| matching_layer(hardware, ModifierState { ctrl_l: true, ..m }, false))
                .flatten()
        })
        .or_else(|| {
            hardware.layers.iter().position(|layer| {
                !layer.is_native() && layer.modifiers.contains(&ModifierSet::Other)
            })
        })
}

/// The dead identity of marker `marker`: the display of `\m{marker}`, or
/// else its flush output.
// [spec:kbdgen:sem:ldml.kbdl.values+1]
pub fn dead_identity(keyboard: &Keyboard, marker: MarkerIndex) -> Option<String> {
    let output = Text(vec![TextElem::Marker(marker)]);
    keyboard
        .displays
        .for_output(&output)
        .filter(|display| !display.is_empty())
        .or_else(|| {
            keyboard
                .flush
                .get(&marker)
                .map(String::as_str)
                .filter(|flush| !flush.is_empty())
        })
        .map(str::to_owned)
}

/// Derives every layer whose state some hardware layer serves.
// [spec:kbdgen:sem:ldml.kbdl.layers]
// [spec:kbdgen:sem:ldml.kbdl.caps]
// [spec:kbdgen:req:ldml.kbdl.windows-inputs]
pub fn derive(
    keyboard: &Keyboard,
    hardware: &Hardware,
    diag: &mut Diagnostics,
) -> Result<DerivedLayers> {
    let mut derived = DerivedLayers::default();
    let states = layer_states(keyboard.windows.extra_modifiers.len());
    for (layer, state, selection) in states {
        let Some(index) = select(hardware, &keyboard.windows, state, selection) else {
            continue;
        };
        check_unplaced_keys(keyboard, hardware, index, layer, diag);
        let mut values = Vec::with_capacity(POSITION_COUNT);
        for (position, (scan_code, _)) in POSITION_KEYS.iter().enumerate() {
            let key = hardware.key_for_scan_code(index, *scan_code);
            let value = key_value(keyboard, key, position, layer, &mut derived, diag)?;
            values.push(value);
        }
        derived.layers.insert(layer, values);
    }
    Ok(derived)
}

/// The value of the key at one position, recording distinct values, dead
/// keys and mixed outputs as they first occur.
// [spec:kbdgen:sem:ldml.kbdl.positions+1]
// [spec:kbdgen:sem:ldml.kbdl.values+1]
fn key_value(
    keyboard: &Keyboard,
    key: Option<KeyIndex>,
    position: usize,
    layer: Layer,
    derived: &mut DerivedLayers,
    diag: &mut Diagnostics,
) -> Result<Option<KeyValue>> {
    let Some((index, key)) = key.and_then(|index| Some((index, keyboard.key(index)?))) else {
        return Ok(None);
    };
    if key.gap || key.output.is_empty() {
        return Ok(None);
    }
    let name = POSITION_NAMES[position];
    let value = match key.output.elements() {
        [TextElem::Marker(marker)] => {
            let marker = *marker;
            let Some(identity) = dead_identity(keyboard, marker) else {
                bail!(
                    "{}: key {name} in layer {layer}: dead key \\m{{{}}} has neither a display nor a flush output to identify it",
                    diag.layout(),
                    keyboard.marker_name(marker).unwrap_or("?")
                );
            };
            record_dead_key(keyboard, marker, index, &identity, derived, diag)?;
            KeyValue::dead(identity)
        }
        _ if key.output.is_plain() => KeyValue::new(key.output.plain()),
        _ => {
            diag.warn(format!(
                "key {name} in layer {layer}: its output mixes text and markers or holds several markers, which the DLL cannot express; treated as no key"
            ));
            derived.mixed.push(format!("{name} in {layer}"));
            return Ok(None);
        }
    };
    if !derived.values.iter().any(|v| v.value == value) {
        derived.values.push(Value {
            key: index,
            value: value.clone(),
        });
    }
    Ok(Some(value))
}

/// Records the first key of each dead-key marker. Two markers with one
/// identity cannot both be dead keys of the tables.
fn record_dead_key(
    keyboard: &Keyboard,
    marker: MarkerIndex,
    key: KeyIndex,
    identity: &str,
    derived: &mut DerivedLayers,
    diag: &Diagnostics,
) -> Result<()> {
    if derived.dead_keys.iter().any(|dead| dead.marker == marker) {
        return Ok(());
    }
    if let Some(other) = derived.dead_keys.iter().find(|d| d.identity == identity) {
        bail!(
            "{}: dead keys \\m{{{}}} and \\m{{{}}} are both identified as {identity:?}",
            diag.layout(),
            keyboard.marker_name(other.marker).unwrap_or("?"),
            keyboard.marker_name(marker).unwrap_or("?")
        );
    }
    derived.dead_keys.push(DeadKey {
        marker,
        key,
        identity: identity.to_owned(),
    });
    Ok(())
}

/// Warns about keys of hardware layer `index` that no position of the
/// tables holds: scan codes outside the 49 ISO positions, and a space bar
/// whose output is not U+0020.
// [spec:kbdgen:sem:ldml.kbdl.positions+1]
fn check_unplaced_keys(
    keyboard: &Keyboard,
    hardware: &Hardware,
    index: usize,
    layer: Layer,
    diag: &mut Diagnostics,
) {
    for (row, codes) in hardware.form.rows.iter().enumerate() {
        for (col, scan_code) in codes.iter().enumerate() {
            let Some(key) = hardware.layers[index]
                .key_at(row, col)
                .and_then(|key| keyboard.key(key))
                .filter(|key| !key.gap && !key.output.is_empty())
            else {
                continue;
            };
            if *scan_code == SPACE_SCAN_CODE {
                if key.output != Text::from(" ") {
                    diag.warn(format!(
                        "space bar in layer {layer}: the DLL's space row always types U+0020, so its output {:?} is dropped",
                        key.output.plain()
                    ));
                }
            } else if !POSITION_KEYS.iter().any(|(code, _)| code == scan_code) {
                diag.warn(format!(
                    "key at scan code {scan_code:02X} in layer {layer}: the DLL has no position outside the 49 ISO keys; dropped"
                ));
            }
        }
    }
}
