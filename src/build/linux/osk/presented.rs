//! The presented set (`ldml.linux.touch`): K's hardware set as the
//! on-screen keyboards show it, in no OSK's format. The GNOME Shell,
//! plasma-keyboard and Stevia writers each map it to their own.

use anyhow::{Result, anyhow};
use kbd_model::{
    DEFAULT_WIDTH, Hardware, Key, KeyIndex, Keyboard, MarkerIndex, ModifierSet, Modifiers, TextElem,
};

use super::super::source::LinuxLayout;

/// The width of the added shift, backspace and symbol-switch keys, in
/// thousandths of a key.
pub const ADDED_KEY_WIDTH: u32 = 1500;

/// The space bar's scan code. Each OSK draws its own space bar, so its
/// position is not shown.
const SPACE_SCAN_CODE: u8 = 0x39;

/// What a dead key shows when neither its marker nor its standalone gives
/// it a face.
const DOTTED_CIRCLE: &str = "\u{25CC}";

/// The rows of the two symbol levels, those GNOME Shell's and Stevia's own
/// layouts share.
const SYMBOL_ROWS: [[&str; 3]; 2] = [
    ["1 2 3 4 5 6 7 8 9 0", "@ # € % & - + ( )", "* \" ' : ; ! ?"],
    ["~ ` | • √ π ÷ × ¶ ∆", "£ ¥ $ ¢ ^ ° = { }", "\\ © ® ™ ℅ [ ]"],
];

/// Which level of the presented set a level is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LevelKind {
    /// The layer selected with no modifier.
    Default,
    /// The layer selected with Shift.
    Shift,
    /// The first symbol level: digits and common punctuation.
    Symbols,
    /// The second symbol level, which the first switches to.
    MoreSymbols,
}

/// A key that commits text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextKey {
    /// The text it commits, one or more scalar values.
    pub output: String,
    /// Its display, where that differs from its output.
    pub label: Option<String>,
    /// The plain outputs of its long-press keys, in order.
    pub candidates: Vec<String>,
    /// In thousandths of a key.
    pub width: u32,
}

/// A key whose output is a single dead-key marker. OSKs without dead keys
/// type its standalone instead; the GNOME Shell writer sends the marker's
/// dead keysym where the XKB layout has one (`ldml.linux.touch.gnome`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeadKey {
    pub marker: MarkerIndex,
    /// The marker's display, the key's label.
    pub display: String,
    /// `flush[m]`, what a pending marker stands for; empty when the dead
    /// key has no standalone.
    pub standalone: String,
    /// The plain outputs of its long-press keys, in order.
    pub candidates: Vec<String>,
    /// In thousandths of a key.
    pub width: u32,
}

/// One position of a level's row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Item {
    Text(TextKey),
    Dead(DeadKey),
    /// Empty space of this width, in thousandths of a key: a gap key, a key
    /// with no output, or a position with no key.
    Gap(u32),
    /// Switches between the default and shift levels.
    Shift,
    Backspace,
    /// Switches between the two symbol levels.
    SymbolSwitch,
}

impl Item {
    /// The width, in thousandths of a key.
    pub fn width(&self) -> u32 {
        match self {
            Item::Text(key) => key.width,
            Item::Dead(key) => key.width,
            Item::Gap(width) => *width,
            Item::Shift | Item::Backspace | Item::SymbolSwitch => ADDED_KEY_WIDTH,
        }
    }
}

/// A level: rows of items, top to bottom, without the bottom row with the
/// space bar, which each OSK adds itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Level {
    pub kind: LevelKind,
    /// The hardware layer the level shows; none for a symbol level.
    pub layer: Option<usize>,
    pub rows: Vec<Vec<Item>>,
}

impl Level {
    /// The widest row's width, in thousandths of a key.
    pub fn width(&self) -> u32 {
        self.rows
            .iter()
            .map(|row| row.iter().map(Item::width).sum())
            .max()
            .unwrap_or(0)
    }
}

/// The presented set of one layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Presented {
    pub default: Level,
    pub shift: Option<Level>,
    pub symbols: [Level; 2],
    /// `labels.space`, for the OSK's space bar.
    pub space_label: Option<String>,
    /// `labels.return`, for the OSK's return key.
    pub return_label: Option<String>,
}

impl Presented {
    /// The levels in order: default, shift, then the symbol levels.
    pub fn levels(&self) -> impl Iterator<Item = &Level> {
        self.hardware_levels().chain(self.symbols.iter())
    }

    /// The levels that show hardware layers: default, then shift.
    pub fn hardware_levels(&self) -> impl Iterator<Item = &Level> {
        std::iter::once(&self.default).chain(self.shift.as_ref())
    }
}

/// What a hardware key is on screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Face {
    Text(String),
    Dead(MarkerIndex),
    /// Shown as empty space: a gap, a key with no output, or a role key,
    /// which only touch sets have.
    Blank,
    /// An output mixing text and markers, or of several markers, which no
    /// OSK can type.
    Mixed,
}

// [spec:kbdgen:sem:ldml.linux.touch.keys]
pub(super) fn face(key: &Key) -> Face {
    if key.gap || key.role.is_some() {
        return Face::Blank;
    }
    match key.output.elements() {
        [] => Face::Blank,
        [TextElem::Marker(marker)] => Face::Dead(*marker),
        elements if elements.iter().all(|e| matches!(e, TextElem::Char(_))) => {
            Face::Text(key.output.plain())
        }
        _ => Face::Mixed,
    }
}

/// The layer the engine selects with exactly the components `set`: the
/// non-native layer with that set, else the `Other` layer
/// (`ldml.engine.modifiers`).
// [spec:kbdgen:def:ldml.linux.touch]
fn selected(hardware: &Hardware, set: Modifiers) -> Option<usize> {
    let with = |wanted: ModifierSet| {
        hardware
            .layers
            .iter()
            .position(|layer| !layer.is_native() && layer.modifiers.contains(&wanted))
    };
    with(ModifierSet::Set(set)).or_else(|| with(ModifierSet::Other))
}

/// The form's positions as (row, column), row by row, without the space
/// bar; a row holding only the space bar is left out.
pub(super) fn positions(hardware: &Hardware) -> Vec<Vec<(usize, usize)>> {
    hardware
        .form
        .rows
        .iter()
        .enumerate()
        .map(|(r, row)| {
            row.iter()
                .enumerate()
                .filter(|(_, code)| **code != SPACE_SCAN_CODE)
                .map(|(c, _)| (r, c))
                .collect::<Vec<_>>()
        })
        .filter(|row| !row.is_empty())
        .collect()
}

/// The plain outputs of `key`'s long-press keys, in order.
// [spec:kbdgen:sem:ldml.linux.touch.keys]
fn candidates(keyboard: &Keyboard, key: &Key) -> Vec<String> {
    key.long_press
        .iter()
        .filter_map(|&index| keyboard.key(index))
        .filter_map(|candidate| match face(candidate) {
            Face::Text(text) => Some(text),
            _ => None,
        })
        .collect()
}

/// The display of key `index`: by key id first, then by output.
fn display(keyboard: &Keyboard, index: KeyIndex, key: &Key) -> Option<String> {
    let displays = &keyboard.displays;
    displays
        .for_key(index)
        .or_else(|| displays.for_output(&key.output))
        .map(str::to_owned)
}

/// The face of `marker` on screen: its display, else its standalone, else
/// a dotted circle.
fn marker_display(keyboard: &Keyboard, index: KeyIndex, key: &Key, marker: MarkerIndex) -> String {
    display(keyboard, index, key)
        .filter(|display| !display.is_empty())
        .or_else(|| {
            keyboard
                .flush
                .get(&marker)
                .filter(|flush| !flush.is_empty())
                .cloned()
        })
        .unwrap_or_else(|| DOTTED_CIRCLE.to_owned())
}

/// The item key `index` gives, none for a key no OSK can type, which is
/// left out.
// [spec:kbdgen:sem:ldml.linux.touch.keys]
fn item(keyboard: &Keyboard, index: KeyIndex) -> Option<Item> {
    let Some(key) = keyboard.key(index) else {
        return Some(Item::Gap(DEFAULT_WIDTH));
    };
    match face(key) {
        Face::Blank => Some(Item::Gap(key.width)),
        Face::Mixed => None,
        Face::Text(output) => {
            let label = display(keyboard, index, key).filter(|label| *label != output);
            Some(Item::Text(TextKey {
                output,
                label,
                candidates: candidates(keyboard, key),
                width: key.width,
            }))
        }
        Face::Dead(marker) => Some(Item::Dead(DeadKey {
            marker,
            display: marker_display(keyboard, index, key, marker),
            standalone: keyboard.flush.get(&marker).cloned().unwrap_or_default(),
            candidates: candidates(keyboard, key),
            width: key.width,
        })),
    }
}

/// Adds the backspace key at the end of the first row and `first` at the
/// start of the last, as on a physical keyboard.
// [spec:kbdgen:def:ldml.linux.touch]
fn add_keys(rows: &mut Vec<Vec<Item>>, first: Option<Item>) {
    if rows.is_empty() {
        rows.push(Vec::new());
    }
    if let Some(row) = rows.first_mut() {
        row.push(Item::Backspace);
    }
    if let (Some(row), Some(first)) = (rows.last_mut(), first) {
        row.insert(0, first);
    }
}

/// The level showing hardware layer `layer`: each position of the form
/// but the space bar, a position with no key being a gap one key wide.
// [spec:kbdgen:def:ldml.linux.touch]
fn hardware_level(
    keyboard: &Keyboard,
    hardware: &Hardware,
    kind: LevelKind,
    layer: usize,
    shift: bool,
) -> Level {
    let shown = hardware.layers.get(layer);
    let mut rows: Vec<Vec<Item>> = positions(hardware)
        .into_iter()
        .map(|row| {
            row.into_iter()
                .filter_map(|(r, c)| match shown.and_then(|l| l.key_at(r, c)) {
                    Some(index) => item(keyboard, index),
                    None => Some(Item::Gap(DEFAULT_WIDTH)),
                })
                .collect()
        })
        .collect();
    add_keys(&mut rows, shift.then_some(Item::Shift));
    Level {
        kind,
        layer: Some(layer),
        rows,
    }
}

/// A symbol level of `SYMBOL_ROWS`.
// [spec:kbdgen:def:ldml.linux.touch]
fn symbol_level(kind: LevelKind, rows: &[&str; 3]) -> Level {
    let mut rows: Vec<Vec<Item>> = rows
        .iter()
        .map(|row| {
            row.split(' ')
                .map(|symbol| {
                    Item::Text(TextKey {
                        output: symbol.to_owned(),
                        label: None,
                        candidates: Vec::new(),
                        width: DEFAULT_WIDTH,
                    })
                })
                .collect()
        })
        .collect();
    add_keys(&mut rows, Some(Item::SymbolSwitch));
    Level {
        kind,
        layer: None,
        rows,
    }
}

/// The presented set of `layout`'s K. Fails, naming the layout, when K
/// has no hardware set or no layer that no modifier selects.
// [spec:kbdgen:def:ldml.linux.touch]
pub fn present(layout: &LinuxLayout) -> Result<Presented> {
    let keyboard = &layout.keyboard;
    let tag = &layout.tag;
    let hardware = keyboard
        .hardware
        .as_ref()
        .ok_or_else(|| anyhow!("{tag}: the linux keyboard has no hardware layers"))?;
    let default = selected(hardware, Modifiers::NONE).ok_or_else(|| {
        anyhow!("{tag}: no hardware layer is selected with no modifier, so the on-screen keyboards have no default level")
    })?;
    let shift = selected(hardware, Modifiers::of(&[kbd_model::Component::Shift]));
    let [first, second] = &SYMBOL_ROWS;
    let labels = &keyboard.displays.labels;
    Ok(Presented {
        default: hardware_level(
            keyboard,
            hardware,
            LevelKind::Default,
            default,
            shift.is_some(),
        ),
        shift: shift.map(|layer| hardware_level(keyboard, hardware, LevelKind::Shift, layer, true)),
        symbols: [
            symbol_level(LevelKind::Symbols, first),
            symbol_level(LevelKind::MoreSymbols, second),
        ],
        space_label: labels.space.clone(),
        return_label: labels.r#return.clone(),
    })
}
