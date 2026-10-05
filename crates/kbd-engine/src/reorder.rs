//! Reorder groups (`ldml.engine.reorder`): UTS #35 Part 7 §Element:
//! reorder, run with the marker algorithm of §Reorder and Markers.

use alloc::vec::Vec;

use kbd_model::{ReorderClass, ReorderRule, TextElem};

use crate::normalize::{Glued, reglue, unglue};

/// The ordering attributes a rule gives one character.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Weights {
    order: i8,
    tertiary: i8,
    tertiary_base: bool,
    pre_base: bool,
}

impl Weights {
    /// A base: primary and tertiary 0. It starts a run.
    fn is_base(self) -> bool {
        self.order == 0 && self.tertiary == 0
    }

    /// A prebase primary character, part of a run's prefix. A tertiary
    /// character or a base is never a prebase.
    fn is_prebase(self) -> bool {
        self.pre_base && self.tertiary == 0 && !self.is_base()
    }
}

fn classes_match(classes: &[ReorderClass], chars: &[char]) -> bool {
    classes.len() == chars.len() && classes.iter().zip(chars).all(|(k, c)| k.contains(*c))
}

/// The weights of each character. At each position the first rule in
/// priority order applies whose `from` matches there and whose `before`
/// matches just before it; it covers all of its `from` characters, and
/// scanning resumes after them. Unmatched characters are bases.
fn assign(chars: &[char], rules: &[ReorderRule]) -> Vec<Weights> {
    let mut weights = Vec::with_capacity(chars.len());
    let mut pos = 0;
    while pos < chars.len() {
        let rule = rules.iter().find(|r| {
            let end = pos.saturating_add(r.from.len());
            let from = chars.get(pos..end);
            let before = pos
                .checked_sub(r.before.len())
                .and_then(|start| chars.get(start..pos));
            matches!((from, before), (Some(f), Some(b))
                if classes_match(&r.from, f) && classes_match(&r.before, b))
        });
        match rule {
            Some(r) if !r.from.is_empty() => {
                for j in 0..r.from.len() {
                    weights.push(Weights {
                        order: r.order.get(j).copied().unwrap_or(0),
                        tertiary: r.tertiary.get(j).copied().unwrap_or(0),
                        tertiary_base: r.tertiary_base.get(j).copied().unwrap_or(false),
                        pre_base: r.pre_base.get(j).copied().unwrap_or(false),
                    });
                }
                pos = pos.saturating_add(r.from.len());
            }
            _ => {
                weights.push(Weights::default());
                pos = pos.saturating_add(1);
            }
        }
    }
    weights
}

/// The sort key (primary, index, tertiary, quaternary) of each character.
///
/// A primary character (tertiary 0) is keyed by its own order and index.
/// A tertiary character takes the order and index of the most recent
/// tertiary base, a primary character marked `tertiary_base` or with
/// order 0; with none before it, it keeps order 0 and its own index.
fn sort_keys(weights: &[Weights]) -> Vec<(i8, usize, i8, usize)> {
    let mut last_base: Option<(i8, usize)> = None;
    let mut keys = Vec::with_capacity(weights.len());
    for (j, w) in weights.iter().enumerate() {
        if w.tertiary == 0 {
            keys.push((w.order, j, 0, j));
            if w.tertiary_base || w.order == 0 {
                last_base = Some((w.order, j));
            }
        } else {
            let (order, index) = last_base.unwrap_or((0, j));
            keys.push((order, index, w.tertiary, j));
        }
    }
    keys
}

/// The runs `preBase* base ((primary≠0 || tertiary≠0) && !preBase)*`, as
/// index ranges covering every character in order. Characters before the
/// first base that do not form a prefix make up a run of their own, and
/// no filler base is inserted for a prefix without a base
/// (`ldml.scope.deferred`).
fn runs(weights: &[Weights]) -> Vec<core::ops::Range<usize>> {
    #[derive(PartialEq)]
    enum Phase {
        Empty,
        Prefix,
        Body,
    }
    let mut runs = Vec::new();
    let mut start = 0;
    let mut phase = Phase::Empty;
    for (j, w) in weights.iter().enumerate() {
        let new_run = if w.is_prebase() || w.is_base() {
            phase == Phase::Body
        } else {
            false
        };
        if new_run {
            runs.push(start..j);
            start = j;
        }
        phase = if w.is_prebase() {
            Phase::Prefix
        } else {
            Phase::Body
        };
    }
    if start < weights.len() {
        runs.push(start..weights.len());
    }
    runs
}

// [spec:kbdgen:sem:ldml.engine.reorder+1]
// [spec:kbdgen:req:ldml.scope.deferred]
/// Runs a reorder group over the whole of `elems`: removes the markers,
/// sorts each run of the plain text by its sort keys, and re-adds the
/// markers, each glued to the scalar value it preceded.
pub(crate) fn reorder(elems: &[TextElem], rules: &[ReorderRule]) -> Vec<TextElem> {
    let (glued, end) = unglue(elems);
    let chars: Vec<char> = glued.iter().map(|g| g.c).collect();
    let weights = assign(&chars, rules);
    let keys = sort_keys(&weights);
    let mut order: Vec<usize> = Vec::with_capacity(chars.len());
    for run in runs(&weights) {
        let mut indices: Vec<usize> = run.collect();
        indices.sort_by_key(|&j| keys.get(j).copied());
        order.extend(indices);
    }
    let mut slots: Vec<Option<Glued>> = glued.into_iter().map(Some).collect();
    let sorted: Vec<Glued> = order
        .into_iter()
        .filter_map(|j| slots.get_mut(j).and_then(Option::take))
        .collect();
    reglue(sorted, end)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use kbd_model::ClassRange;

    fn rule(before: &[ReorderClass], from: &[ReorderClass], order: &[i8]) -> ReorderRule {
        let n = from.len();
        let mut order = order.to_vec();
        while order.len() < n {
            order.push(*order.last().unwrap_or(&0));
        }
        ReorderRule {
            before: before.to_vec(),
            from: from.to_vec(),
            order,
            tertiary: vec![0; n],
            tertiary_base: vec![false; n],
            pre_base: vec![false; n],
        }
    }

    fn range(lo: u32, hi: u32) -> ReorderClass {
        ReorderClass::Ranges(vec![ClassRange {
            lo: char::from_u32(lo).unwrap(),
            hi: char::from_u32(hi).unwrap(),
        }])
    }

    fn sort(mut rules: Vec<ReorderRule>) -> Vec<ReorderRule> {
        rules.sort_by_key(|r| core::cmp::Reverse(r.priority()));
        rules
    }

    fn plain(s: &[u32]) -> Vec<TextElem> {
        s.iter()
            .map(|&x| TextElem::Char(char::from_u32(x).unwrap()))
            .collect()
    }

    /// The Northern Thai (Tai Tham) rules of §Element: reorder.
    fn tai_tham() -> Vec<ReorderRule> {
        let tones = || range(0x1A75, 0x1A79);
        sort(vec![
            rule(&[], &[ReorderClass::Char('\u{1A60}')], &[127]),
            rule(&[], &[ReorderClass::Char('\u{1A6B}')], &[42]),
            rule(&[], &[tones()], &[55]),
            rule(
                &[ReorderClass::Char('\u{1A6B}')],
                &[
                    ReorderClass::Char('\u{1A60}'),
                    ReorderClass::Char('\u{1A45}'),
                ],
                &[10],
            ),
            rule(
                &[ReorderClass::Char('\u{1A6B}'), tones()],
                &[
                    ReorderClass::Char('\u{1A60}'),
                    ReorderClass::Char('\u{1A45}'),
                ],
                &[10],
            ),
            rule(
                &[ReorderClass::Char('\u{1A6B}')],
                &[
                    ReorderClass::Char('\u{1A60}'),
                    tones(),
                    ReorderClass::Char('\u{1A45}'),
                ],
                &[10, 55, 10],
            ),
        ])
    }

    // [spec:kbdgen:sem:ldml.engine.reorder+1/test]
    #[test]
    fn tai_tham_typing_orders_converge() {
        let rules = tai_tham();
        let wanted = plain(&[0x1A21, 0x1A60, 0x1A45, 0x1A6B, 0x1A76]);
        for typed in [
            [0x1A21, 0x1A6B, 0x1A76, 0x1A60, 0x1A45],
            [0x1A21, 0x1A6B, 0x1A60, 0x1A76, 0x1A45],
            [0x1A21, 0x1A6B, 0x1A60, 0x1A45, 0x1A76],
        ] {
            assert_eq!(reorder(&plain(&typed), &rules), wanted, "{typed:x?}");
        }
        assert_eq!(reorder(&wanted, &rules), wanted);
    }

    /// The Myanmar fragment of §Using `<import>` with `<reorder>` elements,
    /// as merged: e-vowel and medial-r become prebases.
    fn myanmar() -> Vec<ReorderRule> {
        let mut e_vowel = rule(&[], &[ReorderClass::Char('\u{1031}')], &[30]);
        e_vowel.pre_base = vec![true];
        let mut medial_r = rule(&[], &[ReorderClass::Char('\u{103C}')], &[20]);
        medial_r.pre_base = vec![true];
        sort(vec![
            medial_r,
            rule(
                &[],
                &[ReorderClass::Ranges(vec![
                    ClassRange::single('\u{103D}'),
                    ClassRange::single('\u{1082}'),
                ])],
                &[25],
            ),
            rule(
                &[],
                &[ReorderClass::Ranges(vec![
                    ClassRange::single('\u{103E}'),
                    ClassRange::single('\u{1060}'),
                ])],
                &[27],
            ),
            e_vowel,
            rule(&[], &[ReorderClass::Char('\u{1084}')], &[30]),
            rule(
                &[],
                &[
                    ReorderClass::Char('\u{1004}'),
                    ReorderClass::Char('\u{103A}'),
                    ReorderClass::Char('\u{1039}'),
                ],
                &[-1],
            ),
        ])
    }

    // [spec:kbdgen:sem:ldml.engine.reorder+1/test]
    #[test]
    fn myanmar_prebases_follow_their_base() {
        let rules = myanmar();
        // e-vowel, medial-r, ka typed visually → ka, medial-r, e-vowel.
        assert_eq!(
            reorder(&plain(&[0x1031, 0x103C, 0x1000]), &rules),
            plain(&[0x1000, 0x103C, 0x1031])
        );
        // Each prefix attaches to the base after it.
        assert_eq!(
            reorder(&plain(&[0x1031, 0x1000, 0x1031, 0x1001]), &rules),
            plain(&[0x1000, 0x1031, 0x1001, 0x1031])
        );
        // Kinzi (order -1) moves before the base of its run.
        assert_eq!(
            reorder(&plain(&[0x1000, 0x1004, 0x103A, 0x1039]), &rules),
            plain(&[0x1004, 0x103A, 0x1039, 0x1000])
        );
        // A shan-e-vowel is not a prebase, so it stays after its base.
        assert_eq!(
            reorder(&plain(&[0x1000, 0x1084, 0x103D]), &rules),
            plain(&[0x1000, 0x103D, 0x1084])
        );
    }

    // [spec:kbdgen:req:ldml.scope.deferred/test]
    #[test]
    fn lone_prebase_gets_no_filler_base() {
        let rules = myanmar();
        let typed = plain(&[0x1031]);
        assert_eq!(reorder(&typed, &rules), typed);
        let typed = plain(&[0x1000, 0x1031]);
        assert_eq!(reorder(&typed, &rules), typed);
    }

    // [spec:kbdgen:sem:ldml.engine.reorder+1/test]
    #[test]
    fn markers_move_with_their_characters() {
        let rules = myanmar();
        let input = vec![
            TextElem::Marker(0),
            TextElem::Char('\u{1031}'),
            TextElem::Char('\u{1000}'),
            TextElem::Marker(1),
        ];
        assert_eq!(
            reorder(&input, &rules),
            [
                TextElem::Char('\u{1000}'),
                TextElem::Marker(0),
                TextElem::Char('\u{1031}'),
                TextElem::Marker(1),
            ]
        );
    }

    // [spec:kbdgen:sem:ldml.engine.reorder+1/test]
    #[test]
    fn tertiary_sorts_after_its_base() {
        // Devanagari: a nukta typed after a vowel sign moves back to the
        // consonant, its tertiary base.
        let mut nukta = rule(&[], &[ReorderClass::Char('\u{093C}')], &[0]);
        nukta.tertiary = vec![1];
        let rules = sort(vec![
            nukta,
            rule(&[], &[ReorderClass::Char('\u{0940}')], &[10]),
        ]);
        assert_eq!(
            reorder(&plain(&[0x0915, 0x0940, 0x093C]), &rules),
            plain(&[0x0915, 0x093C, 0x0940])
        );
        // Halant + consonant joins the run of the first consonant.
        let conjunct = rule(
            &[],
            &[ReorderClass::Char('\u{094D}'), range(0x0915, 0x0939)],
            &[2],
        );
        let rules = sort(vec![
            conjunct,
            rule(&[], &[ReorderClass::Char('\u{0940}')], &[10]),
        ]);
        assert_eq!(
            reorder(&plain(&[0x0915, 0x0940, 0x094D, 0x0937]), &rules),
            plain(&[0x0915, 0x094D, 0x0937, 0x0940])
        );
    }

    #[test]
    fn runs_split_before_each_prefix_or_base() {
        let base = Weights::default();
        let pre = Weights {
            order: 5,
            pre_base: true,
            ..Weights::default()
        };
        let mark = Weights {
            order: 5,
            ..Weights::default()
        };
        assert_eq!(runs(&[mark, pre, base, mark, base]), [0..1, 1..4, 4..5]);
        assert_eq!(runs(&[pre, pre, mark, base]), [0..3, 3..4]);
        assert!(runs(&[]).is_empty());
    }
}
