//! Keysyms, as IBus passes them in key events: the character each types,
//! as libxkbcommon's `xkb_keysym_to_utf32` gives it, and which are
//! modifiers (`linux.ibus.keys`, `linux.ibus.virtual-keys`).
//!
//! The table is the one kbdgen derives the XKB layout with
//! (`ldml.linux.values`), read from libxkbcommon, so a character the
//! layout writes as a keysym maps back to itself here.

use std::collections::BTreeMap;
use std::sync::LazyLock;

const TABLE: &str = include_str!("../../../src/build/linux/xkb/data/keysyms.tsv");

/// `XKB_KEYSYM_UNICODE_OFFSET`, and the range of keysyms libxkbcommon
/// maps by it.
const UNICODE_OFFSET: u32 = 0x0100_0000;
const UNICODE: std::ops::RangeInclusive<u32> = 0x0100_0100..=0x0110_ffff;

pub const BACKSPACE: u32 = 0xff08;
pub const KP_DELETE: u32 = 0xff9f;

/// Keysym → character, for the keysyms the table gives one.
static CHARS: LazyLock<BTreeMap<u32, char>> = LazyLock::new(|| {
    TABLE
        .lines()
        .filter(|line| !line.starts_with('#'))
        .filter_map(|line| {
            let mut columns = line.split('\t');
            let keysym = u32::from_str_radix(columns.next()?.strip_prefix("0x")?, 16).ok()?;
            let c = u32::from_str_radix(columns.nth(1)?, 16).ok()?;
            Some((keysym, char::from_u32(c)?))
        })
        .collect()
});

/// The character `keysym` types, if any.
// [spec:kbdgen:sem:linux.ibus.virtual-keys]
pub fn to_char(keysym: u32) -> Option<char> {
    if let Some(c) = CHARS.get(&keysym) {
        return Some(*c);
    }
    if !UNICODE.contains(&keysym) {
        return None;
    }
    char::from_u32(keysym.checked_sub(UNICODE_OFFSET)?)
}

/// Whether `keysym` is a modifier's: Shift, Control, Caps Lock, Shift
/// Lock, Meta, Alt, Super and Hyper (`Shift_L` to `Hyper_R`),
/// `ISO_Level3_Shift`, `ISO_Level5_Shift`, `Mode_switch`, which is also
/// `ISO_Group_Shift`, the other `ISO_*Group*` keysyms, and `Num_Lock`.
// [spec:kbdgen:req:linux.ibus.keys]
pub fn is_modifier(keysym: u32) -> bool {
    matches!(
        keysym,
        0xffe1..=0xffee | 0xfe03 | 0xfe11 | 0xff7e | 0xfe06..=0xfe0f | 0xff7f
    )
}

/// The keysym the table names `name`.
pub fn by_name(name: &str) -> Option<u32> {
    TABLE
        .lines()
        .filter(|line| !line.starts_with('#'))
        .find_map(|line| {
            let mut columns = line.split('\t');
            let keysym = columns.next()?;
            (columns.next()? == name)
                .then(|| u32::from_str_radix(keysym.strip_prefix("0x")?, 16).ok())
                .flatten()
        })
}
