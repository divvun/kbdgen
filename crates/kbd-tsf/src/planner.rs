//! The planner (`tsf.test.host`): turns an engine `Action` and the
//! context's mode into the text operations of `tsf.edit.apply`,
//! `tsf.edit.inject` and `tsf.edit.preedit`. It makes no TSF calls.

use kbd_engine::{Action, Context};

/// How an edit can reach the context.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Mode {
    /// The context was read and is not transitory (`tsf.edit.session`).
    pub authoritative: bool,
    /// A keyboard-disabled or empty context, such as a password field
    /// (`tsf.security.disabled`).
    pub disabled: bool,
}

/// One text operation of an edit, in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// Replace `units` UTF-16 units before the caret with `text`.
    Replace { units: usize, text: String },
    /// Show this preedit as the composition; empty ends it.
    Preedit(String),
    /// Insert `text` at the selection with `SetText`.
    Insert(String),
    /// Send Backspaces and Unicode presses with `SendInput`.
    Inject { backspaces: usize, text: String },
}

/// The UTF-16 length of the last `delete` scalar values of `context`: the
/// units before the caret that an edit deleting `delete` scalar values
/// removes. It never splits a surrogate pair, because it counts whole
/// scalar values of the text the engine was given.
// [spec:kbdgen:thm:tsf.edit.units]
pub fn units(context: &str, delete: usize) -> usize {
    context
        .chars()
        .rev()
        .take(delete)
        .map(char::len_utf16)
        .sum()
}

/// The steps that apply `action`, computed for `context`, in `mode`.
///
/// In an authoritative context the deletion and insertion are one range
/// replacement, followed by the preedit. Elsewhere an edit with nothing to
/// delete is a `SetText` at the selection, and one that deletes is sent
/// whole through `SendInput`: mixing the two would apply the insertion
/// before the queued deletions. No preedit is shown there, nor in a
/// disabled context.
// [spec:kbdgen:def:tsf.edit.ops]
// [spec:kbdgen:req:tsf.edit.apply+1]
// [spec:kbdgen:req:tsf.edit.inject+1]
// [spec:kbdgen:req:tsf.edit.preedit+1]
// [spec:kbdgen:req:tsf.security.disabled+1]
// [spec:kbdgen:def:tsf.engine.api]
pub fn plan(action: &Action, context: &Context, mode: Mode) -> Vec<Step> {
    let Action::Edit {
        delete,
        insert,
        preedit,
        ..
    } = action
    else {
        return Vec::new();
    };
    let mut steps = Vec::new();
    if mode.authoritative {
        let units = units(&context.text, *delete);
        if units > 0 || !insert.is_empty() {
            steps.push(Step::Replace {
                units,
                text: insert.clone(),
            });
        }
        let shown = if mode.disabled { "" } else { preedit.as_str() };
        steps.push(Step::Preedit(shown.to_owned()));
    } else if *delete > 0 {
        steps.push(Step::Inject {
            backspaces: *delete,
            text: insert.clone(),
        });
    } else if !insert.is_empty() {
        steps.push(Step::Insert(insert.clone()));
    }
    steps
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn edit(delete: usize, insert: &str, preedit: &str) -> Action {
        Action::Edit {
            delete,
            insert: insert.to_owned(),
            preedit: preedit.to_owned(),
            layer: Some("ignored".to_owned()),
        }
    }

    const STORE: Mode = Mode {
        authoritative: true,
        disabled: false,
    };
    const TRANSITORY: Mode = Mode {
        authoritative: false,
        disabled: false,
    };

    // [spec:kbdgen:req:tsf.edit.apply+1/test]
    // [spec:kbdgen:def:tsf.edit.ops/test]
    #[test]
    fn authoritative_edit_replaces_then_shows_preedit() {
        let context = Context::new("xa");
        assert_eq!(
            plan(&edit(1, "a\u{308}\u{303}", "´"), &context, STORE),
            [
                Step::Replace {
                    units: 1,
                    text: "a\u{308}\u{303}".to_owned()
                },
                Step::Preedit("´".to_owned()),
            ]
        );
        assert_eq!(
            plan(&edit(0, "", "´"), &context, STORE),
            [Step::Preedit("´".to_owned())]
        );
    }

    // [spec:kbdgen:req:tsf.edit.inject+1/test]
    #[test]
    fn transitory_edit_injects_or_sets_text() {
        let context = Context::default();
        assert_eq!(
            plan(&edit(2, "ʠ", "´"), &context, TRANSITORY),
            [Step::Inject {
                backspaces: 2,
                text: "ʠ".to_owned()
            }]
        );
        assert_eq!(
            plan(&edit(0, "b\u{301}", "´"), &context, TRANSITORY),
            [Step::Insert("b\u{301}".to_owned())]
        );
        assert!(plan(&edit(0, "", "´"), &context, TRANSITORY).is_empty());
        assert!(plan(&Action::Pass, &context, STORE).is_empty());
    }

    // [spec:kbdgen:req:tsf.security.disabled+1/test]
    // [spec:kbdgen:req:tsf.edit.preedit+1/test]
    #[test]
    fn disabled_context_drops_preedit() {
        let mode = Mode {
            authoritative: true,
            disabled: true,
        };
        assert_eq!(
            plan(&edit(0, "a", "´"), &Context::new("x"), mode),
            [
                Step::Replace {
                    units: 0,
                    text: "a".to_owned()
                },
                Step::Preedit(String::new()),
            ]
        );
    }

    // [spec:kbdgen:thm:tsf.edit.units/test]
    #[test]
    fn units_count_whole_scalars_from_end() {
        assert_eq!(units("ab𝕫", 1), 2);
        assert_eq!(units("ab𝕫", 2), 3);
        assert_eq!(units("ab𝕫", 9), 4);
        assert_eq!(units("", 0), 0);
    }

    proptest! {
        // [spec:kbdgen:thm:tsf.edit.units/test]
        #[test]
        fn units_never_split_a_surrogate_pair(
            text in "\\PC{0,12}",
            delete in 0usize..16,
        ) {
            let delete = delete.min(text.chars().count());
            let units_before: Vec<u16> = text.encode_utf16().collect();
            let n = units(&text, delete);
            let keep = &units_before[..units_before.len() - n];
            let kept = String::from_utf16(keep).expect("whole scalars remain");
            prop_assert_eq!(kept.chars().count() + delete, text.chars().count());
            prop_assert!(text.starts_with(&kept));
        }
    }
}
