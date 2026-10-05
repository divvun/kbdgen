//! The hardware layer set and the touch layer sets.

use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use crate::keyboard::KeyIndex;
use crate::modifiers::{ModifierSet, Modifiers};

/// A scan code: PC/AT set 1, without the `E0` prefix.
pub type ScanCode = u8;

/// A form: the physical key positions of a hardware keyboard.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Form {
    /// An implied form id (`us`, `iso`, `jis`, `ks`, `abnt2`) or a custom
    /// one.
    pub id: String,
    /// Rows of scan codes. A scan code occurs at most once in a form.
    pub rows: Vec<Vec<ScanCode>>,
}

impl Form {
    /// The (row, column) of `scan_code`, if the form has it.
    pub fn position_of(&self, scan_code: ScanCode) -> Option<(usize, usize)> {
        self.rows
            .iter()
            .enumerate()
            .find_map(|(r, row)| row.iter().position(|&c| c == scan_code).map(|col| (r, col)))
    }
}

/// A layer of the hardware set.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct HardwareLayer {
    pub id: Option<String>,
    /// The modifier sets that select this layer, non-empty, distinct and
    /// sorted (`ldml.model.modifiers`).
    pub modifiers: Vec<ModifierSet>,
    /// Rows of key indices. Row *r* is at most as long as form row *r*.
    pub rows: Vec<Vec<KeyIndex>>,
}

impl HardwareLayer {
    /// Whether the layer is native-only (`ldml.model.native`).
    pub fn is_native(&self) -> bool {
        self.modifiers.iter().any(|s| s.is_native())
    }

    /// The key at (`row`, `col`). A position past a row's end, or in a row
    /// the layer omits, has no key.
    pub fn key_at(&self, row: usize, col: usize) -> Option<KeyIndex> {
        self.rows.get(row)?.get(col).copied()
    }
}

// [spec:kbdgen:def:ldml.model.hardware+1]
/// The hardware layer set. A keyboard has at most one.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Hardware {
    pub form: Form,
    pub min_device_width: Option<u16>,
    pub layers: Vec<HardwareLayer>,
}

impl Hardware {
    /// The key at `scan_code`'s position in layer `layer`.
    pub fn key_for_scan_code(&self, layer: usize, scan_code: ScanCode) -> Option<KeyIndex> {
        let (row, col) = self.form.position_of(scan_code)?;
        self.layers.get(layer)?.key_at(row, col)
    }

    /// The base when the set is presented as touch: the layer whose sets
    /// are exactly {`none`}.
    pub fn touch_base(&self) -> Option<usize> {
        self.layers
            .iter()
            .position(|l| l.modifiers.as_slice() == [ModifierSet::Set(Modifiers::NONE)])
    }
}

/// Who draws a touch set's bottom row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BottomRow {
    /// The host draws the symbols, globe, space and return row.
    Host,
    /// The authored rows include it.
    Authored,
}

/// A layer of a touch set.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TouchLayer {
    /// Unique within its set.
    pub id: String,
    /// Rows of key indices, of any length.
    pub rows: Vec<Vec<KeyIndex>>,
}

// [spec:kbdgen:def:ldml.model.touch+1]
/// A touch layer set. `Keyboard::touch` lists them in ascending
/// `min_device_width`, a set without one first; widths are distinct whole
/// millimetres from 1 to 999.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TouchSet {
    /// Extension: the size name (`phone`, `tablet`, …); none for a set
    /// read from foreign XML.
    pub name: Option<String>,
    /// Extension.
    pub bottom_row: BottomRow,
    pub min_device_width: Option<u16>,
    pub layers: Vec<TouchLayer>,
    /// The index of the layer with id `base`.
    pub base: u16,
}

impl TouchSet {
    /// The index of the layer with id `id`.
    pub fn layer_index(&self, id: &str) -> Option<usize> {
        self.layers.iter().position(|l| l.id == id)
    }
}
