//! What a `chromeOS` keyboard can say that the extension cannot give
//! (`ldml.chromeos.classify`), reported once per layout and feature. The
//! engine runs whole, so transforms, reorder, backspace rules, markers and
//! normalization all work; what is lost is what a hardware key event on
//! ChromeOS cannot reach, and what only an on-screen keyboard would show.

use kbd_model::{Component, Keyboard, ModifierSet};

use crate::build::macos::adapter::{Category, automatic};
use crate::build::windows::kbdl::diag::Diagnostics;

/// The scan codes the service worker sends as `Key::Scan`: the values of
/// its `SCAN_CODES` table (`ldml.chromeos.keys`).
pub const REACHABLE_SCAN_CODES: [u8; 87] = [
    0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0F, 0x10, 0x11,
    0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x1B, 0x1C, 0x1E, 0x1F, 0x20, 0x21, 0x22,
    0x23, 0x24, 0x25, 0x26, 0x27, 0x28, 0x29, 0x2B, 0x2C, 0x2D, 0x2E, 0x2F, 0x30, 0x31, 0x32, 0x33,
    0x34, 0x35, 0x37, 0x39, 0x3B, 0x3C, 0x3D, 0x3E, 0x3F, 0x40, 0x41, 0x42, 0x43, 0x44, 0x46, 0x47,
    0x48, 0x49, 0x4A, 0x4B, 0x4C, 0x4D, 0x4E, 0x4F, 0x50, 0x51, 0x52, 0x56, 0x57, 0x58, 0x59, 0x70,
    0x71, 0x72, 0x73, 0x79, 0x7B, 0x7D, 0x7E,
];

/// Everything of `ldml.chromeos.classify` for one keyboard.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Classification {
    pub left_alt: Category,
    pub positions: Category,
    pub gestures: Category,
    pub displays: Category,
}

fn set_name(set: kbd_model::Modifiers) -> String {
    if set.is_empty() {
        return "none".to_owned();
    }
    set.components()
        .map(Component::name)
        .collect::<Vec<_>>()
        .join(" ")
}

// [spec:kbdgen:req:ldml.chromeos.classify]
pub fn classify(keyboard: &Keyboard) -> Classification {
    let mut result = Classification::default();
    if let Some(hardware) = &keyboard.hardware {
        for layer in hardware.layers.iter().filter(|layer| !layer.is_native()) {
            for set in &layer.modifiers {
                if let ModifierSet::Set(set) = set
                    && set.contains(Component::AltL)
                {
                    result.left_alt.add(|| set_name(*set));
                }
            }
        }
        for (r, row) in hardware.form.rows.iter().enumerate() {
            for (c, &code) in row.iter().enumerate() {
                let types = hardware.layers.iter().any(|layer| {
                    layer
                        .key_at(r, c)
                        .and_then(|k| keyboard.key(k))
                        .is_some_and(|k| !k.gap && !k.output.is_empty())
                });
                if types && !REACHABLE_SCAN_CODES.contains(&code) {
                    result.positions.add(|| format!("scan code {code:02X}"));
                }
            }
        }
        let mut seen = Vec::new();
        for key in hardware.layers.iter().flat_map(|l| l.rows.iter().flatten()) {
            let Some(k) = keyboard.key(*key).filter(|_| !seen.contains(key)) else {
                continue;
            };
            seen.push(*key);
            let gestures = !k.long_press.is_empty()
                || k.long_press_default.is_some()
                || !k.multi_tap.is_empty()
                || k.flick.is_some();
            if gestures {
                result.gestures.add(|| k.id.clone());
            }
        }
    }
    for entry in &keyboard.displays.entries {
        if !automatic(keyboard, &entry.target, &entry.display) {
            result.displays.add(|| entry.display.clone());
        }
    }
    let labels = &keyboard.displays.labels;
    for label in [&labels.space, &labels.r#return].into_iter().flatten() {
        let shown = keyboard
            .displays
            .entries
            .iter()
            .any(|entry| entry.display == *label);
        if !shown {
            result.displays.add(|| label.clone());
        }
    }
    result
}

fn describe(what: &str, why: &str, category: &Category, diag: &mut Diagnostics) {
    if category.count > 0 {
        diag.warn(format!(
            "ChromeOS cannot use {} {what}, because {why} (e.g. {})",
            category.count,
            category.examples.join("; ")
        ));
    }
}

// [spec:kbdgen:req:ldml.chromeos.classify]
pub fn report(classification: &Classification, diag: &mut Diagnostics) {
    let c = classification;
    describe(
        "modifier set(s) needing Left Alt",
        "Left Alt is ChromeOS's shortcut modifier and only Right Alt types",
        &c.left_alt,
        diag,
    );
    describe(
        "key position(s)",
        "no ChromeOS key event carries their scan codes",
        &c.positions,
        diag,
    );
    describe(
        "key(s) with long-press, multi-tap or flick gestures",
        "hardware keys have no gestures",
        &c.gestures,
        diag,
    );
    describe(
        "display(s) and key label(s)",
        "the input method draws no keyboard",
        &c.displays,
        diag,
    );
}
