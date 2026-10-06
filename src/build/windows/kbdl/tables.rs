//! Computes the `KBDTABLES` contents of one layout from its input: modifier
//! bindings, character tables, dead keys, ligatures, scan-code maps, key
//! names and locale flags.

use std::collections::{BTreeSet, HashMap};

use anyhow::{Result, anyhow, bail};
use indexmap::IndexMap;

use super::{
    diag::Diagnostics,
    input::{
        B00, B11, DeadKeyNode, ExtraModifierKey, KeyNameEntry, KeyNameTable, KeyValue, Layer,
        LayoutInput, MAX_EXTRA_MODIFIERS, POSITION_COUNT, POSITION_NAMES, STANDALONE,
    },
};

pub const WCH_NONE: u16 = 0xF000;
pub const WCH_DEAD: u16 = 0xF001;
pub const WCH_LGTR: u16 = 0xF002;

pub const CAPLOK: u8 = 0x01;
pub const SGCAPS: u8 = 0x02;
pub const CAPLOKALTGR: u8 = 0x04;

pub const SHFT_INVALID: u8 = 0x0F;
pub const DKF_DEAD: u16 = 0x0001;
pub const KBDEXT: u16 = 0x100;

pub const KBD_VERSION: u32 = 1;
pub const KLLF_ALTGR: u32 = 0x0001;
pub const KLLF_SHIFTLOCK: u32 = 0x0002;
pub const KLLF_LRM_RLM: u32 = 0x0004;

/// Virtual key of a row that holds the dead characters of the row before it.
pub const VK_DEAD_ROW: u8 = 0xff;
const VK_SPACE: u8 = 0x20;
const VK_DECIMAL: u8 = 0x6e;

/// Windows delivers at most 16 UTF-16 units per keystroke, so no ligature
/// is longer; longer values become "no key".
// [spec:kbdgen:thm:kbdl.ligatures.limit]
pub const MAX_LIGATURE_UNITS: usize = 16;

/// The highest private-use unit a dead id may fall back to; fallback ids
/// are taken from here downward within the private use area.
const DEAD_ID_FALLBACK_HIGH: u16 = 0xF8FF;
const DEAD_ID_FALLBACK_LOW: u16 = 0xE000;

/// Number of entries in `ausVK`, scan codes `0x00`..`0x7e`.
pub const SCANCODE_COUNT: usize = 127;

/// `ausVK` for `KBD_TYPE` 4, before extra-modifier rebinding.
// [spec:kbdgen:def:kbdl.scancodes]
pub const AUS_VK: [u16; SCANCODE_COUNT] = [
    0x0ff, 0x01b, 0x031, 0x032, 0x033, 0x034, 0x035, 0x036, 0x037, 0x038, 0x039, 0x030, 0x0bd,
    0x0bb, 0x008, 0x009, 0x051, 0x057, 0x045, 0x052, 0x054, 0x059, 0x055, 0x049, 0x04f, 0x050,
    0x0db, 0x0dd, 0x00d, 0x0a2, 0x041, 0x053, 0x044, 0x046, 0x047, 0x048, 0x04a, 0x04b, 0x04c,
    0x0ba, 0x0de, 0x0c0, 0x0a0, 0x0dc, 0x05a, 0x058, 0x043, 0x056, 0x042, 0x04e, 0x04d, 0x0bc,
    0x0be, 0x0bf, 0x1a1, 0x26a, 0x0a4, 0x020, 0x014, 0x070, 0x071, 0x072, 0x073, 0x074, 0x075,
    0x076, 0x077, 0x078, 0x079, 0x390, 0x291, 0xc24, 0xc26, 0xc21, 0x06d, 0xc25, 0xc0c, 0xc27,
    0x06b, 0xc23, 0xc28, 0xc22, 0xc2d, 0xc2e, 0x02c, 0x0ff, 0x0e2, 0x07a, 0x07b, 0x00c, 0x0ee,
    0x0f1, 0x0ea, 0x0f9, 0x0f5, 0x0f3, 0x0ff, 0x0ff, 0x0fb, 0x02f, 0x07c, 0x07d, 0x07e, 0x07f,
    0x080, 0x081, 0x082, 0x083, 0x084, 0x085, 0x086, 0x0ed, 0x0ff, 0x0e9, 0x0ff, 0x0c1, 0x0ff,
    0x0ff, 0x087, 0x0ff, 0x0ff, 0x0ff, 0x0ff, 0x0eb, 0x009, 0x0ff, 0x0c2,
];

/// `aE0VscToVk` without its terminator, before extra-modifier rebinding.
// [spec:kbdgen:def:kbdl.scancodes.extended]
pub const E0_VSC_TO_VK: [(u8, u16); 38] = [
    (0x10, 0x1b1),
    (0x19, 0x1b0),
    (0x1d, 0x1a3),
    (0x20, 0x1ad),
    (0x21, 0x1b7),
    (0x22, 0x1b3),
    (0x24, 0x1b2),
    (0x2e, 0x1ae),
    (0x30, 0x1af),
    (0x32, 0x1ac),
    (0x35, 0x16f),
    (0x37, 0x12c),
    (0x38, 0x1a5),
    (0x47, 0x124),
    (0x48, 0x126),
    (0x49, 0x121),
    (0x4b, 0x125),
    (0x4d, 0x127),
    (0x4f, 0x123),
    (0x50, 0x128),
    (0x51, 0x122),
    (0x52, 0x12d),
    (0x53, 0x12e),
    (0x5b, 0x15b),
    (0x5c, 0x15c),
    (0x5d, 0x15d),
    (0x5f, 0x15f),
    (0x65, 0x1aa),
    (0x66, 0x1ab),
    (0x67, 0x1a8),
    (0x68, 0x1a9),
    (0x69, 0x1a7),
    (0x6a, 0x1a6),
    (0x6b, 0x1b6),
    (0x6c, 0x1b4),
    (0x6d, 0x1b5),
    (0x1c, 0x10d),
    (0x46, 0x103),
];

/// `aE1VscToVk` without its terminator.
// [spec:kbdgen:def:kbdl.scancodes.extended]
pub const E1_VSC_TO_VK: [(u8, u16); 1] = [(0x1d, 0x13)];

/// Scan code and virtual key of each ISO position, in [`POSITION_NAMES`]
/// order; each virtual key equals the low byte of `AUS_VK` at its scan code.
// [spec:kbdgen:req:kbdl.scancodes.iso]
pub const POSITION_KEYS: [(u8, u8); POSITION_COUNT] = [
    (0x29, 0xc0),
    (0x02, b'1'),
    (0x03, b'2'),
    (0x04, b'3'),
    (0x05, b'4'),
    (0x06, b'5'),
    (0x07, b'6'),
    (0x08, b'7'),
    (0x09, b'8'),
    (0x0a, b'9'),
    (0x0b, b'0'),
    (0x0c, 0xbd),
    (0x0d, 0xbb),
    (0x10, b'Q'),
    (0x11, b'W'),
    (0x12, b'E'),
    (0x13, b'R'),
    (0x14, b'T'),
    (0x15, b'Y'),
    (0x16, b'U'),
    (0x17, b'I'),
    (0x18, b'O'),
    (0x19, b'P'),
    (0x1a, 0xdb),
    (0x1b, 0xdd),
    (0x1e, b'A'),
    (0x1f, b'S'),
    (0x20, b'D'),
    (0x21, b'F'),
    (0x22, b'G'),
    (0x23, b'H'),
    (0x24, b'J'),
    (0x25, b'K'),
    (0x26, b'L'),
    (0x27, 0xba),
    (0x28, 0xde),
    (0x2b, 0xdc),
    (0x56, 0xe2),
    (0x2c, b'Z'),
    (0x2d, b'X'),
    (0x2e, b'C'),
    (0x2f, b'V'),
    (0x30, b'B'),
    (0x31, b'N'),
    (0x32, b'M'),
    (0x33, 0xbc),
    (0x34, 0xbe),
    (0x35, 0xbf),
    (0x73, 0xc1),
];

/// Virtual keys the extra modifiers are bound to, in modifier order:
/// `VK_OEM_8` and two unassigned virtual keys.
// [spec:kbdgen:req:kbdl.modifiers]
pub const EXTRA_MODIFIER_VKS: [u8; MAX_EXTRA_MODIFIERS] = [0xdf, 0xe8, 0x97];

/// The fixed three-column table: Back, Escape, Return, Cancel.
// [spec:kbdgen:def:kbdl.vk-chars.fixed]
pub const FIXED_VK_TO_WCH3: [(u8, [u16; 3]); 4] = [
    (0x08, [0x0008, 0x0008, 0x007f]),
    (0x1b, [0x001b, 0x001b, 0x001b]),
    (0x0d, [0x000d, 0x000d, 0x000a]),
    (0x03, [0x0003, 0x0003, 0x0003]),
];

/// The fixed two-column table: Tab and the numpad operators.
// [spec:kbdgen:def:kbdl.vk-chars.fixed]
pub const FIXED_VK_TO_WCH2: [(u8, [u16; 2]); 5] = [
    (0x09, [0x0009, 0x0009]),
    (0x6b, [0x002b, 0x002b]),
    (0x6f, [0x002f, 0x002f]),
    (0x6a, [0x002a, 0x002a]),
    (0x6d, [0x002d, 0x002d]),
];

/// The fixed one-column table: Numpad 0-9.
// [spec:kbdgen:def:kbdl.vk-chars.fixed]
pub const FIXED_VK_TO_WCH1: [(u8, [u16; 1]); 10] = [
    (0x60, [0x0030]),
    (0x61, [0x0031]),
    (0x62, [0x0032]),
    (0x63, [0x0033]),
    (0x64, [0x0034]),
    (0x65, [0x0035]),
    (0x66, [0x0036]),
    (0x67, [0x0037]),
    (0x68, [0x0038]),
    (0x69, [0x0039]),
];

/// `aKeyNames`, by scan code, without its terminator.
// [spec:kbdgen:req:kbdl.key-names]
pub const KEY_NAMES: [(u8, &str); 51] = [
    (0x01, "Esc"),
    (0x0e, "Backspace"),
    (0x0f, "Tab"),
    (0x1c, "Enter"),
    (0x1d, "Ctrl"),
    (0x2a, "Shift"),
    (0x36, "Right Shift"),
    (0x37, "Num *"),
    (0x38, "Alt"),
    (0x39, "Space"),
    (0x3a, "Caps Lock"),
    (0x3b, "F1"),
    (0x3c, "F2"),
    (0x3d, "F3"),
    (0x3e, "F4"),
    (0x3f, "F5"),
    (0x40, "F6"),
    (0x41, "F7"),
    (0x42, "F8"),
    (0x43, "F9"),
    (0x44, "F10"),
    (0x45, "Pause"),
    (0x46, "Scroll Lock"),
    (0x47, "Num 7"),
    (0x48, "Num 8"),
    (0x49, "Num 9"),
    (0x4a, "Num -"),
    (0x4b, "Num 4"),
    (0x4c, "Num 5"),
    (0x4d, "Num 6"),
    (0x4e, "Num +"),
    (0x4f, "Num 1"),
    (0x50, "Num 2"),
    (0x51, "Num 3"),
    (0x52, "Num 0"),
    (0x53, "Num Del"),
    (0x54, "Sys Req"),
    (0x57, "F11"),
    (0x58, "F12"),
    (0x7c, "F13"),
    (0x7d, "F14"),
    (0x7e, "F15"),
    (0x7f, "F16"),
    (0x80, "F17"),
    (0x81, "F18"),
    (0x82, "F19"),
    (0x83, "F20"),
    (0x84, "F21"),
    (0x85, "F22"),
    (0x86, "F23"),
    (0x87, "F24"),
];

/// `aKeyNamesExt`, by scan code, without its terminator.
// [spec:kbdgen:req:kbdl.key-names]
pub const KEY_NAMES_EXT: [(u8, &str); 22] = [
    (0x1c, "Num Enter"),
    (0x1d, "Right Ctrl"),
    (0x35, "Num /"),
    (0x37, "Prnt Scrn"),
    (0x38, "Right Alt"),
    (0x45, "Num Lock"),
    (0x46, "Break"),
    (0x47, "Home"),
    (0x48, "Up"),
    (0x49, "Page Up"),
    (0x4b, "Left"),
    (0x4d, "Right"),
    (0x4f, "End"),
    (0x50, "Down"),
    (0x51, "Page Down"),
    (0x52, "Insert"),
    (0x53, "Delete"),
    (0x54, "<00>"),
    (0x56, "Help"),
    (0x5b, "Left Windows"),
    (0x5c, "Right Windows"),
    (0x5d, "Application"),
];

/// One `VK_TO_WCHARS` row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub vk: u8,
    pub attributes: u8,
    pub wch: Vec<u16>,
}

/// One `DEADKEY` entry, `DEADTRANS(base, id, composed, flags)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeadKeyEntry {
    pub base: u16,
    pub id: u16,
    pub composed: u16,
    pub flags: u16,
}

impl DeadKeyEntry {
    pub fn both(&self) -> u32 {
        u32::from(self.base) | (u32::from(self.id) << 16)
    }
}

/// One `LIGATURE` entry before padding to the table's width.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LigatureEntry {
    pub vk: u8,
    pub column: u16,
    pub units: Vec<u16>,
}

/// One `VSC_LPWSTR` entry; `name` excludes the terminator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyName {
    pub scan_code: u8,
    pub name: Vec<u16>,
}

/// Everything `KbdTables` points to, without table terminators.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tables {
    /// *N*, the column count of the layout's character table.
    pub columns: usize,
    pub vk_to_bits: Vec<(u8, u8)>,
    pub max_mod_bits: u16,
    pub mod_numbers: Vec<u8>,
    pub rows: Vec<Row>,
    pub dead_keys: Vec<DeadKeyEntry>,
    /// Each entry is a dead id followed by the dead key's name.
    pub dead_key_names: Vec<Vec<u16>>,
    pub ligatures: Vec<LigatureEntry>,
    pub key_names: Vec<KeyName>,
    pub key_names_ext: Vec<KeyName>,
    pub aus_vk: Vec<u16>,
    pub e0_vsc_to_vk: Vec<(u8, u16)>,
    pub e1_vsc_to_vk: Vec<(u8, u16)>,
    pub locale_flags: u32,
}

impl Tables {
    /// `nLgMax`: the longest ligature's unit count, or 0 without ligatures.
    pub fn lg_max(&self) -> usize {
        self.ligatures
            .iter()
            .map(|ligature| ligature.units.len())
            .max()
            .unwrap_or(0)
    }
}

/// What one value puts in its cell.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Cell {
    None,
    Char(u16),
    Dead(u16),
    Ligature(Vec<u16>),
}

fn is_special(unit: u16) -> bool {
    (WCH_NONE..=WCH_LGTR).contains(&unit)
}

fn units(text: &str) -> Vec<u16> {
    text.encode_utf16().collect()
}

/// Renders units for messages, escaping everything outside printable ASCII.
fn show(text: &str) -> String {
    format!("{:?}", text)
}

/// The column a layer's values occupy, or `None` for `caps`, `caps+shift`
/// and `alt+caps`, which only feed SGCAPS rows and attributes.
// [spec:kbdgen:def:kbdl.layers]
fn column_of(layer: Layer) -> Option<usize> {
    match layer {
        Layer::Default => Some(0),
        Layer::Shift => Some(1),
        Layer::Ctrl => Some(2),
        Layer::Alt => Some(3),
        Layer::AltShift => Some(4),
        Layer::Extra(i) => Some(5 + 2 * usize::from(i)),
        Layer::ExtraShift(i) => Some(6 + 2 * usize::from(i)),
        Layer::Caps | Layer::CapsShift | Layer::AltCaps => None,
    }
}

/// The layer whose values fill each column, for *N* columns.
// [spec:kbdgen:def:kbdl.layers]
fn column_layers(extra_modifiers: usize) -> Vec<Layer> {
    let mut layers = vec![
        Layer::Default,
        Layer::Shift,
        Layer::Ctrl,
        Layer::Alt,
        Layer::AltShift,
    ];
    for i in 0..extra_modifiers {
        let i = u8::try_from(i).unwrap_or(u8::MAX);
        layers.push(Layer::Extra(i));
        layers.push(Layer::ExtraShift(i));
    }
    debug_assert!(
        layers
            .iter()
            .enumerate()
            .all(|(column, layer)| column_of(*layer) == Some(column))
    );
    layers
}

/// Values of every layer at every position, with absent layers and ignored
/// values reading as "no key".
struct Grid<'a> {
    layers: &'a IndexMap<Layer, Vec<Option<KeyValue>>>,
    b00_rebound: bool,
}

impl<'a> Grid<'a> {
    fn get(&self, layer: Layer, position: usize) -> Option<&'a KeyValue> {
        if position == B00 && self.b00_rebound {
            return None;
        }
        self.layers
            .get(&layer)
            .and_then(|values| values.get(position))
            .and_then(Option::as_ref)
    }
}

/// Builds the tables for one layout. Fatal conditions are returned as
/// errors; everything else that cannot be represented is reported through
/// `diag` and degraded as `kbdl.*` specifies.
pub fn build(input: &LayoutInput, diag: &mut Diagnostics) -> Result<Tables> {
    let layout = diag.layout().to_owned();
    check_input_shape(input, &layout, diag)?;

    let extra = input.extra_modifiers.len();
    let columns = 5 + 2 * extra;
    let b00_rebound = input.extra_modifiers.contains(&ExtraModifierKey::B00);
    let grid = Grid {
        layers: &input.layers,
        b00_rebound,
    };

    check_reserved_units(input, &layout)?;
    let (vk_to_bits, max_mod_bits, mod_numbers) = modifiers(extra);
    let (aus_vk, e0_vsc_to_vk) = scancodes(&input.extra_modifiers);

    let mut builder = CharTableBuilder {
        columns,
        layers: column_layers(extra),
        rows: Vec::new(),
        ligatures: Vec::new(),
        key_dead_chars: Vec::new(),
        diag,
    };
    for position in 0..POSITION_COUNT {
        builder.position(&grid, position)?;
    }
    let decimal = decimal_char(input.decimal.as_deref(), &layout, builder.diag)?;
    builder.fixed_rows(decimal);

    let CharTableBuilder {
        mut rows,
        ligatures,
        key_dead_chars,
        diag,
        ..
    } = builder;

    let emitted = emitted_units(&rows, &ligatures, decimal, &input.dead_key_tree);
    let dead = dead_keys(
        &key_dead_chars,
        &input.dead_key_tree,
        &emitted,
        &layout,
        diag,
    )?;
    substitute_dead_ids(&mut rows, &dead.ids);
    let dead_key_names = dead_key_names(&key_dead_chars, &dead.ids, &input.dead_key_names, diag);
    let (key_names, key_names_ext) = key_names(&input.key_name_overrides, diag);
    let locale_flags = locale_flags(&rows, input.shift_lock, input.lrm_rlm);

    let tables = Tables {
        columns,
        vk_to_bits,
        max_mod_bits,
        mod_numbers,
        rows,
        dead_keys: dead.entries,
        dead_key_names,
        ligatures,
        key_names,
        key_names_ext,
        aus_vk,
        e0_vsc_to_vk,
        e1_vsc_to_vk: E1_VSC_TO_VK.to_vec(),
        locale_flags,
    };
    check_invariants(&tables).map_err(|error| anyhow!("{layout}: {error}"))?;
    Ok(tables)
}

/// Rejects inputs outside the shape `kbdl.input` defines and warns about
/// values that no column or rebinding can use.
// [spec:kbdgen:def:kbdl.input]
fn check_input_shape(input: &LayoutInput, layout: &str, diag: &mut Diagnostics) -> Result<()> {
    if input.extra_modifiers.len() > MAX_EXTRA_MODIFIERS {
        bail!(
            "{layout}: {} extra modifiers given, at most {MAX_EXTRA_MODIFIERS} are possible",
            input.extra_modifiers.len()
        );
    }
    for (i, key) in input.extra_modifiers.iter().enumerate() {
        if input.extra_modifiers[..i].contains(key) {
            bail!("{layout}: extra modifier key {key} is bound more than once");
        }
    }
    for (layer, values) in &input.layers {
        if values.len() != POSITION_COUNT {
            bail!(
                "{layout}: layer {layer} has {} values, expected {POSITION_COUNT}",
                values.len()
            );
        }
        if let Layer::Extra(i) | Layer::ExtraShift(i) = layer
            && usize::from(*i) >= input.extra_modifiers.len()
        {
            diag.warn(format!(
                "layer {layer} has no extra modifier to select it; its values are ignored"
            ));
        }
    }
    if input.extra_modifiers.contains(&ExtraModifierKey::B00) {
        for (layer, values) in &input.layers {
            if values[B00].is_some() {
                diag.warn(format!(
                    "key B00 in layer {layer}: B00 is an extra modifier, so its value is ignored"
                ));
            }
        }
    }
    Ok(())
}

/// Any value containing `WCH_NONE`, `WCH_DEAD` or `WCH_LGTR` is fatal,
/// since Windows would read it as that marker.
// [spec:kbdgen:sem:kbdl.vk-chars.values]
fn check_reserved_units(input: &LayoutInput, layout: &str) -> Result<()> {
    let b00_rebound = input.extra_modifiers.contains(&ExtraModifierKey::B00);
    for (layer, values) in &input.layers {
        for (position, value) in values.iter().enumerate() {
            if position == B00 && b00_rebound {
                continue;
            }
            if let Some(value) = value
                && value.text.encode_utf16().any(is_special)
            {
                bail!(
                    "{layout}: key {} in layer {layer}: value {} contains a unit in 0xF000-0xF002",
                    POSITION_NAMES[position],
                    show(&value.text)
                );
            }
        }
    }
    Ok(())
}

/// `aVkToBits`, `wMaxModBits` and `ModNumber` for `extra` extra modifiers.
// [spec:kbdgen:req:kbdl.modifiers]
fn modifiers(extra: usize) -> (Vec<(u8, u8)>, u16, Vec<u8>) {
    let mut vk_to_bits = vec![(0x10, 0x01), (0x11, 0x02), (0x12, 0x04)];
    let mut columns: Vec<(usize, u8)> = vec![(0, 0), (1, 1), (2, 2), (6, 3), (7, 4)];
    for (i, vk) in EXTRA_MODIFIER_VKS.iter().take(extra).enumerate() {
        let bit = 0x08usize << i;
        vk_to_bits.push((*vk, bit as u8));
        columns.push((bit, (5 + 2 * i) as u8));
        columns.push((bit | 1, (6 + 2 * i) as u8));
    }
    let max_mod_bits: u16 = if extra == 0 {
        7
    } else {
        ((0x08u16) << (extra - 1)) | 1
    };
    let mut mod_numbers = vec![SHFT_INVALID; usize::from(max_mod_bits) + 1];
    for (bits, column) in columns {
        mod_numbers[bits] = column;
    }
    (vk_to_bits, max_mod_bits, mod_numbers)
}

/// `ausVK` and `aE0VscToVk` with each extra modifier's key rebound to the
/// modifier's virtual key.
// [spec:kbdgen:req:kbdl.scancodes.extra-modifiers]
fn scancodes(extra_modifiers: &[ExtraModifierKey]) -> (Vec<u16>, Vec<(u8, u16)>) {
    let mut aus_vk = AUS_VK.to_vec();
    let mut e0 = E0_VSC_TO_VK.to_vec();
    for (key, vk) in extra_modifiers.iter().zip(EXTRA_MODIFIER_VKS) {
        let vk = u16::from(vk);
        match key {
            ExtraModifierKey::RightCtrl => {
                for entry in e0.iter_mut().filter(|(scan_code, _)| *scan_code == 0x1d) {
                    entry.1 = vk | KBDEXT;
                }
            }
            ExtraModifierKey::CapsLock => aus_vk[0x3a] = vk,
            ExtraModifierKey::B00 => aus_vk[0x56] = vk,
        }
    }
    (aus_vk, e0)
}

/// Accumulates the main character table in emission order.
struct CharTableBuilder<'d> {
    columns: usize,
    layers: Vec<Layer>,
    rows: Vec<Row>,
    ligatures: Vec<LigatureEntry>,
    /// Distinct dead units in order of first emission.
    key_dead_chars: Vec<u16>,
    diag: &'d mut Diagnostics,
}

impl CharTableBuilder<'_> {
    /// Classifies one value for a cell of the main table.
    // [spec:kbdgen:sem:kbdl.vk-chars.values]
    fn classify(&mut self, value: Option<&KeyValue>, position: usize, layer: Layer) -> Cell {
        let Some(value) = value else {
            return Cell::None;
        };
        let units = units(&value.text);
        let position = POSITION_NAMES[position];
        match units.len() {
            0 => Cell::None,
            1 if value.dead => Cell::Dead(units[0]),
            1 => Cell::Char(units[0]),
            2..=MAX_LIGATURE_UNITS => {
                if value.dead {
                    self.diag.warn(format!(
                        "key {position} in layer {layer}: ligature {} cannot be a dead key; the dead-key flag is ignored",
                        show(&value.text)
                    ));
                }
                Cell::Ligature(units)
            }
            n => {
                self.diag.warn(format!(
                    "key {position} in layer {layer}: value {} has {n} UTF-16 units, more than the {MAX_LIGATURE_UNITS} Windows delivers per keystroke; treated as no key",
                    show(&value.text)
                ));
                Cell::None
            }
        }
    }

    /// Emits the rows of one ISO position, if it has any.
    // [spec:kbdgen:req:kbdl.vk-chars]
    fn position(&mut self, grid: &Grid, position: usize) -> Result<()> {
        if position == B00 && grid.b00_rebound {
            return Ok(());
        }
        if position == B11
            && grid
                .layers
                .values()
                .all(|values| values.get(B11).is_none_or(Option::is_none))
        {
            return Ok(());
        }
        let vk = POSITION_KEYS[position].1;
        let name = POSITION_NAMES[position];

        let layers = self.layers.clone();
        let cells: Vec<Cell> = layers
            .iter()
            .map(|layer| self.classify(grid.get(*layer, position), position, *layer))
            .collect();
        let has_dead = cells.iter().any(|cell| matches!(cell, Cell::Dead(_)));

        let mut attributes = attributes(grid, position);
        if attributes == SGCAPS && has_dead {
            self.diag.warn(format!(
                "key {name}: SGCAPS cannot be combined with a dead key on the same key; the caps layers are ignored"
            ));
            attributes = caps_attributes(grid, position, false);
        }

        let mut wch = Vec::with_capacity(self.columns);
        for (column, cell) in cells.iter().enumerate() {
            wch.push(match cell {
                Cell::None => WCH_NONE,
                Cell::Char(unit) => *unit,
                Cell::Dead(unit) => {
                    if !self.key_dead_chars.contains(unit) {
                        self.key_dead_chars.push(*unit);
                    }
                    WCH_DEAD
                }
                Cell::Ligature(units) => {
                    self.ligatures.push(LigatureEntry {
                        vk,
                        column: column as u16,
                        units: units.clone(),
                    });
                    WCH_LGTR
                }
            });
        }
        self.rows.push(Row {
            vk,
            attributes,
            wch,
        });

        if has_dead {
            self.dead_row(&cells);
        } else if attributes == SGCAPS {
            self.sgcaps_row(grid, position, vk);
        }
        Ok(())
    }

    /// The `0xff` row after a row with dead keys. It holds the dead
    /// characters until [`substitute_dead_ids`] replaces them with ids.
    // [spec:kbdgen:req:kbdl.dead-keys+2]
    fn dead_row(&mut self, cells: &[Cell]) {
        let wch = cells
            .iter()
            .map(|cell| match cell {
                Cell::Dead(unit) => *unit,
                _ => WCH_NONE,
            })
            .collect();
        self.rows.push(Row {
            vk: VK_DEAD_ROW,
            attributes: 0,
            wch,
        });
    }

    /// The row after an SGCAPS row: `caps` and `caps+shift` in columns 0
    /// and 1, which cannot hold dead keys or ligatures.
    // [spec:kbdgen:req:kbdl.caps.sgcaps]
    fn sgcaps_row(&mut self, grid: &Grid, position: usize, vk: u8) {
        let name = POSITION_NAMES[position];
        let mut wch = vec![WCH_NONE; self.columns];
        for (column, layer) in [(0, Layer::Caps), (1, Layer::CapsShift)] {
            wch[column] = match self.classify(grid.get(layer, position), position, layer) {
                Cell::None => WCH_NONE,
                Cell::Char(unit) => unit,
                Cell::Dead(unit) => {
                    self.diag.warn(format!(
                        "key {name} in layer {layer}: a dead key cannot be expressed in an SGCAPS row; emitted as a plain character"
                    ));
                    unit
                }
                Cell::Ligature(units) => {
                    self.diag.warn(format!(
                        "key {name} in layer {layer}: a ligature ({} units) cannot be expressed in an SGCAPS row; treated as no key",
                        units.len()
                    ));
                    WCH_NONE
                }
            };
        }
        self.rows.push(Row {
            vk,
            attributes: 0,
            wch,
        });
    }

    /// The space and decimal rows that close the table.
    // [spec:kbdgen:req:kbdl.vk-chars]
    fn fixed_rows(&mut self, decimal: u16) {
        let mut space = vec![WCH_NONE; self.columns];
        space[..3].fill(0x0020);
        self.rows.push(Row {
            vk: VK_SPACE,
            attributes: 0,
            wch: space,
        });
        let mut decimal_row = vec![WCH_NONE; self.columns];
        decimal_row[..2].fill(decimal);
        self.rows.push(Row {
            vk: VK_DECIMAL,
            attributes: 0,
            wch: decimal_row,
        });
    }
}

/// A row's `Attributes` from its `default`, `shift`, `caps`, `alt`,
/// `alt+shift` and `alt+caps` values.
// [spec:kbdgen:sem:kbdl.caps]
fn attributes(grid: &Grid, position: usize) -> u8 {
    let default = grid.get(Layer::Default, position);
    let shift = grid.get(Layer::Shift, position);
    let caps = grid.get(Layer::Caps, position);
    if caps.is_some() && caps != default && caps != shift {
        SGCAPS
    } else {
        caps_attributes(grid, position, caps.is_some())
    }
}

/// `CAPLOK` and `CAPLOKALTGR` for a row that is not SGCAPS, computed as if
/// `caps` were absent when `with_caps` is false.
// [spec:kbdgen:sem:kbdl.caps]
fn caps_attributes(grid: &Grid, position: usize, with_caps: bool) -> u8 {
    let default = grid.get(Layer::Default, position);
    let shift = grid.get(Layer::Shift, position);
    let alt = grid.get(Layer::Alt, position);
    let alt_shift = grid.get(Layer::AltShift, position);
    let mut attributes = 0;
    if with_caps {
        if grid.get(Layer::Caps, position) == shift {
            attributes |= CAPLOK;
        }
        if grid.get(Layer::AltCaps, position) == alt_shift {
            attributes |= CAPLOKALTGR;
        }
    } else {
        if default != shift {
            attributes |= CAPLOK;
        }
        if alt != alt_shift {
            attributes |= CAPLOKALTGR;
        }
    }
    attributes
}

/// The decimal key's character.
// [spec:kbdgen:req:kbdl.vk-chars.decimal]
fn decimal_char(decimal: Option<&str>, layout: &str, diag: &mut Diagnostics) -> Result<u16> {
    let Some(decimal) = decimal else {
        return Ok(u16::from(b'.'));
    };
    let mut chars = decimal.chars();
    let Some(first) = chars.next() else {
        bail!("{layout}: the decimal separator is empty");
    };
    let Ok(unit) = u16::try_from(u32::from(first)) else {
        bail!(
            "{layout}: the decimal separator {} lies outside the Basic Multilingual Plane",
            show(decimal)
        );
    };
    if is_special(unit) {
        bail!(
            "{layout}: the decimal separator {} is a unit in 0xF000-0xF002",
            show(decimal)
        );
    }
    if chars.next().is_some() {
        diag.warn(format!(
            "decimal separator {} has more than one character; only the first is used",
            show(decimal)
        ));
    }
    Ok(unit)
}

/// Every unit the tables emit as output, which a fallback dead id must not
/// collide with: cells, ligatures, the decimal character and all strings
/// of the dead-key tree.
fn emitted_units(
    rows: &[Row],
    ligatures: &[LigatureEntry],
    decimal: u16,
    tree: &IndexMap<String, DeadKeyNode>,
) -> BTreeSet<u16> {
    fn walk(node: &DeadKeyNode, set: &mut BTreeSet<u16>) {
        match node {
            DeadKeyNode::Leaf(output) => set.extend(output.encode_utf16()),
            DeadKeyNode::Branch(children) => {
                for (input, child) in children {
                    set.extend(input.encode_utf16());
                    walk(child, set);
                }
            }
        }
    }
    let mut set = BTreeSet::new();
    for row in rows {
        set.extend(row.wch.iter().copied().filter(|unit| !is_special(*unit)));
    }
    for ligature in ligatures {
        set.extend(ligature.units.iter().copied());
    }
    set.insert(decimal);
    for fixed in FIXED_VK_TO_WCH3.iter().map(|(_, wch)| &wch[..]) {
        set.extend(fixed.iter().copied());
    }
    for fixed in FIXED_VK_TO_WCH2.iter().map(|(_, wch)| &wch[..]) {
        set.extend(fixed.iter().copied());
    }
    for fixed in FIXED_VK_TO_WCH1.iter().map(|(_, wch)| &wch[..]) {
        set.extend(fixed.iter().copied());
    }
    for (input, node) in tree {
        set.extend(input.encode_utf16());
        walk(node, &mut set);
    }
    set
}

/// The dead states of a layout and their `aDeadKey` entries.
struct DeadKeys {
    /// Key dead character to its dead id.
    ids: HashMap<u16, u16>,
    entries: Vec<DeadKeyEntry>,
}

/// A dead state: one group of `aDeadKey`.
struct DeadState<'t> {
    id: u16,
    path: String,
    children: &'t IndexMap<String, DeadKeyNode>,
}

/// One child of a dead state that survives validation.
enum Child {
    Leaf { base: u16, composed: u16 },
    Chain { base: u16, state: usize },
}

/// Allocates unique dead ids.
struct DeadIds<'e> {
    taken: BTreeSet<u16>,
    emitted: &'e BTreeSet<u16>,
}

impl DeadIds<'_> {
    /// A branch's standalone output, when it is one usable unit.
    fn standalone_unit(children: &IndexMap<String, DeadKeyNode>) -> Option<u16> {
        match children.get(STANDALONE) {
            Some(DeadKeyNode::Leaf(output)) => match units(output)[..] {
                [unit] if !is_special(unit) => Some(unit),
                _ => None,
            },
            _ => None,
        }
    }

    fn is_free(&self, unit: u16) -> bool {
        !is_special(unit) && !self.taken.contains(&unit)
    }

    /// The highest private-use unit that is neither a dead id nor emitted.
    fn fallback(&self) -> Option<u16> {
        (DEAD_ID_FALLBACK_LOW..=DEAD_ID_FALLBACK_HIGH)
            .rev()
            .find(|unit| self.is_free(*unit) && !self.emitted.contains(unit))
    }

    fn take(&mut self, unit: u16) -> u16 {
        self.taken.insert(unit);
        unit
    }
}

/// Builds `aDeadKey`: one group per dead state, key dead characters first,
/// then chained states depth-first.
// [spec:kbdgen:req:kbdl.dead-keys.table]
fn dead_keys(
    key_dead_chars: &[u16],
    tree: &IndexMap<String, DeadKeyNode>,
    emitted: &BTreeSet<u16>,
    layout: &str,
    diag: &mut Diagnostics,
) -> Result<DeadKeys> {
    let mut ids = DeadIds {
        taken: BTreeSet::new(),
        emitted,
    };

    let mut states: Vec<DeadState> = Vec::new();
    let mut key_ids = HashMap::new();
    for &dead_char in key_dead_chars {
        let text = String::from_utf16_lossy(&[dead_char]);
        let children = match tree.get(&text) {
            Some(DeadKeyNode::Branch(children)) => children,
            Some(DeadKeyNode::Leaf(_)) => bail!(
                "{layout}: dead key {} (U+{dead_char:04X}) has a plain output instead of a set of compositions",
                show(&text)
            ),
            None => bail!(
                "{layout}: dead key {} (U+{dead_char:04X}) has no dead-key entry",
                show(&text)
            ),
        };
        check_branch(children, &text, layout)?;
        let id = key_dead_id(dead_char, children, &mut ids, &text, layout, diag)?;
        key_ids.insert(dead_char, id);
        states.push(DeadState {
            id,
            path: text,
            children,
        });
    }

    let mut groups: Vec<Vec<Child>> = Vec::new();
    let key_states = states.len();
    for state in 0..key_states {
        visit(state, &mut states, &mut groups, &mut ids, layout, diag)?;
    }

    let mut entries = Vec::new();
    for (index, children) in groups.iter().enumerate() {
        let id = states[index].id;
        for child in children {
            entries.push(match child {
                Child::Leaf { base, composed } => DeadKeyEntry {
                    base: *base,
                    id,
                    composed: *composed,
                    flags: 0,
                },
                Child::Chain { base, state } => DeadKeyEntry {
                    base: *base,
                    id,
                    composed: states[*state].id,
                    flags: DKF_DEAD,
                },
            });
        }
    }
    Ok(DeadKeys {
        ids: key_ids,
        entries,
    })
}

/// A key dead character's id: its standalone output when that is one unit
/// outside `0xF000`-`0xF002` and not yet another state's id, so Windows'
/// fallback for an unmatched key emits the standalone output; otherwise the
/// dead character itself, or, if that too is taken, a private-use unit.
// [spec:kbdgen:req:kbdl.dead-keys+2]
fn key_dead_id(
    dead_char: u16,
    children: &IndexMap<String, DeadKeyNode>,
    ids: &mut DeadIds,
    path: &str,
    layout: &str,
    diag: &mut Diagnostics,
) -> Result<u16> {
    if let Some(unit) = DeadIds::standalone_unit(children)
        && ids.is_free(unit)
    {
        return Ok(ids.take(unit));
    }
    if ids.is_free(dead_char) {
        diag.warn(format!(
            "dead key {}: its standalone output cannot serve as its dead id; an unmatched key emits the dead character itself instead",
            show(path)
        ));
        return Ok(ids.take(dead_char));
    }
    let Some(unit) = ids.fallback() else {
        bail!(
            "{layout}: dead key {}: no unit is left for its dead id",
            show(path)
        );
    };
    diag.warn(format!(
        "dead key {}: its standalone output and its character are already dead ids; using U+{unit:04X}, which an unmatched key emits",
        show(path)
    ));
    Ok(ids.take(unit))
}

/// Fatal shape errors of a reachable branch, at any depth.
// [spec:kbdgen:req:kbdl.dead-keys.diagnostics]
fn check_branch(children: &IndexMap<String, DeadKeyNode>, path: &str, layout: &str) -> Result<()> {
    match children.get(STANDALONE) {
        None => bail!(
            "{layout}: dead-key state {} has no standalone (\" \") output",
            show(path)
        ),
        Some(DeadKeyNode::Branch(_)) => bail!(
            "{layout}: dead-key state {}: the standalone (\" \") output is a nested set instead of a string",
            show(path)
        ),
        Some(DeadKeyNode::Leaf(_)) => Ok(()),
    }
}

/// Builds the group of one state from its valid children, in input order
/// with the standalone child last. A branch child is allocated as a chained
/// state and visited before the next sibling, so allocation is depth-first.
// [spec:kbdgen:req:kbdl.dead-keys.diagnostics]
// [spec:kbdgen:req:kbdl.dead-keys.chains]
fn visit<'t>(
    state: usize,
    states: &mut Vec<DeadState<'t>>,
    groups: &mut Vec<Vec<Child>>,
    ids: &mut DeadIds,
    layout: &str,
    diag: &mut Diagnostics,
) -> Result<()> {
    let children = states[state].children;
    let path = states[state].path.clone();
    let ordered = children
        .iter()
        .filter(|(input, _)| input.as_str() != STANDALONE)
        .chain(
            children
                .iter()
                .filter(|(input, _)| input.as_str() == STANDALONE),
        );

    let mut result = Vec::new();
    for (input, node) in ordered {
        let child_path = format!("{path} -> {input}");
        let base = match units(input)[..] {
            [unit] if !is_special(unit) => unit,
            _ => {
                diag.warn(format!(
                    "dead-key input {}: the input must be exactly one UTF-16 unit outside 0xF000-0xF002; omitted",
                    show(&child_path)
                ));
                continue;
            }
        };
        match node {
            DeadKeyNode::Leaf(output) => match units(output)[..] {
                [composed] if !is_special(composed) => result.push(Child::Leaf { base, composed }),
                _ => diag.warn(format!(
                    "dead-key input {} -> output {}: the output must be exactly one UTF-16 unit outside 0xF000-0xF002; omitted",
                    show(&child_path),
                    show(output)
                )),
            },
            DeadKeyNode::Branch(grandchildren) => {
                check_branch(grandchildren, &child_path, layout)?;
                let id = chained_dead_id(grandchildren, ids, &child_path, layout, diag)?;
                states.push(DeadState {
                    id,
                    path: child_path,
                    children: grandchildren,
                });
                let chained = states.len() - 1;
                result.push(Child::Chain {
                    base,
                    state: chained,
                });
                visit(chained, states, groups, ids, layout, diag)?;
            }
        }
    }
    if groups.len() <= state {
        groups.resize_with(state + 1, Vec::new);
    }
    groups[state] = result;
    Ok(())
}

/// A chained state's id: its standalone output when usable and free,
/// otherwise the highest free private-use unit.
// [spec:kbdgen:req:kbdl.dead-keys.chains]
fn chained_dead_id(
    children: &IndexMap<String, DeadKeyNode>,
    ids: &mut DeadIds,
    path: &str,
    layout: &str,
    diag: &mut Diagnostics,
) -> Result<u16> {
    if let Some(unit) = DeadIds::standalone_unit(children)
        && ids.is_free(unit)
    {
        return Ok(ids.take(unit));
    }
    let Some(unit) = ids.fallback() else {
        bail!(
            "{layout}: chained dead-key state {}: no unit is left for its dead id",
            show(path)
        );
    };
    diag.warn(format!(
        "chained dead-key state {}: its standalone output cannot serve as its dead id; using U+{unit:04X}, which an unmatched key emits",
        show(path)
    ));
    Ok(ids.take(unit))
}

/// Replaces the dead characters held by `0xff` rows with their dead ids.
// [spec:kbdgen:req:kbdl.dead-keys+2]
fn substitute_dead_ids(rows: &mut [Row], ids: &HashMap<u16, u16>) {
    for row in rows.iter_mut().filter(|row| row.vk == VK_DEAD_ROW) {
        for unit in row.wch.iter_mut().filter(|unit| **unit != WCH_NONE) {
            if let Some(id) = ids.get(unit) {
                *unit = *id;
            }
        }
    }
}

/// `aKeyNamesDead`: each named key dead character's id and name.
// [spec:kbdgen:req:kbdl.dead-keys.names+1]
fn dead_key_names(
    key_dead_chars: &[u16],
    ids: &HashMap<u16, u16>,
    names: &IndexMap<String, String>,
    diag: &mut Diagnostics,
) -> Vec<Vec<u16>> {
    let mut valid: HashMap<u16, &str> = HashMap::new();
    for (output, name) in names {
        let unit = match units(output)[..] {
            [unit] if key_dead_chars.contains(&unit) => unit,
            _ => {
                diag.warn(format!(
                    "dead-key name {} for {}: no key emits that dead key; ignored",
                    show(name),
                    show(output)
                ));
                continue;
            }
        };
        if name.is_empty() || name.contains('\0') {
            diag.warn(format!(
                "dead-key name {} for {}: the name is empty or contains U+0000; ignored",
                show(name),
                show(output)
            ));
            continue;
        }
        valid.insert(unit, name);
    }
    key_dead_chars
        .iter()
        .filter_map(|dead_char| {
            let name = valid.get(dead_char)?;
            let id = ids.get(dead_char).copied().unwrap_or(*dead_char);
            Some(std::iter::once(id).chain(name.encode_utf16()).collect())
        })
        .collect()
}

/// `aKeyNames` and `aKeyNamesExt` with overrides applied.
// [spec:kbdgen:req:kbdl.key-names]
fn key_names(
    overrides: &IndexMap<KeyNameEntry, String>,
    diag: &mut Diagnostics,
) -> (Vec<KeyName>, Vec<KeyName>) {
    let table = |fixed: &[(u8, &str)]| -> Vec<KeyName> {
        fixed
            .iter()
            .map(|(scan_code, name)| KeyName {
                scan_code: *scan_code,
                name: units(name),
            })
            .collect()
    };
    let mut normal = table(&KEY_NAMES);
    let mut extended = table(&KEY_NAMES_EXT);
    for (entry, name) in overrides {
        let (target, label) = match entry.table {
            KeyNameTable::Normal => (&mut normal, "aKeyNames"),
            KeyNameTable::Extended => (&mut extended, "aKeyNamesExt"),
        };
        let Some(slot) = target
            .iter_mut()
            .find(|key_name| key_name.scan_code == entry.scan_code)
        else {
            diag.warn(format!(
                "key-name override {} for {label} scan code 0x{:02x}: no such entry; ignored",
                show(name),
                entry.scan_code
            ));
            continue;
        };
        if name.is_empty() || name.contains('\0') {
            diag.warn(format!(
                "key-name override {} for {label} scan code 0x{:02x}: the name is empty or contains U+0000; ignored",
                show(name),
                entry.scan_code
            ));
            continue;
        }
        slot.name = units(name);
    }
    (normal, extended)
}

/// `fLocaleFlags`: `KBD_VERSION` in the high word, `KLLF_*` in the low.
// [spec:kbdgen:req:kbdl.locale]
fn locale_flags(rows: &[Row], shift_lock: bool, lrm_rlm: bool) -> u32 {
    let mut flags = KBD_VERSION << 16;
    if rows
        .iter()
        .any(|row| row.wch[3] != WCH_NONE || row.wch[4] != WCH_NONE)
    {
        flags |= KLLF_ALTGR;
    }
    if shift_lock {
        flags |= KLLF_SHIFTLOCK;
    }
    if lrm_rlm {
        flags |= KLLF_LRM_RLM;
    }
    flags
}

/// Checks the structural guarantees Windows relies on when it walks the
/// tables; a failure is a generator defect, never an input problem.
// [spec:kbdgen:thm:kbdl.tables.invariants]
pub fn check_invariants(tables: &Tables) -> Result<()> {
    let columns = tables.columns;
    let rows = &tables.rows;
    let dead_ids: BTreeSet<u16> = tables.dead_keys.iter().map(|entry| entry.id).collect();

    for (index, row) in rows.iter().enumerate() {
        if row.wch.len() != columns {
            bail!(
                "row {index} has {} cells, expected {columns}",
                row.wch.len()
            );
        }
        let has_dead = row.wch.contains(&WCH_DEAD);
        if has_dead {
            let Some(next) = rows.get(index + 1).filter(|next| next.vk == VK_DEAD_ROW) else {
                bail!("row {index} has a dead key but no following 0xff row");
            };
            for (column, cell) in row.wch.iter().enumerate() {
                if *cell == WCH_DEAD && !dead_ids.contains(&next.wch[column]) {
                    bail!(
                        "row {index} column {column}: dead id 0x{:04x} has no aDeadKey group",
                        next.wch[column]
                    );
                }
            }
        }
        if row.vk == VK_DEAD_ROW
            && row
                .wch
                .iter()
                .any(|unit| *unit == WCH_DEAD || *unit == WCH_LGTR)
        {
            bail!("dead row {index} holds a marker");
        }
        for (column, cell) in row.wch.iter().enumerate() {
            if *cell == WCH_LGTR {
                let count = tables
                    .ligatures
                    .iter()
                    .filter(|ligature| {
                        ligature.vk == row.vk && usize::from(ligature.column) == column
                    })
                    .count();
                if count != 1 {
                    bail!("row {index} column {column}: {count} ligature entries, expected 1");
                }
            }
        }
    }
    for ligature in &tables.ligatures {
        let has_cell = rows.iter().any(|row| {
            row.vk == ligature.vk && row.wch.get(usize::from(ligature.column)) == Some(&WCH_LGTR)
        });
        if !has_cell {
            bail!(
                "ligature entry for vk 0x{:02x} column {} has no WCH_LGTR cell",
                ligature.vk,
                ligature.column
            );
        }
        if ligature.units.iter().any(|unit| is_special(*unit)) {
            bail!("ligature for vk 0x{:02x} holds a marker unit", ligature.vk);
        }
    }
    let lg_max = tables.lg_max();
    if !tables.ligatures.is_empty() && !(2..=MAX_LIGATURE_UNITS).contains(&lg_max) {
        bail!("nLgMax {lg_max} is outside 2..=16");
    }

    let mut groups: Vec<u16> = Vec::new();
    let mut seen = BTreeSet::new();
    for entry in &tables.dead_keys {
        if !seen.insert(entry.both()) {
            bail!("two aDeadKey entries share dwBoth 0x{:08x}", entry.both());
        }
        if groups.last() != Some(&entry.id) {
            if groups.contains(&entry.id) {
                bail!("dead id 0x{:04x} has more than one group", entry.id);
            }
            groups.push(entry.id);
        }
        if is_special(entry.composed) || is_special(entry.id) || is_special(entry.base) {
            bail!("aDeadKey entry 0x{:08x} holds a marker unit", entry.both());
        }
        if entry.flags & DKF_DEAD != 0 && !dead_ids.contains(&entry.composed) {
            bail!(
                "chained dead id 0x{:04x} has no aDeadKey group",
                entry.composed
            );
        }
    }

    if tables
        .mod_numbers
        .iter()
        .any(|number| *number != SHFT_INVALID && usize::from(*number) >= columns)
    {
        bail!("a ModNumber entry selects a column beyond {columns}");
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::build::windows::kbdl::input::Metadata;

    pub(crate) fn base() -> LayoutInput {
        LayoutInput {
            metadata: Metadata {
                name: "kbdtest".into(),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    pub(crate) fn index(position: &str) -> usize {
        POSITION_NAMES
            .iter()
            .position(|name| *name == position)
            .unwrap()
    }

    pub(crate) fn set(input: &mut LayoutInput, layer: Layer, position: &str, value: KeyValue) {
        input
            .layers
            .entry(layer)
            .or_insert_with(|| vec![None; POSITION_COUNT])[index(position)] = Some(value);
    }

    pub(crate) fn leaf(output: &str) -> DeadKeyNode {
        DeadKeyNode::Leaf(output.into())
    }

    pub(crate) fn branch(children: &[(&str, DeadKeyNode)]) -> DeadKeyNode {
        DeadKeyNode::Branch(
            children
                .iter()
                .map(|(input, node)| (input.to_string(), node.clone()))
                .collect(),
        )
    }

    pub(crate) fn run(input: &LayoutInput) -> (Result<Tables>, Diagnostics) {
        let mut diag = Diagnostics::new(input.metadata.name.clone());
        let tables = build(input, &mut diag);
        (tables, diag)
    }

    fn ok(input: &LayoutInput) -> (Tables, Diagnostics) {
        let (tables, diag) = run(input);
        (tables.unwrap(), diag)
    }

    fn err(input: &LayoutInput) -> String {
        let (tables, _) = run(input);
        format!("{:#}", tables.unwrap_err())
    }

    fn rows_of(tables: &Tables, vk: u8) -> Vec<&Row> {
        let start = tables.rows.iter().position(|row| row.vk == vk).unwrap();
        let mut rows = vec![&tables.rows[start]];
        if let Some(next) = tables.rows.get(start + 1)
            && (next.vk == vk || next.vk == VK_DEAD_ROW)
        {
            rows.push(next);
        }
        rows
    }

    fn has_row(tables: &Tables, vk: u8) -> bool {
        tables.rows.iter().any(|row| row.vk == vk)
    }

    fn entries(tables: &Tables) -> Vec<(u16, u16, u16, u16)> {
        tables
            .dead_keys
            .iter()
            .map(|entry| (entry.base, entry.id, entry.composed, entry.flags))
            .collect()
    }

    fn warned(diag: &Diagnostics, needles: &[&str]) -> bool {
        diag.warnings()
            .iter()
            .any(|warning| needles.iter().all(|needle| warning.contains(needle)))
    }

    fn letters(input: &mut LayoutInput) {
        set(input, Layer::Default, "C01", KeyValue::new("a"));
        set(input, Layer::Shift, "C01", KeyValue::new("A"));
        set(input, Layer::Default, "D01", KeyValue::new("q"));
        set(input, Layer::Shift, "D01", KeyValue::new("Q"));
        set(input, Layer::Default, "B01", KeyValue::new("z"));
        set(input, Layer::Shift, "B01", KeyValue::new("Z"));
    }

    // [spec:kbdgen:req:kbdl.scancodes.iso/test]
    // [spec:kbdgen:def:kbdl.scancodes/test]
    #[test]
    fn iso_positions_map_to_the_scancode_table() {
        for (position, (scan_code, vk)) in POSITION_KEYS.iter().enumerate() {
            assert_eq!(
                AUS_VK[usize::from(*scan_code)] & 0xff,
                u16::from(*vk),
                "{}",
                POSITION_NAMES[position]
            );
        }
        assert_eq!(POSITION_KEYS[B11], (0x73, 0xc1));
        assert_eq!(POSITION_KEYS[B00], (0x56, 0xe2));
        assert_eq!(AUS_VK[0x36], 0x1a1);
        assert_eq!(AUS_VK[0x7e], 0x0c2);
        assert_eq!(E0_VSC_TO_VK.len(), 38);
    }

    // [spec:kbdgen:req:kbdl.modifiers/test]
    // [spec:kbdgen:def:kbdl.layers/test]
    #[test]
    fn modifier_bindings_follow_the_extra_modifier_count() {
        let (bits, max, numbers) = modifiers(0);
        assert_eq!(bits, vec![(0x10, 1), (0x11, 2), (0x12, 4)]);
        assert_eq!(max, 7);
        assert_eq!(numbers, vec![0, 1, 2, 0x0f, 0x0f, 0x0f, 3, 4]);

        let (bits, max, numbers) = modifiers(1);
        assert_eq!(bits[3], (0xdf, 0x08));
        assert_eq!(max, 9);
        assert_eq!(numbers, vec![0, 1, 2, 0x0f, 0x0f, 0x0f, 3, 4, 5, 6]);

        let (bits, max, numbers) = modifiers(3);
        assert_eq!(&bits[3..], &[(0xdf, 0x08), (0xe8, 0x10), (0x97, 0x20)]);
        assert_eq!(max, 0x21);
        assert_eq!(numbers.len(), 0x22);
        for (bits, column) in [
            (0, 0),
            (1, 1),
            (2, 2),
            (6, 3),
            (7, 4),
            (0x08, 5),
            (0x09, 6),
            (0x10, 7),
            (0x11, 8),
            (0x20, 9),
            (0x21, 10),
        ] {
            assert_eq!(numbers[bits], column, "bits {bits:#x}");
        }
        let valid = numbers
            .iter()
            .filter(|number| **number != SHFT_INVALID)
            .count();
        assert_eq!(valid, 11);
    }

    // [spec:kbdgen:req:kbdl.scancodes.extra-modifiers/test]
    // [spec:kbdgen:def:kbdl.layers/test]
    #[test]
    fn extra_modifiers_add_columns_and_rebind_their_keys() {
        let mut input = base();
        letters(&mut input);
        input.extra_modifiers = vec![
            ExtraModifierKey::RightCtrl,
            ExtraModifierKey::CapsLock,
            ExtraModifierKey::B00,
        ];
        for (i, (plain, shifted)) in [("α", "Α"), ("β", "Β"), ("γ", "Γ")].iter().enumerate() {
            set(
                &mut input,
                Layer::Extra(i as u8),
                "C01",
                KeyValue::new(*plain),
            );
            set(
                &mut input,
                Layer::ExtraShift(i as u8),
                "C01",
                KeyValue::new(*shifted),
            );
        }
        set(&mut input, Layer::Default, "B00", KeyValue::new("<"));
        let (tables, diag) = ok(&input);
        assert_eq!(tables.columns, 11);
        let a = rows_of(&tables, b'A')[0];
        assert_eq!(
            a.wch,
            vec![
                0x61, 0x41, WCH_NONE, WCH_NONE, WCH_NONE, 0x3b1, 0x391, 0x3b2, 0x392, 0x3b3, 0x393
            ]
        );
        assert_eq!(tables.aus_vk[0x3a], 0xe8);
        assert_eq!(tables.aus_vk[0x56], 0x97);
        assert!(tables.e0_vsc_to_vk.contains(&(0x1d, 0x1df)));
        assert!(!has_row(&tables, 0xe2));
        assert!(warned(&diag, &["B00", "default", "ignored"]));
        assert!(tables.rows.iter().all(|row| row.wch.len() == 11));

        let mut one = base();
        letters(&mut one);
        one.extra_modifiers = vec![ExtraModifierKey::B00];
        set(&mut one, Layer::Extra(0), "C01", KeyValue::new("α"));
        set(&mut one, Layer::Extra(1), "C01", KeyValue::new("β"));
        let (tables, diag) = ok(&one);
        assert_eq!(tables.columns, 7);
        assert_eq!(tables.aus_vk[0x56], 0xdf);
        assert_eq!(tables.aus_vk[0x3a], 0x014);
        assert!(tables.e0_vsc_to_vk.contains(&(0x1d, 0x1a3)));
        assert!(warned(&diag, &["extra1", "ignored"]));
    }

    // [spec:kbdgen:def:kbdl.input/test]
    #[test]
    fn malformed_extra_modifiers_are_fatal() {
        let mut input = base();
        input.extra_modifiers = vec![ExtraModifierKey::B00; 4];
        assert!(err(&input).contains("at most 3"));
        input.extra_modifiers = vec![ExtraModifierKey::CapsLock, ExtraModifierKey::CapsLock];
        assert!(err(&input).contains("capsLock"));
        let mut input = base();
        input.layers.insert(Layer::Default, vec![None; 3]);
        assert!(err(&input).contains("3 values"));
    }

    // [spec:kbdgen:sem:kbdl.vk-chars.values/test]
    // [spec:kbdgen:thm:kbdl.ligatures.limit/test]
    #[test]
    fn values_become_characters_dead_keys_ligatures_or_no_key() {
        let sixteen = "abcdefghijklmnop";
        let mut input = base();
        set(&mut input, Layer::Default, "C01", KeyValue::new("a"));
        set(&mut input, Layer::Shift, "C01", KeyValue::dead("ab"));
        set(&mut input, Layer::Ctrl, "C01", KeyValue::new("\u{1F600}"));
        set(&mut input, Layer::Alt, "C01", KeyValue::new(sixteen));
        set(
            &mut input,
            Layer::AltShift,
            "C01",
            KeyValue::new(format!("{sixteen}q")),
        );
        let (tables, diag) = ok(&input);
        let a = rows_of(&tables, b'A')[0];
        assert_eq!(a.wch, vec![0x61, WCH_LGTR, WCH_LGTR, WCH_LGTR, WCH_NONE]);
        assert_eq!(tables.ligatures.len(), 3);
        assert_eq!(tables.ligatures[1].units, vec![0xd83d, 0xde00]);
        assert_eq!(tables.lg_max(), 16);
        assert!(warned(&diag, &["C01", "shift", "dead-key flag is ignored"]));
        assert!(warned(&diag, &["C01", "alt+shift", "17 UTF-16 units"]));

        let mut input = base();
        set(&mut input, Layer::Alt, "D05", KeyValue::new("x\u{F001}"));
        let error = err(&input);
        assert!(error.contains("D05") && error.contains("alt"), "{error}");
        let mut input = base();
        set(&mut input, Layer::Caps, "D05", KeyValue::new("\u{F000}"));
        assert!(err(&input).contains("caps"));
    }

    // [spec:kbdgen:req:kbdl.vk-chars/test]
    // [spec:kbdgen:def:kbdl.layers/test]
    #[test]
    fn character_table_order_and_49th_key() {
        let mut input = base();
        letters(&mut input);
        let (tables, _) = ok(&input);
        assert_eq!(tables.rows.len(), 48 + 2);
        assert_eq!(tables.rows[0].vk, 0xc0);
        assert!(!has_row(&tables, 0xc1));
        assert_eq!(tables.rows[37].vk, 0xe2);
        let space = &tables.rows[48];
        assert_eq!(
            (space.vk, space.attributes, space.wch.clone()),
            (0x20, 0, vec![0x20, 0x20, 0x20, WCH_NONE, WCH_NONE])
        );
        let decimal = &tables.rows[49];
        assert_eq!(
            (decimal.vk, decimal.attributes, decimal.wch.clone()),
            (0x6e, 0, vec![0x2e, 0x2e, WCH_NONE, WCH_NONE, WCH_NONE])
        );
        assert_eq!(
            tables.rows[0].wch,
            vec![WCH_NONE; 5],
            "empty positions are still emitted"
        );

        set(&mut input, Layer::Shift, "B11", KeyValue::new("?"));
        let (tables, _) = ok(&input);
        assert_eq!(tables.rows.len(), 49 + 2);
        let c1 = rows_of(&tables, 0xc1)[0];
        assert_eq!(c1.wch, vec![WCH_NONE, 0x3f, WCH_NONE, WCH_NONE, WCH_NONE]);
        assert_eq!(tables.rows[48].vk, 0xc1);

        let mut caps_only = base();
        set(&mut caps_only, Layer::Caps, "B11", KeyValue::new("/"));
        let (tables, _) = ok(&caps_only);
        assert!(has_row(&tables, 0xc1));
    }

    // [spec:kbdgen:req:kbdl.vk-chars.decimal/test]
    #[test]
    fn decimal_separator_rules() {
        let mut diag = Diagnostics::new("kbdtest");
        assert_eq!(decimal_char(None, "kbdtest", &mut diag).unwrap(), 0x2e);
        assert_eq!(decimal_char(Some(","), "kbdtest", &mut diag).unwrap(), 0x2c);
        assert!(diag.warnings().is_empty());
        assert_eq!(
            decimal_char(Some(",."), "kbdtest", &mut diag).unwrap(),
            0x2c
        );
        assert_eq!(diag.warnings().len(), 1);
        assert!(decimal_char(Some(""), "kbdtest", &mut diag).is_err());
        assert!(decimal_char(Some("\u{1D7CE}"), "kbdtest", &mut diag).is_err());
        assert!(decimal_char(Some("\u{F002}"), "kbdtest", &mut diag).is_err());

        let mut input = base();
        input.decimal = Some("٫".into());
        let (tables, _) = ok(&input);
        assert_eq!(tables.rows.last().unwrap().wch[..2], [0x066b, 0x066b]);
    }

    // [spec:kbdgen:sem:kbdl.caps/test]
    #[test]
    fn caps_attributes() {
        let mut input = base();
        letters(&mut input);
        set(&mut input, Layer::Default, "E01", KeyValue::new("1"));
        set(&mut input, Layer::Shift, "E01", KeyValue::new("!"));
        set(&mut input, Layer::Alt, "D02", KeyValue::new("š"));
        set(&mut input, Layer::AltShift, "D02", KeyValue::new("Š"));
        set(&mut input, Layer::Default, "D02", KeyValue::new("w"));
        set(&mut input, Layer::Shift, "D02", KeyValue::new("W"));
        let (tables, _) = ok(&input);
        assert_eq!(rows_of(&tables, b'A')[0].attributes, CAPLOK);
        assert_eq!(rows_of(&tables, b'1')[0].attributes, CAPLOK);
        assert_eq!(rows_of(&tables, b'W')[0].attributes, CAPLOK | CAPLOKALTGR);
        assert_eq!(rows_of(&tables, 0xc0)[0].attributes, 0);

        set(&mut input, Layer::Caps, "C01", KeyValue::new("A"));
        set(&mut input, Layer::Caps, "E01", KeyValue::new("1"));
        set(&mut input, Layer::Caps, "D02", KeyValue::new("W"));
        set(&mut input, Layer::AltCaps, "D02", KeyValue::new("Š"));
        let (tables, _) = ok(&input);
        assert_eq!(
            rows_of(&tables, b'A')[0].attributes,
            CAPLOK | CAPLOKALTGR,
            "absent alt+caps equals absent alt+shift"
        );
        assert_eq!(rows_of(&tables, b'1')[0].attributes, CAPLOKALTGR);
        assert_eq!(rows_of(&tables, b'W')[0].attributes, CAPLOK | CAPLOKALTGR);

        set(&mut input, Layer::AltCaps, "D02", KeyValue::new("š"));
        let (tables, _) = ok(&input);
        assert_eq!(rows_of(&tables, b'W')[0].attributes, CAPLOK);

        let mut dead = base();
        set(&mut dead, Layer::Default, "C01", KeyValue::dead("´"));
        set(&mut dead, Layer::Shift, "C01", KeyValue::new("´"));
        dead.dead_key_tree
            .insert("´".into(), branch(&[(" ", leaf("´"))]));
        let (tables, _) = ok(&dead);
        assert_eq!(rows_of(&tables, b'A')[0].attributes, CAPLOK);
    }

    // [spec:kbdgen:req:kbdl.caps.sgcaps/test]
    #[test]
    fn sgcaps_rows_and_their_degradations() {
        let mut input = base();
        set(&mut input, Layer::Default, "C01", KeyValue::new("a"));
        set(&mut input, Layer::Shift, "C01", KeyValue::new("A"));
        set(&mut input, Layer::Alt, "C01", KeyValue::new("x"));
        set(&mut input, Layer::Caps, "C01", KeyValue::new("ä"));
        set(&mut input, Layer::CapsShift, "C01", KeyValue::new("Ä"));
        set(&mut input, Layer::Default, "C02", KeyValue::new("s"));
        set(&mut input, Layer::Shift, "C02", KeyValue::new("S"));
        set(&mut input, Layer::Caps, "C02", KeyValue::dead("´"));
        set(
            &mut input,
            Layer::CapsShift,
            "C02",
            KeyValue::new("s\u{301}"),
        );
        let (tables, diag) = ok(&input);
        let a = rows_of(&tables, b'A');
        assert_eq!(a[0].attributes, SGCAPS);
        assert_eq!(a[0].wch, vec![0x61, 0x41, WCH_NONE, 0x78, WCH_NONE]);
        assert_eq!(
            (a[1].vk, a[1].attributes, a[1].wch.clone()),
            (b'A', 0, vec![0xe4, 0xc4, WCH_NONE, WCH_NONE, WCH_NONE])
        );
        let s = rows_of(&tables, b'S');
        assert_eq!(s[0].attributes, SGCAPS);
        assert_eq!(s[1].wch, vec![0xb4, WCH_NONE, WCH_NONE, WCH_NONE, WCH_NONE]);
        assert!(warned(&diag, &["C02", "caps", "plain character"]));
        assert!(warned(&diag, &["C02", "caps+shift", "ligature"]));
        assert!(tables.ligatures.is_empty());
        assert!(tables.dead_keys.is_empty());

        let mut dead = base();
        set(&mut dead, Layer::Default, "C01", KeyValue::dead("´"));
        set(&mut dead, Layer::Shift, "C01", KeyValue::new("A"));
        set(&mut dead, Layer::Caps, "C01", KeyValue::new("ä"));
        dead.dead_key_tree
            .insert("´".into(), branch(&[(" ", leaf("´"))]));
        let (tables, diag) = ok(&dead);
        let a = rows_of(&tables, b'A');
        assert_eq!(a[0].attributes, CAPLOK);
        assert_eq!(a[1].vk, VK_DEAD_ROW);
        assert!(warned(&diag, &["C01", "SGCAPS cannot be combined"]));
    }

    /// Two dead keys in the layout of Võro: `´` on E12 in `default` and
    /// `alt`, `ˇ` on E00, each with its own standalone output.
    fn vro_like() -> LayoutInput {
        let mut input = base();
        letters(&mut input);
        set(&mut input, Layer::Default, "E00", KeyValue::dead("ˇ"));
        set(&mut input, Layer::Shift, "E00", KeyValue::dead("~"));
        set(&mut input, Layer::Default, "E12", KeyValue::dead("´"));
        set(&mut input, Layer::Shift, "E12", KeyValue::dead("`"));
        set(&mut input, Layer::Alt, "E12", KeyValue::dead("´"));
        set(&mut input, Layer::Default, "B03", KeyValue::new("c"));
        input.dead_key_tree.insert(
            "´".into(),
            branch(&[
                (" ", leaf("´")),
                ("a", leaf("á")),
                ("b", leaf("b\u{301}")),
                ("z", leaf("ź")),
            ]),
        );
        input
            .dead_key_tree
            .insert("ˇ".into(), branch(&[("c", leaf("č")), (" ", leaf("ˇ"))]));
        input
            .dead_key_tree
            .insert("~".into(), branch(&[("a", leaf("ą")), (" ", leaf("~"))]));
        input
            .dead_key_tree
            .insert("`".into(), branch(&[("a", leaf("à")), (" ", leaf("`"))]));
        input
            .dead_key_tree
            .insert("¨".into(), branch(&[("a", leaf("ä"))]));
        input
    }

    // [spec:kbdgen:req:kbdl.dead-keys+2/test]
    // [spec:kbdgen:req:kbdl.dead-keys.table/test]
    #[test]
    fn dead_rows_and_one_group_per_dead_state() {
        let (tables, diag) = ok(&vro_like());
        let e00 = rows_of(&tables, 0xc0);
        assert_eq!(e00[0].wch[..2], [WCH_DEAD, WCH_DEAD]);
        assert_eq!(
            (e00[1].vk, e00[1].attributes, e00[1].wch.clone()),
            (0xff, 0, vec![0x2c7, 0x7e, WCH_NONE, WCH_NONE, WCH_NONE])
        );
        let e12 = rows_of(&tables, 0xbb);
        assert_eq!(
            e12[0].wch,
            vec![WCH_DEAD, WCH_DEAD, WCH_NONE, WCH_DEAD, WCH_NONE]
        );
        assert_eq!(e12[1].wch, vec![0xb4, 0x60, WCH_NONE, 0xb4, WCH_NONE]);

        let groups: Vec<u16> =
            tables
                .dead_keys
                .iter()
                .map(|entry| entry.id)
                .fold(Vec::new(), |mut ids, id| {
                    if ids.last() != Some(&id) {
                        ids.push(id);
                    }
                    ids
                });
        assert_eq!(groups, vec![0x2c7, 0x7e, 0xb4, 0x60]);
        assert_eq!(
            entries(&tables)
                .into_iter()
                .filter(|entry| entry.1 == 0xb4)
                .collect::<Vec<_>>(),
            vec![
                (0x61, 0xb4, 0xe1, 0),
                (0x7a, 0xb4, 0x17a, 0),
                (0x20, 0xb4, 0xb4, 0)
            ],
            "´ has one group, with the standalone entry last"
        );
        assert_eq!(
            tables.dead_keys[0].both(),
            0x63 | 0x2c7 << 16,
            "dwBoth is base | id << 16"
        );
        assert!(warned(&diag, &["´ -> b", "omitted"]));
        assert!(!diag.warnings().iter().any(|warning| warning.contains('¨')));
    }

    // [spec:kbdgen:req:kbdl.dead-keys+2/test]
    #[test]
    fn dead_ids_are_standalone_outputs_with_fallbacks() {
        let (tables, diag) = ok(&vro_like());
        assert!(entries(&tables).contains(&(0x20, 0xb4, 0xb4, 0)));
        assert!(entries(&tables).contains(&(0x20, 0x2c7, 0x2c7, 0)));
        assert!(!warned(&diag, &["dead id"]));

        let mut input = vro_like();
        input
            .dead_key_tree
            .insert("´".into(), branch(&[("a", leaf("á")), (" ", leaf("'"))]));
        let (tables, diag) = ok(&input);
        let e12 = rows_of(&tables, 0xbb);
        assert_eq!(e12[1].wch, vec![0x27, 0x60, WCH_NONE, 0x27, WCH_NONE]);
        assert!(entries(&tables).contains(&(0x61, 0x27, 0xe1, 0)));
        assert!(entries(&tables).contains(&(0x20, 0x27, 0x27, 0)));
        assert!(
            diag.warnings()
                .iter()
                .all(|warning| !warning.contains("dead id"))
        );

        input
            .dead_key_tree
            .insert("`".into(), branch(&[("a", leaf("à")), (" ", leaf("'"))]));
        let (tables, diag) = ok(&input);
        assert_eq!(rows_of(&tables, 0xbb)[1].wch[..2], [0x27, 0x60]);
        assert!(entries(&tables).contains(&(0x20, 0x60, 0x27, 0)));
        assert!(warned(&diag, &["dead key \"`\"", "dead character itself"]));

        input
            .dead_key_tree
            .insert("´".into(), branch(&[("a", leaf("á")), (" ", leaf("`"))]));
        input
            .dead_key_tree
            .insert("`".into(), branch(&[("a", leaf("à")), (" ", leaf("`"))]));
        let (tables, diag) = ok(&input);
        assert_eq!(rows_of(&tables, 0xbb)[1].wch[..2], [0x60, 0xf8ff]);
        assert!(warned(&diag, &["dead key \"`\"", "U+F8FF"]));
        check_invariants(&tables).unwrap();
    }

    fn chained() -> LayoutInput {
        let mut input = base();
        letters(&mut input);
        set(&mut input, Layer::Default, "E12", KeyValue::dead("´"));
        set(&mut input, Layer::Default, "D11", KeyValue::dead("^"));
        input.dead_key_tree.insert(
            "´".into(),
            branch(&[
                ("a", leaf("á")),
                (
                    "q",
                    branch(&[
                        ("a", leaf("ǻ")),
                        ("^", branch(&[("a", leaf("ḁ")), (" ", leaf("˙"))])),
                        (" ", leaf("˝")),
                    ]),
                ),
                ("^", branch(&[("a", leaf("ấ")), (" ", leaf("ˆ"))])),
                (" ", leaf("´")),
            ]),
        );
        input
            .dead_key_tree
            .insert("^".into(), branch(&[("a", leaf("â")), (" ", leaf("^"))]));
        input
    }

    // [spec:kbdgen:req:kbdl.dead-keys.chains/test]
    // [spec:kbdgen:req:kbdl.dead-keys.table/test]
    #[test]
    fn chained_states_depth_two_and_three() {
        let (tables, diag) = ok(&chained());
        assert!(diag.warnings().is_empty(), "{:?}", diag.warnings());
        assert_eq!(
            entries(&tables),
            vec![
                (0x61, 0xb4, 0xe1, 0),
                (0x71, 0xb4, 0x2dd, DKF_DEAD),
                (0x5e, 0xb4, 0x2c6, DKF_DEAD),
                (0x20, 0xb4, 0xb4, 0),
                (0x61, 0x5e, 0xe2, 0),
                (0x20, 0x5e, 0x5e, 0),
                (0x61, 0x2dd, 0x1fb, 0),
                (0x5e, 0x2dd, 0x2d9, DKF_DEAD),
                (0x20, 0x2dd, 0x2dd, 0),
                (0x61, 0x2d9, 0x1e01, 0),
                (0x20, 0x2d9, 0x2d9, 0),
                (0x61, 0x2c6, 0x1ea5, 0),
                (0x20, 0x2c6, 0x2c6, 0),
            ]
        );
        check_invariants(&tables).unwrap();
    }

    // [spec:kbdgen:req:kbdl.dead-keys.chains/test]
    #[test]
    fn chained_ids_fall_back_to_private_use() {
        let mut input = chained();
        set(&mut input, Layer::Alt, "C01", KeyValue::new("\u{F8FF}"));
        input.dead_key_tree.insert(
            "´".into(),
            branch(&[
                ("q", branch(&[("a", leaf("ǻ")), (" ", leaf("q\u{301}"))])),
                ("^", branch(&[("a", leaf("ấ")), (" ", leaf("^"))])),
                (" ", leaf("´")),
            ]),
        );
        let (tables, diag) = ok(&input);
        assert!(entries(&tables).contains(&(0x71, 0xb4, 0xf8fe, DKF_DEAD)));
        assert!(entries(&tables).contains(&(0x5e, 0xb4, 0xf8fd, DKF_DEAD)));
        assert!(entries(&tables).contains(&(0x61, 0xf8fe, 0x1fb, 0)));
        assert!(warned(&diag, &["´ -> q", "U+F8FE"]));
        assert!(warned(&diag, &["´ -> ^", "U+F8FD"]));
        check_invariants(&tables).unwrap();
    }

    // [spec:kbdgen:req:kbdl.dead-keys.diagnostics/test]
    #[test]
    fn dead_key_diagnostics_never_panic() {
        let mut input = chained();
        input.dead_key_tree.shift_remove("^");
        assert!(err(&input).contains("no dead-key entry"));

        input.dead_key_tree.insert("^".into(), leaf("^"));
        assert!(err(&input).contains("plain output"));

        let mut input = chained();
        input
            .dead_key_tree
            .insert("^".into(), branch(&[("a", leaf("â"))]));
        assert!(err(&input).contains("no standalone"));

        let mut input = chained();
        input.dead_key_tree.insert(
            "´".into(),
            branch(&[("q", branch(&[("a", leaf("ǻ"))])), (" ", leaf("´"))]),
        );
        let error = err(&input);
        assert!(
            error.contains("´ -> q") && error.contains("no standalone"),
            "{error}"
        );

        let mut input = chained();
        input
            .dead_key_tree
            .insert("´".into(), branch(&[(" ", branch(&[(" ", leaf("´"))]))]));
        assert!(err(&input).contains("nested set"));

        let mut input = chained();
        input.dead_key_tree.insert(
            "´".into(),
            branch(&[
                ("ab", leaf("x")),
                ("", leaf("x")),
                ("\u{F001}", leaf("x")),
                ("b", leaf("b\u{301}")),
                ("c", leaf("")),
                ("d", leaf("\u{F002}")),
                ("e", leaf("é")),
                (" ", leaf("´")),
            ]),
        );
        let (tables, diag) = ok(&input);
        assert_eq!(
            entries(&tables)
                .into_iter()
                .filter(|entry| entry.1 == 0xb4)
                .collect::<Vec<_>>(),
            vec![(0x65, 0xb4, 0xe9, 0), (0x20, 0xb4, 0xb4, 0)]
        );
        assert_eq!(diag.warnings().len(), 6, "{:?}", diag.warnings());
        assert!(warned(&diag, &["´ -> b", "b\\u{301}", "omitted"]));
    }

    // [spec:kbdgen:req:kbdl.dead-keys.names+1/test]
    #[test]
    fn dead_key_names_use_dead_ids() {
        let mut input = vro_like();
        input
            .dead_key_tree
            .insert("´".into(), branch(&[("a", leaf("á")), (" ", leaf("'"))]));
        input.dead_key_names.insert("´".into(), "AKUT".into());
        input.dead_key_names.insert("ˇ".into(), "HÁČEK".into());
        input.dead_key_names.insert("¨".into(), "TREMA".into());
        input.dead_key_names.insert("~".into(), String::new());
        input.dead_key_names.insert("`".into(), "GR\0AVE".into());
        let (tables, diag) = ok(&input);
        let expected: Vec<Vec<u16>> = vec![
            std::iter::once(0x2c7)
                .chain("HÁČEK".encode_utf16())
                .collect(),
            std::iter::once(0x27).chain("AKUT".encode_utf16()).collect(),
        ];
        assert_eq!(tables.dead_key_names, expected);
        assert!(warned(&diag, &["TREMA", "no key emits"]));
        assert!(warned(&diag, &["\"~\"", "empty"]));
        assert!(warned(&diag, &["\"`\"", "U+0000"]));

        let (tables, _) = ok(&vro_like());
        assert!(tables.dead_key_names.is_empty());
    }

    // [spec:kbdgen:req:kbdl.ligatures+1/test]
    #[test]
    fn ligatures_in_emission_order() {
        let mut input = base();
        input.extra_modifiers = vec![ExtraModifierKey::RightCtrl];
        set(&mut input, Layer::Alt, "D05", KeyValue::new("t\u{301}"));
        set(
            &mut input,
            Layer::AltShift,
            "D05",
            KeyValue::new("T\u{301}"),
        );
        set(&mut input, Layer::Shift, "D05", KeyValue::new("TTT"));
        set(&mut input, Layer::Extra(0), "D01", KeyValue::new("ββ"));
        let (tables, _) = ok(&input);
        let ligatures: Vec<(u8, u16, usize)> = tables
            .ligatures
            .iter()
            .map(|ligature| (ligature.vk, ligature.column, ligature.units.len()))
            .collect();
        assert_eq!(
            ligatures,
            vec![(b'Q', 5, 2), (b'T', 1, 3), (b'T', 3, 2), (b'T', 4, 2)]
        );
        assert_eq!(tables.lg_max(), 3);
        check_invariants(&tables).unwrap();

        let (tables, _) = ok(&base());
        assert_eq!(tables.lg_max(), 0);
        assert!(tables.ligatures.is_empty());
    }

    // [spec:kbdgen:req:kbdl.key-names/test]
    #[test]
    fn key_names_and_overrides() {
        let mut input = base();
        let normal = |scan_code| KeyNameEntry {
            table: KeyNameTable::Normal,
            scan_code,
        };
        let extended = |scan_code| KeyNameEntry {
            table: KeyNameTable::Extended,
            scan_code,
        };
        input
            .key_name_overrides
            .insert(normal(0x01), "Échap".into());
        input
            .key_name_overrides
            .insert(extended(0x38), "AltGr".into());
        input.key_name_overrides.insert(normal(0x02), "One".into());
        input
            .key_name_overrides
            .insert(extended(0x47), String::new());
        input
            .key_name_overrides
            .insert(normal(0x0e), "Back\0".into());
        let (tables, diag) = ok(&input);
        assert_eq!(tables.key_names.len(), 51);
        assert_eq!(tables.key_names_ext.len(), 22);
        assert_eq!(tables.key_names[0].name, units("Échap"));
        assert_eq!(tables.key_names[1].name, units("Backspace"));
        let alt = tables
            .key_names_ext
            .iter()
            .find(|name| name.scan_code == 0x38)
            .unwrap();
        assert_eq!(alt.name, units("AltGr"));
        let home = tables
            .key_names_ext
            .iter()
            .find(|name| name.scan_code == 0x47)
            .unwrap();
        assert_eq!(home.name, units("Home"));
        assert!(warned(&diag, &["0x02", "no such entry"]));
        assert!(warned(&diag, &["0x47", "empty"]));
        assert!(warned(&diag, &["0x0e", "U+0000"]));
        assert_eq!(tables.key_names.last().unwrap().scan_code, 0x87);
        assert_eq!(tables.key_names_ext.last().unwrap().scan_code, 0x5d);
    }

    // [spec:kbdgen:req:kbdl.locale/test]
    #[test]
    fn locale_flags_from_altgr_and_input_flags() {
        let mut input = base();
        letters(&mut input);
        assert_eq!(ok(&input).0.locale_flags, 0x0001_0000);
        set(&mut input, Layer::AltShift, "C01", KeyValue::new("Á"));
        assert_eq!(ok(&input).0.locale_flags, 0x0001_0001);
        input.shift_lock = true;
        assert_eq!(ok(&input).0.locale_flags, 0x0001_0003);
        input.lrm_rlm = true;
        assert_eq!(ok(&input).0.locale_flags, 0x0001_0007);
        let mut ctrl_only = base();
        set(&mut ctrl_only, Layer::Ctrl, "C01", KeyValue::new("\u{1}"));
        ctrl_only.lrm_rlm = true;
        assert_eq!(ok(&ctrl_only).0.locale_flags, 0x0001_0004);
    }

    // [spec:kbdgen:thm:kbdl.tables.invariants/test]
    #[test]
    fn invariant_check_rejects_broken_tables() {
        let (tables, _) = ok(&chained());
        check_invariants(&tables).unwrap();

        let mut broken = tables.clone();
        let dead_row = broken
            .rows
            .iter()
            .position(|row| row.vk == VK_DEAD_ROW)
            .unwrap();
        broken.rows.remove(dead_row);
        assert!(check_invariants(&broken).is_err());

        let mut broken = tables.clone();
        broken.dead_keys.push(broken.dead_keys[0]);
        assert!(check_invariants(&broken).is_err());

        let mut broken = tables.clone();
        broken.rows[0].wch[0] = WCH_LGTR;
        assert!(check_invariants(&broken).is_err());

        let mut broken = tables.clone();
        broken.mod_numbers[3] = 7;
        assert!(check_invariants(&broken).is_err());

        let mut broken = tables.clone();
        broken.dead_keys[1].composed = 0x1234;
        assert!(check_invariants(&broken).is_err());

        let mut broken = tables;
        broken.ligatures.push(LigatureEntry {
            vk: b'A',
            column: 0,
            units: vec![0x61; 17],
        });
        assert!(check_invariants(&broken).is_err());
    }
}
