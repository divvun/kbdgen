//! Which features of a keyboard rewrite committed text
//! (`ldml.linux.rewrites`): only through them can an edit's `delete` be
//! non-zero, since a pending marker is never committed text. kbdgen
//! reports them for the native layouts that cannot reproduce them, and the
//! IBus engine holds text back for a model that has them where it cannot
//! delete.

use alloc::vec::Vec;

use crate::{Atom, Item, Keyboard, Normalization, Pattern, ReplacementItem, Rule, TransformGroup};

/// Whether every alternative of `pattern` must start with a marker, as a
/// dead-key transform does.
pub fn starts_with_marker(pattern: &Pattern) -> bool {
    pattern.nodes.first().is_some_and(|top| {
        top.alternatives.iter().all(|sequence| {
            sequence.first().is_some_and(|item| {
                item.min >= 1 && matches!(item.atom, Atom::Marker(_) | Atom::AnyMarker)
            })
        })
    })
}

/// Whether `rule` is the backspace rule `ldml.yaml.dead-keys.backspace`
/// generates: `\m{.}` with no replacement.
pub fn is_generated_backspace(rule: &Rule) -> bool {
    let only_any_marker = match rule.from.nodes.as_slice() {
        [top] => top.alternatives.as_slice() == [[Item::one(Atom::AnyMarker)]],
        _ => false,
    };
    !rule.from.anchored
        && only_any_marker
        && rule
            .to
            .iter()
            .all(|item| matches!(item, ReplacementItem::Text(text) if text.is_empty()))
}

/// A feature by which a keyboard rewrites committed text. Groups and
/// rules are numbered from 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rewrite {
    /// A `simple` rule that does not start with a marker.
    TextTransform {
        group: usize,
        rule: usize,
    },
    Reorder {
        group: usize,
        rules: usize,
    },
    /// A `backspace` rule other than the generated one.
    BackspaceRule {
        group: usize,
        rule: usize,
    },
    BackspaceReorder {
        group: usize,
    },
    Normalization,
}

impl Keyboard {
    /// The features by which the keyboard rewrites committed text, in
    /// order: `simple` groups, `backspace` groups, then normalization.
    // [spec:kbdgen:def:ldml.linux.rewrites]
    pub fn rewrites(&self) -> Vec<Rewrite> {
        let mut rewrites = Vec::new();
        for (group, transforms) in self.simple.iter().enumerate() {
            match transforms {
                TransformGroup::Rules(rules) => rewrites.extend(
                    rules
                        .iter()
                        .enumerate()
                        .filter(|(_, rule)| !starts_with_marker(&rule.from))
                        .map(|(rule, _)| Rewrite::TextTransform { group, rule }),
                ),
                TransformGroup::Reorder(rules) => rewrites.push(Rewrite::Reorder {
                    group,
                    rules: rules.len(),
                }),
            }
        }
        for (group, transforms) in self.backspace.iter().enumerate() {
            match transforms {
                TransformGroup::Rules(rules) => rewrites.extend(
                    rules
                        .iter()
                        .enumerate()
                        .filter(|(_, rule)| !is_generated_backspace(rule))
                        .map(|(rule, _)| Rewrite::BackspaceRule { group, rule }),
                ),
                TransformGroup::Reorder(_) => rewrites.push(Rewrite::BackspaceReorder { group }),
            }
        }
        if self.normalization == Normalization::Enabled {
            rewrites.push(Rewrite::Normalization);
        }
        rewrites
    }
}
