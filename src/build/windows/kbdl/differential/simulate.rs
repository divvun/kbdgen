//! `ToUnicodeEx` over a layout's generated tables: the modifier bits of
//! `aVkToBits`, the column `ModNumber` gives them, the caps attributes, and
//! the dead-key state `aDeadKey` drives. It follows the documented
//! behaviour of `kbd.h` and of `kbdl.*`, so that the comparison runs on
//! every host; on Windows the real function is checked against it.

use super::super::tables::{
    CAPLOK, CAPLOKALTGR, DKF_DEAD, DeadKeyEntry, EXTRA_MODIFIER_VKS, SGCAPS, SHFT_INVALID, Tables,
    VK_DEAD_ROW, WCH_DEAD, WCH_LGTR, WCH_NONE,
};

const VK_SHIFT: u8 = 0x10;
const VK_CONTROL: u8 = 0x11;
const VK_MENU: u8 = 0x12;
const KBDSHIFT: u16 = 0x01;
const KBDCTRL: u16 = 0x02;
const KBDALT: u16 = 0x04;

/// The keyboard state `ToUnicodeEx` is given: which modifier keys are held
/// and whether Caps Lock is toggled on. AltGr is Ctrl and Alt together.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WinKeys {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub extra: [bool; 3],
    pub caps: bool,
}

impl WinKeys {
    /// The held virtual keys that `aVkToBits` can name.
    pub fn held(self) -> Vec<u8> {
        let mut held = Vec::new();
        for (down, vk) in [
            (self.shift, VK_SHIFT),
            (self.ctrl, VK_CONTROL),
            (self.alt, VK_MENU),
        ] {
            if down {
                held.push(vk);
            }
        }
        for (down, vk) in self.extra.iter().zip(EXTRA_MODIFIER_VKS) {
            if *down {
                held.push(vk);
            }
        }
        held
    }
}

/// What one key translates to before dead-key processing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Translated {
    Nothing,
    Char(u16),
    /// A dead key, with its dead id from the following `0xff` row.
    Dead(u16),
    Ligature(Vec<u16>),
}

/// The result of one `ToUnicodeEx` call: 0, a positive count of units, or
/// -1 with the dead character now pending.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Typed {
    Nothing,
    Text(Vec<u16>),
    Dead(u16),
}

/// A loaded layout with the kernel's pending dead character.
pub struct Dll<'t> {
    tables: &'t Tables,
    pending: Option<u16>,
}

impl<'t> Dll<'t> {
    pub fn new(tables: &'t Tables) -> Self {
        Dll {
            tables,
            pending: None,
        }
    }

    pub fn pending(&self) -> Option<u16> {
        self.pending
    }

    /// The virtual key of a scan code without `E0`, from `ausVK`.
    pub fn vk(&self, scan: u8) -> u8 {
        self.tables
            .aus_vk
            .get(usize::from(scan))
            .map_or(0xff, |vk| (vk & 0xff) as u8)
    }

    /// The modifier bits of the held keys, through `aVkToBits`.
    fn mod_bits(&self, keys: WinKeys) -> u16 {
        let held = keys.held();
        self.tables
            .vk_to_bits
            .iter()
            .filter(|(vk, _)| held.contains(vk))
            .fold(0, |bits, (_, bit)| bits | u16::from(*bit))
    }

    /// The index of the character row of `vk`; dead and SGCAPS rows follow
    /// their key's row, so the first row with the key is its own.
    pub fn row(&self, vk: u8) -> Option<usize> {
        if vk == VK_DEAD_ROW {
            return None;
        }
        self.tables.rows.iter().position(|row| row.vk == vk)
    }

    /// The `Attributes` of `vk`'s row, or 0 when it has none.
    pub fn attributes(&self, vk: u8) -> u8 {
        self.row(vk)
            .map_or(0, |row| self.tables.rows[row].attributes)
    }

    fn ligature(&self, vk: u8, column: usize) -> Vec<u16> {
        self.tables
            .ligatures
            .iter()
            .find(|ligature| ligature.vk == vk && usize::from(ligature.column) == column)
            .map(|ligature| {
                ligature
                    .units
                    .iter()
                    .copied()
                    .take_while(|unit| *unit != WCH_NONE)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The character of `vk` under `keys`, before dead-key processing. Caps
    /// Lock acts on a `CAPLOK` row only when no modifier but Shift is held,
    /// on a `CAPLOKALTGR` row only with AltGr, and selects an SGCAPS row's
    /// follower only when no modifier but Shift is held. Ctrl alone on a
    /// letter with no character types its C0 control, as Windows does for
    /// every layout.
    pub fn translate(&self, vk: u8, keys: WinKeys) -> Translated {
        let Some(mut row) = self.row(vk) else {
            return Translated::Nothing;
        };
        let mut bits = self.mod_bits(keys);
        let attributes = self.tables.rows[row].attributes;
        let only_shift = bits & !KBDSHIFT == 0;
        if keys.caps {
            if attributes & CAPLOK != 0 && only_shift {
                bits ^= KBDSHIFT;
            }
            if attributes & CAPLOKALTGR != 0 && bits & (KBDCTRL | KBDALT) == KBDCTRL | KBDALT {
                bits ^= KBDSHIFT;
            }
            if attributes & SGCAPS != 0 && only_shift {
                row += 1;
            }
        }
        if bits > self.tables.max_mod_bits {
            return Translated::Nothing;
        }
        let column = self.tables.mod_numbers[usize::from(bits)];
        if column == SHFT_INVALID || usize::from(column) >= self.tables.columns {
            return Translated::Nothing;
        }
        let column = usize::from(column);
        match self.tables.rows[row].wch[column] {
            WCH_NONE if bits == KBDCTRL && vk.is_ascii_uppercase() => {
                Translated::Char(u16::from(vk - 0x40))
            }
            WCH_NONE => Translated::Nothing,
            WCH_DEAD => match self.tables.rows.get(row + 1) {
                Some(dead) if dead.vk == VK_DEAD_ROW => Translated::Dead(dead.wch[column]),
                _ => Translated::Nothing,
            },
            WCH_LGTR => Translated::Ligature(self.ligature(vk, column)),
            unit => Translated::Char(unit),
        }
    }

    fn composition(&self, dead: u16, base: u16) -> Option<&DeadKeyEntry> {
        self.tables
            .dead_keys
            .iter()
            .find(|entry| entry.id == dead && entry.base == base)
    }

    /// One `ToUnicodeEx` call. A pending dead character composes with the
    /// next character or dead key through `aDeadKey`; a `DKF_DEAD` entry
    /// leaves its result pending. With a ligature, it composes with the
    /// ligature's first unit, and the rest follows (verified with
    /// `ToUnicodeEx`; a `DKF_DEAD` entry there is untested and taken as no
    /// entry). Without an entry, the pending dead
    /// character is typed, followed by the key's own units, a dead key's
    /// dead character included, and nothing stays pending (`kbdl.dead-keys`;
    /// verified with `ToUnicodeEx`). A key that translates to nothing keeps
    /// the pending state.
    pub fn press(&mut self, vk: u8, keys: WinKeys) -> Typed {
        let translated = self.translate(vk, keys);
        let units = match &translated {
            Translated::Nothing => return Typed::Nothing,
            Translated::Char(unit) | Translated::Dead(unit) => vec![*unit],
            Translated::Ligature(units) => units.clone(),
        };
        let Some(dead) = self.pending.take() else {
            return match translated {
                Translated::Dead(id) => {
                    self.pending = Some(id);
                    Typed::Dead(id)
                }
                _ => Typed::Text(units),
            };
        };
        let entry = self.composition(dead, units[0]).copied();
        match entry {
            Some(entry) if units.len() > 1 && entry.flags & DKF_DEAD == 0 => {
                Typed::Text([&[entry.composed], &units[1..]].concat())
            }
            Some(entry) if units.len() == 1 && entry.flags & DKF_DEAD != 0 => {
                self.pending = Some(entry.composed);
                Typed::Dead(entry.composed)
            }
            Some(entry) if units.len() == 1 => Typed::Text(vec![entry.composed]),
            _ => Typed::Text([vec![dead], units].concat()),
        }
    }
}
