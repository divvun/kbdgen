//! The abstract input of the layout DLL generator: everything the tables,
//! resources and crate are computed from, independent of any bundle format.

use indexmap::IndexMap;

/// Number of ISO positions `E00`..`B11` a layer assigns values to.
pub const POSITION_COUNT: usize = 49;

/// Upper bound on extra modifiers; each adds two character-table columns.
pub const MAX_EXTRA_MODIFIERS: usize = 3;

/// The 49 ISO positions in key order, so that index `i` of a layer's values
/// belongs to `POSITION_NAMES[i]`.
pub const POSITION_NAMES: [&str; POSITION_COUNT] = [
    "E00", "E01", "E02", "E03", "E04", "E05", "E06", "E07", "E08", "E09", "E10", "E11", "E12",
    "D01", "D02", "D03", "D04", "D05", "D06", "D07", "D08", "D09", "D10", "D11", "D12", "C01",
    "C02", "C03", "C04", "C05", "C06", "C07", "C08", "C09", "C10", "C11", "C12", "B00", "B01",
    "B02", "B03", "B04", "B05", "B06", "B07", "B08", "B09", "B10", "B11",
];

/// Index of `B00` in [`POSITION_NAMES`].
pub const B00: usize = 37;
/// Index of `B11` in [`POSITION_NAMES`].
pub const B11: usize = 48;

/// One layout's complete generator input.
// [spec:kbdgen:def:kbdl.input]
#[derive(Debug, Clone, Default)]
pub struct LayoutInput {
    pub metadata: Metadata,
    /// `None` when the layout states no decimal separator.
    pub decimal: Option<String>,
    /// Values per layer; a layer missing from the map is absent from the
    /// input, and every vector holds exactly [`POSITION_COUNT`] entries.
    pub layers: IndexMap<Layer, Vec<Option<KeyValue>>>,
    /// Extra modifiers in order; the *i*-th selects the `Extra(i)` layers.
    pub extra_modifiers: Vec<ExtraModifierKey>,
    pub dead_key_tree: IndexMap<String, DeadKeyNode>,
    pub dead_key_names: IndexMap<String, String>,
    /// Replacement names for entries of the fixed key-name tables.
    pub key_name_overrides: IndexMap<KeyNameEntry, String>,
    pub shift_lock: bool,
    pub lrm_rlm: bool,
}

/// A layer of `kbdl.layers`. `Extra(i)` and `ExtraShift(i)` belong to the
/// *i*-th extra modifier, counted from 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Layer {
    Default,
    Shift,
    Ctrl,
    Alt,
    AltShift,
    Caps,
    CapsShift,
    AltCaps,
    Extra(u8),
    ExtraShift(u8),
}

impl std::fmt::Display for Layer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Layer::Default => f.write_str("default"),
            Layer::Shift => f.write_str("shift"),
            Layer::Ctrl => f.write_str("ctrl"),
            Layer::Alt => f.write_str("alt"),
            Layer::AltShift => f.write_str("alt+shift"),
            Layer::Caps => f.write_str("caps"),
            Layer::CapsShift => f.write_str("caps+shift"),
            Layer::AltCaps => f.write_str("alt+caps"),
            Layer::Extra(i) => write!(f, "extra{i}"),
            Layer::ExtraShift(i) => write!(f, "extra{i}+shift"),
        }
    }
}

/// A key's output in one layer: a non-empty string and whether it is a dead
/// key. Two values are equal when both the text and the flag are.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyValue {
    pub text: String,
    pub dead: bool,
}

impl KeyValue {
    pub fn new(text: impl Into<String>) -> Self {
        KeyValue {
            text: text.into(),
            dead: false,
        }
    }

    pub fn dead(text: impl Into<String>) -> Self {
        KeyValue {
            text: text.into(),
            dead: true,
        }
    }
}

/// The physical key an extra modifier is bound to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExtraModifierKey {
    RightCtrl,
    CapsLock,
    B00,
}

impl std::fmt::Display for ExtraModifierKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            ExtraModifierKey::RightCtrl => "rightCtrl",
            ExtraModifierKey::CapsLock => "capsLock",
            ExtraModifierKey::B00 => "B00",
        })
    }
}

/// A node of the dead-key tree. A branch maps input strings to nodes in
/// input order; its `" "` child is the branch's standalone output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeadKeyNode {
    Leaf(String),
    Branch(IndexMap<String, DeadKeyNode>),
}

/// The input string of a branch's standalone child.
pub const STANDALONE: &str = " ";

/// Which fixed key-name table an entry lives in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyNameTable {
    /// `aKeyNames`, keys without the `E0` prefix.
    Normal,
    /// `aKeyNamesExt`, keys with the `E0` prefix.
    Extended,
}

/// An entry of a fixed key-name table, addressed by table and scan code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KeyNameEntry {
    pub table: KeyNameTable,
    pub scan_code: u8,
}

/// Descriptive data for the crate, DLL and resources.
// [spec:kbdgen:req:kbdl.metadata]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Metadata {
    /// Names the crate directory, the DLL and its resources.
    pub name: String,
    pub description: String,
    pub language_name: String,
    pub locale_name: String,
    pub lcid: u32,
    pub company: String,
    pub copyright: String,
    /// Dot-separated `a.b.c` version, or `None` when unknown.
    pub version: Option<String>,
    /// Decimal build number, or `None` when unknown.
    pub build: Option<String>,
}
