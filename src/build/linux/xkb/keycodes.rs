//! The keys the XKB layout writes and their names (`ldml.linux.keycodes`).
//! The evdev key code of a scan code is `kbd-model`'s, which the IBus
//! engine reads keys by too.

use kbd_model::ScanCode;
pub use kbd_model::{evdev_code, scan_code};

/// A key the XKB layout writes: its scan code and its name in
/// xkeyboard-config's `keycodes/evdev`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XkbKey {
    pub scan: ScanCode,
    pub name: &'static str,
}

impl XkbKey {
    /// The evdev key code, which IBus passes.
    pub fn evdev(self) -> u16 {
        evdev_code(self.scan).unwrap_or_default()
    }

    /// The XKB key code, the evdev key code + 8.
    pub fn keycode(self) -> u32 {
        u32::from(self.evdev()) + 8
    }
}

const fn key(scan: ScanCode, name: &'static str) -> XkbKey {
    XkbKey { scan, name }
}

/// The keys of `ldml.linux.levels`, in the order the symbols file writes
/// them: the 49 ISO positions of `kbdl.scancodes.iso`, the JIS yen key and
/// the space bar.
// [spec:kbdgen:def:ldml.linux.keycodes]
pub const KEYS: [XkbKey; 51] = [
    key(0x29, "TLDE"),
    key(0x02, "AE01"),
    key(0x03, "AE02"),
    key(0x04, "AE03"),
    key(0x05, "AE04"),
    key(0x06, "AE05"),
    key(0x07, "AE06"),
    key(0x08, "AE07"),
    key(0x09, "AE08"),
    key(0x0A, "AE09"),
    key(0x0B, "AE10"),
    key(0x0C, "AE11"),
    key(0x0D, "AE12"),
    key(0x7D, "AE13"),
    key(0x10, "AD01"),
    key(0x11, "AD02"),
    key(0x12, "AD03"),
    key(0x13, "AD04"),
    key(0x14, "AD05"),
    key(0x15, "AD06"),
    key(0x16, "AD07"),
    key(0x17, "AD08"),
    key(0x18, "AD09"),
    key(0x19, "AD10"),
    key(0x1A, "AD11"),
    key(0x1B, "AD12"),
    key(0x1E, "AC01"),
    key(0x1F, "AC02"),
    key(0x20, "AC03"),
    key(0x21, "AC04"),
    key(0x22, "AC05"),
    key(0x23, "AC06"),
    key(0x24, "AC07"),
    key(0x25, "AC08"),
    key(0x26, "AC09"),
    key(0x27, "AC10"),
    key(0x28, "AC11"),
    key(0x2B, "BKSL"),
    key(0x56, "LSGT"),
    key(0x2C, "AB01"),
    key(0x2D, "AB02"),
    key(0x2E, "AB03"),
    key(0x2F, "AB04"),
    key(0x30, "AB05"),
    key(0x31, "AB06"),
    key(0x32, "AB07"),
    key(0x33, "AB08"),
    key(0x34, "AB09"),
    key(0x35, "AB10"),
    key(0x73, "AB11"),
    key(0x39, "SPCE"),
];

/// The numpad decimal key, written for K's `decimal` (`ldml.linux.decimal`).
// [spec:kbdgen:def:ldml.linux.keycodes]
pub const DECIMAL: XkbKey = key(0x53, "KPDL");
