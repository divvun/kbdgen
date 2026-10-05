//! Transform groups (`ldml.engine.transforms`) and replacement
//! (`ldml.engine.replace`).

use alloc::vec::Vec;

use kbd_model::{
    InvariantError, Keyboard, Normalization, Replacement, ReplacementItem, Site, TextElem,
    TransformGroup, TransformList,
};

use crate::matcher::{self, Match, RuleInfo};
use crate::normalize::nfd_elems;
use crate::reorder::reorder;

/// What the engine precomputes for a transform group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum GroupInfo {
    /// One entry per rule, in rule order.
    Rules(Vec<RuleInfo>),
    Reorder,
}

/// The precomputed infos of one transform list, aligned with its groups.
pub(crate) fn group_infos(
    keyboard: &Keyboard,
    list: TransformList,
) -> Result<Vec<GroupInfo>, InvariantError> {
    let groups = match list {
        TransformList::Simple => &keyboard.simple,
        TransformList::Backspace => &keyboard.backspace,
    };
    groups
        .iter()
        .enumerate()
        .map(|(g, group)| match group {
            TransformGroup::Reorder(_) => Ok(GroupInfo::Reorder),
            TransformGroup::Rules(rules) => rules
                .iter()
                .enumerate()
                .map(|(r, rule)| {
                    RuleInfo::new(keyboard, &rule.from).map_err(|e| InvariantError {
                        invariant: e.invariant,
                        site: Site::Rule {
                            list,
                            group: g,
                            rule: r,
                        },
                    })
                })
                .collect::<Result<Vec<_>, _>>()
                .map(GroupInfo::Rules),
        })
        .collect()
}

// [spec:kbdgen:sem:ldml.engine.replace]
/// The expansion of `to` for match `m` of `c`. `Text` gives its elements;
/// `Group(n)` the elements of capture *n*, markers included, or nothing if
/// it did not participate; `MapSet` the item of `to` at the index of the
/// first item of `from` equal to the capture.
pub(crate) fn expand(
    keyboard: &Keyboard,
    to: &Replacement,
    c: &[TextElem],
    m: &Match,
) -> Vec<TextElem> {
    let captured = |n: u8| -> &[TextElem] {
        m.groups
            .get(usize::from(n))
            .copied()
            .flatten()
            .and_then(|(a, b)| c.get(a..b))
            .unwrap_or(&[])
    };
    let mut out = Vec::new();
    for item in to {
        match item {
            ReplacementItem::Text(text) => out.extend_from_slice(text.elements()),
            ReplacementItem::Group(n) => out.extend_from_slice(captured(*n)),
            ReplacementItem::MapSet { group, from, to } => {
                let capture = captured(*group);
                let sets = &keyboard.sets;
                let index = sets
                    .get(usize::from(*from))
                    .and_then(|items| items.iter().position(|t| t.elements() == capture));
                if let Some(target) = index
                    .zip(sets.get(usize::from(*to)))
                    .and_then(|(i, items)| items.get(i))
                {
                    out.extend_from_slice(target.elements());
                }
            }
        }
    }
    out
}

// [spec:kbdgen:sem:ldml.engine.transforms]
/// Runs `groups` in order over `c`, each on the result of the previous
/// one: a `Rules` group applies its first rule that matches the end of
/// `c`, replacing exactly the matched elements, and a `Reorder` group
/// reorders. With normalization `Enabled`, `c` is normalized after each
/// group. At most one rule applies per group and processing never loops
/// back. Returns whether any rule of any `Rules` group matched.
pub(crate) fn run_groups(
    keyboard: &Keyboard,
    groups: &[TransformGroup],
    infos: &[GroupInfo],
    c: &mut Vec<TextElem>,
    at_start: bool,
) -> bool {
    let mut matched = false;
    for (group, info) in groups.iter().zip(infos) {
        match (group, info) {
            (TransformGroup::Rules(rules), GroupInfo::Rules(infos)) => {
                let hit = rules.iter().zip(infos).find_map(|(rule, info)| {
                    matcher::find(keyboard, &rule.from, info, c, at_start).map(|m| (rule, m))
                });
                if let Some((rule, m)) = hit {
                    let replacement = expand(keyboard, &rule.to, c, &m);
                    c.truncate(m.start);
                    c.extend(replacement);
                    matched = true;
                }
            }
            (TransformGroup::Reorder(rules), _) => *c = reorder(c, rules),
            (TransformGroup::Rules(_), GroupInfo::Reorder) => {}
        }
        if keyboard.normalization == Normalization::Enabled {
            *c = nfd_elems(c);
        }
    }
    matched
}
