//! What a layout's XKB layout does not reproduce, reported once per layout
//! and feature (`ldml.linux.classify`), and whether its model rewrites
//! committed text (`ldml.linux.rewrites`).

use kbd_model::{
    Component, Hardware, HardwareLayer, Keyboard, ModifierSet, Normalization, TransformGroup,
};

use super::Adapted;
use super::keycodes::KEYS;
use super::levels::Value;
use super::types::XkbState;
use crate::build::diag::Diagnostics;
use crate::build::macos::adapter::Category;
use crate::build::windows::kbdl::adapter::{is_generated_backspace, starts_with_marker};

/// The features of `ldml.linux.rewrites`, each with up to ten examples.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Rewrites {
    pub text_transforms: Category,
    pub reorder_groups: Category,
    pub backspace_rules: Category,
    pub normalization: bool,
}

impl Rewrites {
    /// Whether the model rewrites committed text, so that an edit's
    /// `delete` can be non-zero.
    pub fn any(&self) -> bool {
        self.text_transforms.count > 0
            || self.reorder_groups.count > 0
            || self.backspace_rules.count > 0
            || self.normalization
    }
}

/// The features by which a model rewrites committed text: a `simple`
/// transform that does not start with a marker, a `reorder` group, a
/// `backspace` rule other than the generated one, and an enabled
/// normalization.
// [spec:kbdgen:def:ldml.linux.rewrites]
pub fn rewrites(keyboard: &Keyboard) -> Rewrites {
    let mut result = Rewrites {
        normalization: keyboard.normalization == Normalization::Enabled,
        ..Rewrites::default()
    };
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
    result
}

/// Everything of `ldml.linux.classify` for one layout.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Classification {
    pub long_outputs: Category,
    pub mixed_outputs: Category,
    pub dead_keys_without_keysym: Category,
    pub dead_key_results: Category,
    pub rewrites: Rewrites,
    pub wrong_states: Category,
    pub extra_modifier_layers: Category,
    pub shift_lock: bool,
    pub lrm_rlm: bool,
    pub unwritten_positions: Category,
    pub long_decimal: Option<String>,
    pub gestures: Category,
    /// Layers only Left Alt or a Left Ctrl selects.
    pub left_layers: Category,
    /// Layers with a set naming `alt` and no Control, typed with AltGr
    /// alone where macOS types them with either Option.
    pub alt_layers: Category,
}

/// A layer as v4 YAML names it: its sets joined by `, `.
fn layer_name(layer: &HardwareLayer) -> String {
    let names: Vec<String> = layer
        .modifiers
        .iter()
        .map(|set| match set {
            ModifierSet::Other => "other".to_owned(),
            ModifierSet::Set(s) if s.is_empty() => "none".to_owned(),
            ModifierSet::Set(s) => s
                .components()
                .map(Component::name)
                .collect::<Vec<_>>()
                .join(" "),
        })
        .collect();
    names.join(", ")
}

/// `<NAME>`, with the state when it has modifiers.
fn at(name: &str, state: XkbState) -> String {
    if state.mods() == 0 {
        format!("<{name}>")
    } else {
        format!("<{name}> with {state}")
    }
}

fn values(adapted: &Adapted, keyboard: &Keyboard, result: &mut Classification) {
    let mut seen = Vec::new();
    for key in &adapted.levels.keys {
        for state in XkbState::ALL {
            let index = state.index();
            let (Some(value), Some(model_key)) = (key.values.get(index), key.keys[index]) else {
                continue;
            };
            if seen.contains(&(key.key.name, model_key)) {
                continue;
            }
            match value {
                Value::Long(text) => result
                    .long_outputs
                    .add(|| format!("{}: {text:?}", at(key.key.name, state))),
                Value::Mixed => result.mixed_outputs.add(|| {
                    let output = keyboard
                        .key(model_key)
                        .map(|k| k.id.clone())
                        .unwrap_or_default();
                    format!("{}: key {output}", at(key.key.name, state))
                }),
                _ => continue,
            }
            seen.push((key.key.name, model_key));
        }
    }
}

fn dead_keys(adapted: &Adapted, keyboard: &Keyboard, result: &mut Classification) {
    for marker in &adapted.dead.without {
        let flush = keyboard.flush.get(marker).cloned().unwrap_or_default();
        result.dead_keys_without_keysym.add(|| {
            format!(
                "\\m{{{}}}, typed as {flush:?}",
                keyboard.marker_name(*marker).unwrap_or("?")
            )
        });
    }
    for difference in &adapted.dead.differences {
        result.dead_key_results.add(|| difference.line());
    }
}

fn wrong_states(adapted: &Adapted, result: &mut Classification) {
    for (key, typed) in adapted.levels.keys.iter().zip(&adapted.typed) {
        for state in &typed.wrong {
            let wanted = key
                .values
                .get(state.index())
                .map(|value| match value {
                    Value::Char(c) => format!("{c:?}"),
                    Value::Empty => "nothing".to_owned(),
                    _ => "its keysym".to_owned(),
                })
                .unwrap_or_default();
            result.wrong_states.add(|| {
                format!(
                    "{} should type {wanted} ({})",
                    at(key.key.name, *state),
                    typed.key_type.name()
                )
            });
        }
    }
}

fn is_extra(set: &ModifierSet) -> bool {
    matches!(set, ModifierSet::Set(s) if s.components().any(|c| c.extra_number().is_some()))
}

fn is_left(set: &ModifierSet) -> bool {
    matches!(set, ModifierSet::Set(s) if s.contains(Component::AltL) || s.contains(Component::CtrlL))
}

fn is_alt(set: &ModifierSet) -> bool {
    use Component::*;
    matches!(set, ModifierSet::Set(s) if s.contains(Alt) && ![Ctrl, CtrlL, CtrlR].iter().any(|c| s.contains(*c)))
}

fn layers(adapted: &Adapted, hardware: &Hardware, result: &mut Classification) {
    let selected = |index: usize| adapted.levels.layers.contains(&Some(index));
    for (index, layer) in hardware.layers.iter().enumerate() {
        if layer.is_native() {
            continue;
        }
        let name = layer_name(layer);
        if layer.modifiers.iter().any(is_extra) {
            result.extra_modifier_layers.add(|| format!("layer {name}"));
        } else if !selected(index) && layer.modifiers.iter().all(is_left) {
            result.left_layers.add(|| format!("layer {name}"));
        } else if selected(index) && layer.modifiers.iter().any(is_alt) {
            result.alt_layers.add(|| format!("layer {name}"));
        }
    }
}

fn positions(
    adapted: &Adapted,
    keyboard: &Keyboard,
    hardware: &Hardware,
    result: &mut Classification,
) {
    let mut layers: Vec<usize> = adapted.levels.layers.iter().flatten().copied().collect();
    layers.sort_unstable();
    layers.dedup();
    for (row, codes) in hardware.form.rows.iter().enumerate() {
        for (col, scan) in codes.iter().enumerate() {
            if KEYS.iter().any(|key| key.scan == *scan) {
                continue;
            }
            for index in &layers {
                let types = hardware
                    .layers
                    .get(*index)
                    .and_then(|layer| layer.key_at(row, col))
                    .and_then(|key| keyboard.key(key))
                    .is_some_and(|key| !key.gap && !key.output.is_empty());
                if types {
                    let name = hardware
                        .layers
                        .get(*index)
                        .map(layer_name)
                        .unwrap_or_default();
                    result
                        .unwritten_positions
                        .add(|| format!("scan code {scan:02X} in layer {name}"));
                }
            }
        }
    }
}

fn gestures(keyboard: &Keyboard, hardware: &Hardware, result: &mut Classification) {
    let mut seen = Vec::new();
    for key in hardware.layers.iter().flat_map(|l| l.rows.iter().flatten()) {
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

/// Classifies a layout's XKB layout against its keyboard.
// [spec:kbdgen:req:ldml.linux.classify+1]
pub fn classify(adapted: &Adapted, keyboard: &Keyboard, hardware: &Hardware) -> Classification {
    let mut result = Classification {
        rewrites: rewrites(keyboard),
        shift_lock: keyboard.windows.shift_lock,
        lrm_rlm: keyboard.windows.lrm_rlm,
        long_decimal: adapted
            .decimal
            .as_ref()
            .and_then(|decimal| decimal.long.clone()),
        ..Classification::default()
    };
    values(adapted, keyboard, &mut result);
    dead_keys(adapted, keyboard, &mut result);
    wrong_states(adapted, &mut result);
    layers(adapted, hardware, &mut result);
    positions(adapted, keyboard, hardware, &mut result);
    gestures(keyboard, hardware, &mut result);
    result
}

fn counted(label: &str, category: &Category) -> String {
    format!(
        "{} {label} (e.g. {})",
        category.count,
        category.examples.join("; ")
    )
}

/// An information message that the XKB layout cannot type a feature and
/// the IBus engine does.
fn engine_types(diag: &mut Diagnostics, label: &str, category: &Category) {
    if category.count > 0 {
        diag.inform(format!(
            "the XKB layout cannot type {}; the IBus engine types them",
            counted(label, category)
        ));
    }
}

/// Reports `classification`: an information message per feature the XKB
/// layout loses, a warning per kind of layer nothing on Linux types, and
/// one information message when it loses nothing.
// [spec:kbdgen:req:ldml.linux.classify+1]
pub fn report(c: &Classification, diag: &mut Diagnostics) {
    engine_types(
        diag,
        "output(s) of more than one scalar value",
        &c.long_outputs,
    );
    engine_types(diag, "output(s) mixing text and markers", &c.mixed_outputs);
    engine_types(
        diag,
        "dead key(s) without a dead keysym, which it types as their standalone",
        &c.dead_keys_without_keysym,
    );
    if c.dead_key_results.count > 0 {
        diag.inform(format!(
            "the XKB layout composes {} otherwise than the engine, because IBus's simple engine composes by the reference Compose table, drops a key that continues no sequence and keeps the dead key pending, where the engine types the dead key's standalone and the key; the IBus engine types them as the engine does",
            counted("dead-key result(s)", &c.dead_key_results)
        ));
    }
    let r = &c.rewrites;
    engine_types(
        diag,
        "transform rule(s) that do not start with a marker",
        &r.text_transforms,
    );
    engine_types(diag, "reorder group(s)", &r.reorder_groups);
    engine_types(
        diag,
        "backspace rule(s) besides the generated one",
        &r.backspace_rules,
    );
    if r.normalization {
        diag.inform("the XKB layout cannot normalize its output; the IBus engine does");
    }
    engine_types(diag, "key state(s) that no key type gives", &c.wrong_states);
    engine_types(
        diag,
        "layer(s) selected by an extra modifier",
        &c.extra_modifier_layers,
    );
    if c.shift_lock {
        diag.inform("the XKB layout has no Shift Lock; the IBus engine types it");
    }
    if c.lrm_rlm {
        diag.inform(
            "the XKB layout cannot type LRM/RLM on Shift+Backspace; the IBus engine types them",
        );
    }
    engine_types(
        diag,
        "output(s) at scan codes it does not write",
        &c.unwritten_positions,
    );
    if let Some(decimal) = &c.long_decimal {
        diag.inform(format!(
            "the XKB layout cannot type the decimal {decimal:?}, which is more than one scalar value, and types KP_Decimal; the IBus engine types it"
        ));
    }
    if c.gestures.count > 0 {
        diag.inform(format!(
            "the XKB layout ignores the long-press, multi-tap and flick gestures of {}, which no hardware key makes",
            counted("key(s)", &c.gestures)
        ));
    }
    if c.left_layers.count > 0 {
        diag.warn(format!(
            "nothing on Linux types {}: the XKB layout leaves Left Alt and Left Ctrl to shortcuts, and the IBus engine passes their chords",
            counted("layer(s) that only Left Alt or a Left Ctrl selects", &c.left_layers)
        ));
    }
    if c.alt_layers.count > 0 {
        diag.inform(format!(
            "Linux types {} with AltGr alone; Left Alt stays Alt for shortcuts",
            counted("alt layer(s)", &c.alt_layers)
        ));
    }
    if diag.warnings().is_empty() && diag.infos().is_empty() {
        diag.inform("the XKB layout types every feature of the keyboard");
    }
}
