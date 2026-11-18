//! The explanations of `ldml.linux.test.typing`: where the XKB layout may
//! type something other than the engine, each traced to a feature
//! `ldml.linux.classify` reports.

use kbd_engine::Model;
use kbd_model::TransformGroup;

use super::compare::{Case, Press, engine_run};
use super::{OPTIONS, Subject};
use crate::build::linux::xkb::Value;
use crate::build::linux::xkb::dead_keys::{DECIMAL_SHOWN, show};
use crate::build::windows::kbdl::adapter::starts_with_marker;

/// A reported feature that explains a difference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Exception {
    LongOutput,
    MixedOutput,
    WrongState,
    DeadKeyWithoutKeysym,
    DeadKeyResult,
    TextTransform,
    LongDecimal,
}

/// Every explanation, with the feature of `ldml.linux.classify` it is.
// [spec:kbdgen:req:ldml.linux.test.typing+1]
pub const LISTED: [(Exception, &str); 7] = [
    (
        Exception::LongOutput,
        "outputs of more than one scalar value (ldml.linux.values)",
    ),
    (
        Exception::MixedOutput,
        "outputs mixing text and markers (ldml.linux.values)",
    ),
    (
        Exception::WrongState,
        "states no key type gives (ldml.linux.types)",
    ),
    (
        Exception::DeadKeyWithoutKeysym,
        "dead keys without a dead keysym (ldml.linux.dead-keys)",
    ),
    (
        Exception::DeadKeyResult,
        "dead-key results the reference Compose table gives otherwise (ldml.linux.dead-keys)",
    ),
    (
        Exception::TextTransform,
        "transforms that do not start with a marker (ldml.linux.rewrites)",
    ),
    (
        Exception::LongDecimal,
        "a decimal of more than one scalar value (ldml.linux.decimal)",
    ),
];

fn value(subject: &Subject, press: Press) -> Option<&Value> {
    subject
        .adapted
        .levels
        .keys
        .iter()
        .find(|key| key.key.name == press.name)
        .and_then(|key| key.values.get(press.state.index()))
}

fn wrong(subject: &Subject, press: Press) -> bool {
    let adapted = &subject.adapted;
    adapted
        .levels
        .keys
        .iter()
        .zip(&adapted.typed)
        .any(|(key, typed)| key.key.name == press.name && typed.wrong.contains(&press.state))
}

/// The engine on the keyboard without its text transforms.
fn without_transforms(subject: &Subject) -> Option<Model> {
    let mut keyboard = subject.layout.keyboard.clone();
    for group in &mut keyboard.simple {
        if let TransformGroup::Rules(rules) = group {
            rules.retain(|rule| starts_with_marker(&rule.from));
        }
    }
    keyboard
        .simple
        .retain(|group| !matches!(group, TransformGroup::Rules(rules) if rules.is_empty()));
    let len = keyboard.computed_context_len().ok()?;
    keyboard.context_len = u8::try_from(len).ok()?;
    if keyboard == subject.layout.keyboard {
        return None;
    }
    Model::from_keyboard(keyboard, OPTIONS).ok()
}

/// Whether a dead-key path, from a dead key with a dead keysym, has a
/// prefix the adapter lists among its dead-key results.
fn dead_key_result(subject: &Subject, path: &[Press]) -> bool {
    let shown: Vec<Option<String>> = path
        .iter()
        .map(|press| {
            if press.is_decimal() {
                Some(DECIMAL_SHOWN.to_owned())
            } else {
                value(subject, *press).map(|value| show(&subject.layout.keyboard, value))
            }
        })
        .collect();
    subject.adapted.dead.differences.iter().any(|difference| {
        difference.path.len() <= shown.len()
            && difference
                .path
                .iter()
                .zip(&shown)
                .all(|(step, shown)| shown.as_ref() == Some(step))
    })
}

/// The exception that explains the XKB layout typing what `case` records,
/// if any.
pub fn explain(subject: &Subject, case: &Case) -> Option<Exception> {
    let path = &case.path;
    let values: Vec<Option<&Value>> = path.iter().map(|press| value(subject, *press)).collect();
    if values.iter().any(|v| matches!(v, Some(Value::Long(_)))) {
        return Some(Exception::LongOutput);
    }
    if values.iter().any(|v| matches!(v, Some(Value::Mixed))) {
        return Some(Exception::MixedOutput);
    }
    if path.iter().any(|press| wrong(subject, *press)) {
        return Some(Exception::WrongState);
    }
    let without = &subject.adapted.dead.without;
    if values
        .iter()
        .any(|v| matches!(v, Some(Value::Dead(marker)) if without.contains(marker)))
    {
        return Some(Exception::DeadKeyWithoutKeysym);
    }
    if path.len() > 1 && dead_key_result(subject, path) {
        return Some(Exception::DeadKeyResult);
    }
    if let Some(model) = without_transforms(subject) {
        let uses_level3 = subject.adapted.levels.uses_level3;
        if engine_run(&model, path, uses_level3).is_some_and(|(text, _)| text == case.xkb) {
            return Some(Exception::TextTransform);
        }
    }
    let decimal = subject.adapted.decimal.as_ref();
    if path.iter().any(|press| press.is_decimal()) && decimal.is_some_and(|d| d.long.is_some()) {
        return Some(Exception::LongDecimal);
    }
    None
}
