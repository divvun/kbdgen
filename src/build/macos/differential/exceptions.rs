//! The documented exceptions of `ldml.macos.test.typing`: where a
//! `.keylayout` may type something other than the engine, each traced to a
//! feature `ldml.macos.classify` reports.

use kbd_model::{Keyboard, Normalization, Text, TransformGroup};

use super::compare::{Outcome, Press, engine_run};
use super::{Subject, macos_model};
use crate::build::windows::kbdl::adapter::starts_with_marker;

/// A documented difference between the `.keylayout` and the engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Exception {
    TextTransform,
    Reorder,
    Normalization,
    MixedOutput,
    DeadKeyResult,
}

/// Every exception the rule lists, with where it is documented.
// [spec:kbdgen:req:ldml.macos.test.typing]
pub const LISTED: [(Exception, &str); 5] = [
    (
        Exception::TextTransform,
        "transforms that do not start with a marker (ldml.macos.classify)",
    ),
    (Exception::Reorder, "reorder groups (ldml.macos.classify)"),
    (
        Exception::Normalization,
        "an enabled normalization (ldml.macos.classify)",
    ),
    (
        Exception::MixedOutput,
        "outputs mixing text and markers, which have no key (ldml.macos.keys)",
    ),
    (
        Exception::DeadKeyResult,
        "dead-key results leaving text and a marker, or several markers (ldml.macos.dead-keys)",
    ),
];

/// The keyboard without the feature an ablating exception names.
fn ablate(keyboard: &Keyboard, exception: Exception) -> Option<Keyboard> {
    let mut keyboard = keyboard.clone();
    match exception {
        Exception::TextTransform => {
            for group in &mut keyboard.simple {
                if let TransformGroup::Rules(rules) = group {
                    rules.retain(|rule| starts_with_marker(&rule.from));
                }
            }
            keyboard
                .simple
                .retain(|group| !matches!(group, TransformGroup::Rules(rules) if rules.is_empty()));
        }
        Exception::Reorder => keyboard
            .simple
            .retain(|group| !matches!(group, TransformGroup::Reorder(_))),
        Exception::Normalization => keyboard.normalization = Normalization::Disabled,
        Exception::MixedOutput => {
            for key in &mut keyboard.keys {
                if !key.output.is_plain() && key.output.len() > 1 {
                    key.output = Text::new();
                }
            }
        }
        Exception::DeadKeyResult => return None,
    }
    let len = keyboard.computed_context_len().ok()?;
    keyboard.context_len = u8::try_from(len).ok()?;
    Some(keyboard)
}

/// Whether the engine's last press commits text and keeps a marker, or
/// keeps several, where the `.keylayout` keeps none.
fn dead_key_result(subject: &Subject, path: &[Press], keylayout: &Outcome) -> bool {
    let Some((outcome, markers)) = engine_run(&subject.model, path) else {
        return false;
    };
    let before = engine_run(&subject.model, &path[..path.len() - 1])
        .map(|(o, _)| o.text.chars().count())
        .unwrap_or(0);
    let committed = outcome.text.chars().count() > before;
    markers.len() > 1 || (markers.len() == 1 && committed && keylayout.pending.is_none())
}

/// The exception that explains the `.keylayout` typing `keylayout` on
/// `path`, if any: the engine, with that feature removed from the model,
/// types the same; or, for dead-key results, the engine's last press left
/// what no keylayout state can hold.
pub fn explain(subject: &Subject, path: &[Press], keylayout: &Outcome) -> Option<Exception> {
    LISTED
        .iter()
        .map(|(e, _)| *e)
        .find(|exception| match ablate(&subject.keyboard, *exception) {
            Some(keyboard) => {
                keyboard != subject.keyboard
                    && engine_run(&macos_model(&keyboard), path)
                        .is_some_and(|(o, _)| o == *keylayout)
            }
            None => dead_key_result(subject, path, keylayout),
        })
}
