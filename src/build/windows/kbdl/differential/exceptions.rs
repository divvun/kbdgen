//! The documented exceptions of `tsf.test.differential`: where the layout
//! DLL may type something other than the engine, and how a difference is
//! traced to one of them.

use indexmap::IndexMap;
use kbd_engine::{Action, Context, Model, State};
use kbd_model::{Normalization, TextElem, TransformGroup};

use super::super::adapter::{is_generated_backspace, starts_with_marker};
use super::super::input::{DeadKeyNode, KeyValue, Layer, POSITION_NAMES, STANDALONE};
use super::super::tables::{CAPLOK, SGCAPS, VK_DEAD_ROW};
use super::Subject;
use super::compare::{Outcome, Press, dll_outcome, engine_run};
use super::simulate::{Dll, Translated, Typed};
use crate::ldml::migrate::oracle::CapsState;

/// A documented difference between the DLL and the engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Exception {
    CapsShiftCaplok,
    CtrlLayer,
    LongDeadKeyLeaf,
    TextTransform,
    Reorder,
    BackspaceRule,
    MixedOutput,
    Normalization,
    DroppedPosition,
    SgcapsDeadKey,
    MigratedCaps,
    UnmatchedDeadKeys,
    LigatureAfterDeadKey,
}

/// Every exception the rule lists, with where it is documented.
// [spec:kbdgen:req:tsf.test.differential]
pub const LISTED: [(Exception, &str); 13] = [
    (
        Exception::CapsShiftCaplok,
        "Caps+Shift on CAPLOK keys gives the unshifted value (ldml.kbdl.caps)",
    ),
    (
        Exception::CtrlLayer,
        "the ctrl layer, which the engine passes (ldml.engine.shortcuts)",
    ),
    (
        Exception::LongDeadKeyLeaf,
        "dead-key leaves, standalone outputs included, of more than one UTF-16 unit (ldml.kbdl.classify)",
    ),
    (
        Exception::TextTransform,
        "transforms that do not start with a marker (ldml.kbdl.classify)",
    ),
    (Exception::Reorder, "reorder groups (ldml.kbdl.classify)"),
    (
        Exception::BackspaceRule,
        "backspace rules other than the generated one (ldml.kbdl.classify); no case presses Backspace",
    ),
    (
        Exception::MixedOutput,
        "outputs mixing text and markers (ldml.kbdl.classify)",
    ),
    (
        Exception::Normalization,
        "an enabled normalization (ldml.kbdl.classify)",
    ),
    (
        Exception::DroppedPosition,
        "positions ldml.kbdl.positions drops or warns about: scan codes outside the 49, which no case presses, and the space bar, whose row kbdl.vk-chars fixes to U+0020 in the default, shift and ctrl columns only",
    ),
    (
        Exception::SgcapsDeadKey,
        "the caps states of a key with a dead key whose caps value would need an SGCAPS row, which kbdl.caps.sgcaps warns about and drops",
    ),
    (
        Exception::MigratedCaps,
        "a v3 layout's caps states that its migration types differently from the v3 tables (ldml.kbdl.model-resource, M09 of ldml.migrate.caps-diff)",
    ),
    (
        Exception::UnmatchedDeadKeys,
        "a dead key pressed while another with no composition for it is pending: Windows types both dead characters (kbdl.dead-keys), where the engine keeps the second pending (ldml.kbdl.dead-tree)",
    ),
    (
        Exception::LigatureAfterDeadKey,
        "a ligature typed while a dead key is pending: Windows composes the dead key with the ligature's first unit (kbdl.ligatures), which the engine's dead-key transforms do not",
    ),
];

/// The exception that explains `dll` differing from the engine on `path`,
/// if any. Paths are only continued while both sides agree
/// ([`super::compare::cases`]), so the difference comes from the last
/// press, which the exceptions read off a press are tried on first. The
/// text-processing exceptions hold when the engine, with that processing
/// removed from the model, types what the DLL types.
pub fn explain(subject: &Subject, path: &[Press], dll: &Outcome) -> Option<Exception> {
    let model = &subject.model;
    let last = *path.last()?;
    let alone = || differs_alone(subject, last);
    let checks: [(Exception, &dyn Fn() -> bool); 9] = [
        (Exception::CapsShiftCaplok, &|| {
            caps_shift_caplok(subject, last) && alone()
        }),
        (Exception::CtrlLayer, &|| ctrl_passes(model, last)),
        (Exception::LongDeadKeyLeaf, &|| long_leaf(subject, path)),
        (Exception::MixedOutput, &|| mixed_output(subject, last)),
        (Exception::DroppedPosition, &|| {
            last.position.is_none() && alone()
        }),
        (Exception::SgcapsDeadKey, &|| {
            sgcaps_dead_key(subject, last) && alone()
        }),
        (Exception::MigratedCaps, &|| {
            migrated_caps(subject, last) && alone()
        }),
        (Exception::UnmatchedDeadKeys, &|| {
            unmatched_dead_keys(subject, path)
        }),
        (Exception::LigatureAfterDeadKey, &|| {
            ligature_after_dead_key(subject, path)
        }),
    ];
    if let Some((exception, _)) = checks.iter().find(|(_, check)| check()) {
        return Some(*exception);
    }
    let mut present = Vec::new();
    for exception in [
        Exception::TextTransform,
        Exception::Reorder,
        Exception::BackspaceRule,
        Exception::Normalization,
    ] {
        let Some(stripped) = without(model, &[exception]) else {
            continue;
        };
        present.push(exception);
        if engine_run(&stripped, path).0 == *dll {
            return Some(exception);
        }
    }
    let stripped = without(model, &present)?;
    (engine_run(&stripped, path).0 == *dll)
        .then(|| present.first().copied())
        .flatten()
}

fn caps_shift_caplok(subject: &Subject, press: Press) -> bool {
    let dll = Dll::new(&subject.tables);
    let attributes = dll.attributes(dll.vk(press.scan()));
    press.layer == Layer::CapsShift && attributes & CAPLOK != 0 && attributes & SGCAPS == 0
}

fn ctrl_passes(model: &Model, press: Press) -> bool {
    let (action, _) = model.key(&State::default(), &Context::default(), &press.event());
    press.layer == Layer::Ctrl && action == Action::Pass
}

/// Whether the engine and the DLL type `press` differently from the reset
/// state, so that the press itself, not the path to it, differs.
fn differs_alone(subject: &Subject, press: Press) -> bool {
    let single = [press];
    engine_run(&subject.model, &single).0 != dll_outcome(&mut Dll::new(&subject.tables), &single)
}

/// A caps-state press on a key with a dead key whose caps value differs
/// from both its default and shift values, which would need an SGCAPS row
/// where the dead row stands; its attributes are then computed as if it had
/// no caps values, AltGr+Caps included.
fn sgcaps_dead_key(subject: &Subject, press: Press) -> bool {
    let Some(position) = press.position else {
        return false;
    };
    let dll = Dll::new(&subject.tables);
    let has_dead_row = dll
        .row(dll.vk(press.scan()))
        .and_then(|row| subject.tables.rows.get(row + 1))
        .is_some_and(|next| next.vk == VK_DEAD_ROW);
    let at = |layer: Layer| {
        subject
            .input
            .layers
            .get(&layer)
            .and_then(|values| values[position].clone())
    };
    let caps = at(Layer::Caps);
    matches!(press.layer, Layer::Caps | Layer::CapsShift | Layer::AltCaps)
        && has_dead_row
        && caps.is_some()
        && caps != at(Layer::Default)
        && caps != at(Layer::Shift)
}

/// A caps-state press of a v3 layout at a position its migration lists as
/// typing differently on Windows (M09).
fn migrated_caps(subject: &Subject, press: Press) -> bool {
    let (Some(migration), Some(position)) = (&subject.migration, press.position) else {
        return false;
    };
    let state = match press.layer {
        Layer::Caps => CapsState::Caps,
        Layer::CapsShift => CapsState::CapsShift,
        Layer::AltCaps => CapsState::AltCaps,
        _ => return false,
    };
    migration.caps_diffs.iter().any(|diff| {
        diff.platform == "windows"
            && diff.state == state
            && diff.positions.contains(&POSITION_NAMES[position])
    })
}

/// A path whose last press is a dead key that the DLL types, with the dead
/// character pending before it, because no composition joins the two.
fn unmatched_dead_keys(subject: &Subject, path: &[Press]) -> bool {
    let Some((last, prefix)) = path.split_last() else {
        return false;
    };
    let mut dll = Dll::new(&subject.tables);
    dll_outcome(&mut dll, prefix);
    let vk = dll.vk(last.scan());
    let Translated::Dead(id) = dll.translate(vk, last.keys()) else {
        return false;
    };
    let pending = dll.pending();
    pending.is_some()
        && dll.press(vk, last.keys()) == Typed::Text([pending.unwrap_or(0), id].to_vec())
}

/// A path whose last press is a ligature that composes with the dead
/// character pending before it.
fn ligature_after_dead_key(subject: &Subject, path: &[Press]) -> bool {
    let Some((last, prefix)) = path.split_last() else {
        return false;
    };
    let mut dll = Dll::new(&subject.tables);
    dll_outcome(&mut dll, prefix);
    let vk = dll.vk(last.scan());
    let Translated::Ligature(units) = dll.translate(vk, last.keys()) else {
        return false;
    };
    let Some(dead) = dll.pending() else {
        return false;
    };
    dll.press(vk, last.keys()) != Typed::Text([vec![dead], units].concat())
}

/// The value the DLL's input gives `press`; the space bar types U+0020.
fn value(subject: &Subject, press: Press) -> Option<KeyValue> {
    match press.position {
        None => Some(KeyValue::new(STANDALONE)),
        Some(position) => subject.input.layers.get(&press.layer)?[position].clone(),
    }
}

fn long(text: &str) -> bool {
    text.encode_utf16().count() > 1
}

/// A dead-key path whose last press reaches a leaf of more than one unit,
/// or falls back to a standalone output of more than one unit, which the
/// DLL omits or cannot use as the dead id.
fn long_leaf(subject: &Subject, path: &[Press]) -> bool {
    let tree = &subject.input.dead_key_tree;
    let branch = |value: &KeyValue| match (value.dead, tree.get(&value.text)) {
        (true, Some(DeadKeyNode::Branch(children))) => Some(children),
        _ => None,
    };
    let mut state: Option<&IndexMap<String, DeadKeyNode>> = None;
    let mut reached = false;
    for value in path.iter().filter_map(|press| value(subject, *press)) {
        reached = false;
        let Some(children) = state else {
            state = branch(&value);
            continue;
        };
        state = match children.get(&value.text) {
            Some(DeadKeyNode::Leaf(output)) => {
                reached = long(output);
                None
            }
            Some(DeadKeyNode::Branch(next)) => Some(next),
            None => {
                reached = matches!(children.get(STANDALONE), Some(DeadKeyNode::Leaf(s)) if long(s));
                branch(&value)
            }
        };
    }
    reached
}

/// A press the DLL has no key for while a hardware key at its scan code
/// mixes text and markers.
fn mixed_output(subject: &Subject, press: Press) -> bool {
    let keyboard = subject.model.keyboard();
    let Some(hardware) = &keyboard.hardware else {
        return false;
    };
    value(subject, press).is_none()
        && (0..hardware.layers.len()).any(|layer| {
            hardware
                .key_for_scan_code(layer, press.scan())
                .and_then(|key| keyboard.key(key))
                .is_some_and(|key| {
                    let elements = key.output.elements();
                    let markers = elements
                        .iter()
                        .filter(|e| matches!(e, TextElem::Marker(_)))
                        .count();
                    markers > 1 || (markers == 1 && elements.len() > 1)
                })
        })
}

/// The model without the processing of `exceptions`, or `None` when the
/// keyboard has none of it.
fn without(model: &Model, exceptions: &[Exception]) -> Option<Model> {
    let mut keyboard = model.keyboard().clone();
    let before = keyboard.clone();
    for exception in exceptions {
        match exception {
            Exception::TextTransform => {
                retain_rules(&mut keyboard.simple, |rule| starts_with_marker(&rule.from))
            }
            Exception::Reorder => keyboard
                .simple
                .retain(|group| matches!(group, TransformGroup::Rules(_))),
            Exception::BackspaceRule => {
                keyboard
                    .backspace
                    .retain(|group| matches!(group, TransformGroup::Rules(_)));
                retain_rules(&mut keyboard.backspace, is_generated_backspace);
            }
            Exception::Normalization => keyboard.normalization = Normalization::Disabled,
            _ => {}
        }
    }
    if keyboard == before {
        return None;
    }
    keyboard.context_len = u8::try_from(keyboard.computed_context_len().ok()?).ok()?;
    Model::from_keyboard(keyboard, model.options()).ok()
}

fn retain_rules(groups: &mut Vec<TransformGroup>, keep: impl Fn(&kbd_model::Rule) -> bool) {
    for group in groups.iter_mut() {
        if let TransformGroup::Rules(rules) = group {
            rules.retain(&keep);
        }
    }
    groups.retain(|group| !matches!(group, TransformGroup::Rules(rules) if rules.is_empty()));
}
