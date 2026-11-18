//! Linux's evdev key codes of the PC/AT set 1 scan codes without `E0` that
//! the model names keys by (`ldml.linux.keycodes`), paired as Chromium's
//! `dom_code_data.inc` pairs them. kbdgen's XKB layout writes keys by them
//! and the IBus engine, which IBus hands evdev key codes, reads them back.

use crate::ScanCode;

/// The scan codes past `58` with an evdev key code.
const HIGH: [(ScanCode, u16); 9] = [
    (0x59, 117),
    (0x70, 93),
    (0x71, 123),
    (0x72, 122),
    (0x73, 89),
    (0x79, 92),
    (0x7B, 94),
    (0x7D, 124),
    (0x7E, 121),
];

/// The evdev key code of a scan code, if it has one.
// [spec:kbdgen:def:ldml.linux.keycodes]
pub fn evdev_code(scan: ScanCode) -> Option<u16> {
    match scan {
        0x45 | 0x54 | 0x55 => None,
        0x01..=0x58 => Some(u16::from(scan)),
        _ => HIGH
            .iter()
            .find(|(code, _)| *code == scan)
            .map(|(_, evdev)| *evdev),
    }
}

/// The scan code of an evdev key code, if it has one.
// [spec:kbdgen:def:ldml.linux.keycodes]
pub fn scan_code(evdev: u16) -> Option<ScanCode> {
    match evdev {
        0x45 | 0x54 | 0x55 => None,
        0x01..=0x58 => ScanCode::try_from(evdev).ok(),
        _ => HIGH
            .iter()
            .find(|(_, code)| *code == evdev)
            .map(|(scan, _)| *scan),
    }
}
