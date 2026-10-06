//! What a layout's `.keylayout` cannot express, reported once per layout
//! and feature.

use kbd_model::{DisplayTarget, Hardware, Keyboard, Normalization, Text, TextElem, TransformGroup};

use super::dead_keys::DeadKeys;
use super::layers::Derived;
use crate::build::windows::kbdl::adapter::{is_generated_backspace, starts_with_marker};
use crate::build::windows::kbdl::diag::Diagnostics;

/// The most examples a category lists.
const MAX_EXAMPLES: usize = 10;

/// One feature a keylayout drops: how often, and up to ten examples.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Category {
    pub count: usize,
    pub examples: Vec<String>,
}

impl Category {
    fn add(&mut self, example: impl FnOnce() -> String) {
        if self.examples.len() < MAX_EXAMPLES {
            self.examples.push(example());
        }
        self.count += 1;
    }
}

/// Everything of `ldml.macos.classify` for one layout.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Classification {
    pub text_transforms: Category,
    pub reorder_groups: Category,
    pub backspace_rules: Category,
    pub mixed_outputs: Category,
    pub dead_key_results: Category,
    pub gestures: Category,
    pub displays: Category,
    pub windows_options: Category,
    pub normalization: bool,
}

fn transforms(keyboard: &Keyboard, result: &mut Classification) {
    for (g, group) in keyboard.simple.iter().enumerate() {
        match group {
            TransformGroup::Rules(rules) => {
                for (r, rule) in rules.iter().enumerate() {
                    if !starts_with_marker(&rule.from) {
                        result
                            .text_transforms
                            .add(|| format!("group {} rule {}", g + 1, r + 1));
                    }
                }
            }
            TransformGroup::Reorder(rules) => result
                .reorder_groups
                .add(|| format!("group {} ({} rules)", g + 1, rules.len())),
        }
    }
    for (g, group) in keyboard.backspace.iter().enumerate() {
        match group {
            TransformGroup::Rules(rules) => {
                for (r, rule) in rules.iter().enumerate() {
                    if !is_generated_backspace(rule) {
                        result
                            .backspace_rules
                            .add(|| format!("group {} rule {}", g + 1, r + 1));
                    }
                }
            }
            TransformGroup::Reorder(_) => result
                .backspace_rules
                .add(|| format!("group {} (reorder)", g + 1)),
        }
    }
}

/// Gestures on keys of the written layers, which hardware keys ignore.
fn gestures(
    keyboard: &Keyboard,
    hardware: &Hardware,
    derived: &Derived,
    result: &mut Classification,
) {
    let mut seen = Vec::new();
    for map in &derived.maps {
        for key in hardware.layers[map.layer].rows.iter().flatten() {
            let Some(k) = keyboard.key(*key).filter(|_| !seen.contains(key)) else {
                continue;
            };
            seen.push(*key);
            let has = !k.long_press.is_empty()
                || k.long_press_default.is_some()
                || !k.multi_tap.is_empty()
                || k.flick.is_some();
            if has {
                result.gestures.add(|| k.id.clone());
            }
        }
    }
}

/// Whether a display is one lowering adds by itself: U+25CC before an
/// output of marks, or a dead key shown as its flush output.
fn automatic(keyboard: &Keyboard, target: &DisplayTarget, display: &str) -> bool {
    match target {
        DisplayTarget::Output(output) => match output.elements() {
            [TextElem::Marker(marker)] => keyboard.flush.get(marker).is_some_and(|f| f == display),
            _ => display == format!("\u{25CC}{}", output.plain()),
        },
        DisplayTarget::Key(key) => keyboard.key(*key).is_some_and(|key| {
            let shown = Text::from(display);
            key.output == shown || display == format!("\u{25CC}{}", key.output.plain())
        }),
    }
}

/// Classifies a keyboard, its key maps and its dead keys.
// [spec:kbdgen:req:ldml.macos.classify]
pub fn classify(
    keyboard: &Keyboard,
    hardware: &Hardware,
    derived: &Derived,
    dead: &DeadKeys,
) -> Classification {
    let mut result = Classification {
        normalization: keyboard.normalization == Normalization::Enabled,
        ..Classification::default()
    };
    transforms(keyboard, &mut result);
    for at in &derived.mixed {
        result.mixed_outputs.add(|| at.clone());
    }
    for line in &dead.omitted {
        result.dead_key_results.add(|| line.clone());
    }
    gestures(keyboard, hardware, derived, &mut result);
    for entry in &keyboard.displays.entries {
        if !automatic(keyboard, &entry.target, &entry.display) {
            result.displays.add(|| entry.display.clone());
        }
    }
    let labels = &keyboard.displays.labels;
    for label in [&labels.space, &labels.r#return].into_iter().flatten() {
        if !keyboard
            .displays
            .entries
            .iter()
            .any(|entry| entry.display == *label)
        {
            result.displays.add(|| label.clone());
        }
    }
    let windows = &keyboard.windows;
    for key in &windows.extra_modifiers {
        result
            .windows_options
            .add(|| format!("extra modifier {}", key.name()));
    }
    if windows.lrm_rlm {
        result
            .windows_options
            .add(|| "LRM/RLM on Shift+Backspace".to_owned());
    }
    result
}

fn describe(label: &str, category: &Category, diag: &mut Diagnostics) {
    if category.count > 0 {
        diag.warn(format!(
            "the .keylayout cannot express {} {label} (e.g. {})",
            category.count,
            category.examples.join("; ")
        ));
    }
}

/// Reports `classification`: one warning per feature the `.keylayout`
/// drops, or one information message when it drops nothing.
// [spec:kbdgen:req:ldml.macos.classify]
pub fn report(classification: &Classification, diag: &mut Diagnostics) {
    let c = classification;
    describe(
        "transform rule(s) that do not start with a marker",
        &c.text_transforms,
        diag,
    );
    describe("reorder group(s)", &c.reorder_groups, diag);
    describe(
        "backspace rule(s) besides the generated one",
        &c.backspace_rules,
        diag,
    );
    describe("output(s) mixing text and markers", &c.mixed_outputs, diag);
    describe(
        "dead-key result(s) leaving text and a marker, or several markers",
        &c.dead_key_results,
        diag,
    );
    describe(
        "key(s) with long-press, multi-tap or flick gestures",
        &c.gestures,
        diag,
    );
    describe("key display(s) and label(s)", &c.displays, diag);
    describe("Windows-only option(s)", &c.windows_options, diag);
    if c.normalization {
        diag.warn(
            "the .keylayout cannot express normalization: enabled; it types the authored text",
        );
    }
    if diag.warnings().is_empty() {
        diag.inform("the .keylayout expresses every feature of the keyboard");
    }
}
