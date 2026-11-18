//! What each written key gives in its eight states (`ldml.linux.levels`,
//! `ldml.linux.values`).

use std::collections::BTreeMap;

use kbd_engine::ModifierState;
use kbd_model::{Hardware, KeyIndex, Keyboard, MarkerIndex, TextElem};

use super::keycodes::{KEYS, XkbKey};
use super::keysym::Keysym;
use super::types::XkbState;
use crate::build::windows::kbdl::adapter::{LayerSelection, select_layer};

/// What a state of a key outputs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    /// No key, a gap or an empty output.
    Empty,
    Char(char),
    Dead(MarkerIndex),
    /// More than one scalar value.
    Long(String),
    /// Text and markers mixed, or several markers.
    Mixed,
}

/// A written key: its position, the layer and key of each state, and each
/// state's value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyValues {
    pub key: XkbKey,
    pub keys: [Option<KeyIndex>; 8],
    pub values: [Value; 8],
}

/// The written keys of a keyboard and the layer each state selects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Levels {
    pub layers: [Option<usize>; 8],
    pub uses_level3: bool,
    pub keys: Vec<KeyValues>,
}

/// The engine's modifiers for a state: Shift as Left Shift, Lock as Caps
/// Lock, and Level3 as Right Alt sent as AltGr.
pub fn modifiers(state: XkbState) -> ModifierState {
    ModifierState {
        shift_l: state.shift(),
        caps: state.lock(),
        alt_r: state.level3(),
        altgr: state.level3(),
        ..ModifierState::default()
    }
}

/// The value of a key's output.
// [spec:kbdgen:sem:ldml.linux.values+1]
pub fn value(keyboard: &Keyboard, key: Option<KeyIndex>) -> Value {
    let Some(key) = key.and_then(|index| keyboard.key(index)) else {
        return Value::Empty;
    };
    if key.gap || key.output.is_empty() {
        return Value::Empty;
    }
    match key.output.elements() {
        [TextElem::Marker(marker)] => Value::Dead(*marker),
        [TextElem::Char(c)] => Value::Char(*c),
        _ if key.output.is_plain() => Value::Long(key.output.plain()),
        _ => Value::Mixed,
    }
}

/// The keysym of a value, given the dead keysyms of the dead keys that
/// have one.
// [spec:kbdgen:sem:ldml.linux.values+1]
pub fn keysym(keyboard: &Keyboard, dead: &BTreeMap<MarkerIndex, Keysym>, value: &Value) -> Keysym {
    match value {
        Value::Char(c) => Keysym::from_char(*c),
        Value::Dead(marker) => dead.get(marker).copied().unwrap_or_else(|| {
            let mut flush = keyboard
                .flush
                .get(marker)
                .map(|flush| flush.chars())
                .into_iter()
                .flatten();
            match (flush.next(), flush.next()) {
                (Some(c), None) => Keysym::from_char(c),
                _ => Keysym::VOID_SYMBOL,
            }
        }),
        Value::Empty | Value::Long(_) | Value::Mixed => Keysym::VOID_SYMBOL,
    }
}

/// The layer each state selects, as the engine selects it with no extra
/// modifier held, and the values of every key the form has.
// [spec:kbdgen:sem:ldml.linux.levels+1]
pub fn derive(keyboard: &Keyboard, hardware: &Hardware) -> Levels {
    let layers = XkbState::ALL.map(|state| {
        select_layer(
            hardware,
            &keyboard.windows,
            modifiers(state),
            LayerSelection::Engine,
        )
    });
    let uses_level3 = XkbState::ALL
        .iter()
        .any(|state| state.level3() && layers[state.index()].is_some());
    let keys = KEYS
        .iter()
        .filter(|key| hardware.form.position_of(key.scan).is_some())
        .map(|key| {
            let keys = layers
                .map(|layer| layer.and_then(|layer| hardware.key_for_scan_code(layer, key.scan)));
            KeyValues {
                key: *key,
                keys,
                values: keys.map(|index| value(keyboard, index)),
            }
        })
        .collect();
    Levels {
        layers,
        uses_level3,
        keys,
    }
}

/// The `<KPDL>` keysym Num Lock selects, and the decimal when it is more
/// than one scalar value, which keeps `KP_Decimal`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decimal {
    pub keysym: Keysym,
    pub long: Option<String>,
}

/// The decimal key of a keyboard with a `decimal`.
// [spec:kbdgen:sem:ldml.linux.decimal]
pub fn decimal(keyboard: &Keyboard) -> Option<Decimal> {
    let decimal = keyboard.decimal.as_ref()?;
    let keysym = match decimal.elements() {
        [TextElem::Char('.')] => Keysym::KP_DECIMAL,
        [TextElem::Char(',')] => Keysym::KP_SEPARATOR,
        [TextElem::Char(c)] => Keysym::from_char(*c),
        _ => {
            return Some(Decimal {
                keysym: Keysym::KP_DECIMAL,
                long: Some(decimal.plain()),
            });
        }
    };
    Some(Decimal { keysym, long: None })
}
