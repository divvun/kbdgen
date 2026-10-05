//! Reorder groups: parsing, LDML's split-and-merge, and the priority sort
//! (`ldml.xml.resolve` step 9, §Using `<import>` with `<reorder>`).

use kbd_model::{ClassRange, ReorderClass, ReorderRule};

use super::Ctx;
use super::transforms::nfd_ranges;
use crate::diag::Result;
use crate::nfd::is_nfd_char;
use crate::syntax::{
    ReorderElem, complement_ranges, intersect_ranges, parse_reorder, parse_reorder_bools,
    parse_reorder_ints, substitute_strings,
};
use crate::tree::El;

/// A rule before merging. An attribute the element does not give is
/// `None`: merging takes the later rule's values only where it defines
/// them.
#[derive(Debug, Clone, PartialEq)]
struct Draft {
    before: Vec<ReorderClass>,
    from: Vec<ReorderClass>,
    order: Option<Vec<i8>>,
    tertiary: Option<Vec<i8>>,
    tertiary_base: Option<Vec<bool>>,
    pre_base: Option<Vec<bool>>,
}

fn ranges(class: &ReorderClass) -> Vec<ClassRange> {
    match class {
        ReorderClass::Char(c) => vec![ClassRange::single(*c)],
        ReorderClass::Ranges(r) => r.clone(),
    }
}

/// A class from ranges, kept as `Char` when `like` was that character.
fn class_like(like: &ReorderClass, ranges: Vec<ClassRange>) -> Option<ReorderClass> {
    match (like, ranges.as_slice()) {
        (_, []) => None,
        (ReorderClass::Char(c), [r]) if r.lo == *c && r.hi == *c => Some(ReorderClass::Char(*c)),
        _ => Some(ReorderClass::Ranges(ranges)),
    }
}

impl Draft {
    /// Every position of before, then from: the rule's set of strings is
    /// the product of these classes.
    fn positions(&self) -> Vec<&ReorderClass> {
        self.before.iter().chain(&self.from).collect()
    }

    fn with_positions(&self, classes: Vec<ReorderClass>) -> Draft {
        let mut classes = classes;
        let from = classes.split_off(self.before.len());
        Draft {
            before: classes,
            from,
            ..self.clone()
        }
    }

    fn same_shape(&self, other: &Draft) -> bool {
        self.before.len() == other.before.len() && self.from.len() == other.from.len()
    }

    fn intersection(&self, other: &Draft) -> Option<Draft> {
        if !self.same_shape(other) {
            return None;
        }
        let classes = self
            .positions()
            .into_iter()
            .zip(other.positions())
            .map(|(a, b)| class_like(a, intersect_ranges(&ranges(a), &ranges(b))))
            .collect::<Option<Vec<_>>>()?;
        Some(self.with_positions(classes))
    }

    /// `self` minus `other` as disjoint products: for each position k, the
    /// strings that agree with `other` before k and leave it at k.
    fn minus(&self, other: &Draft) -> Vec<Draft> {
        if self.intersection(other).is_none() {
            return vec![self.clone()];
        }
        let mine = self.positions();
        let theirs = other.positions();
        let mut out = Vec::new();
        for k in 0..mine.len() {
            let mut classes = Vec::with_capacity(mine.len());
            let mut empty = false;
            for (i, (a, b)) in mine.iter().zip(&theirs).enumerate() {
                let r = match i.cmp(&k) {
                    std::cmp::Ordering::Less => intersect_ranges(&ranges(a), &ranges(b)),
                    std::cmp::Ordering::Equal => {
                        intersect_ranges(&ranges(a), &complement_ranges(&ranges(b)))
                    }
                    std::cmp::Ordering::Greater => ranges(a),
                };
                match class_like(a, r) {
                    Some(c) => classes.push(c),
                    None => {
                        empty = true;
                        break;
                    }
                }
            }
            if !empty {
                out.push(self.with_positions(classes));
            }
        }
        out
    }

    /// The intersection of `earlier` and `later` takes `later`'s values
    /// where `later` defines them, else `earlier`'s.
    fn merged(&self, earlier: &Draft, later: &Draft) -> Draft {
        Draft {
            order: later.order.clone().or_else(|| earlier.order.clone()),
            tertiary: later.tertiary.clone().or_else(|| earlier.tertiary.clone()),
            tertiary_base: later
                .tertiary_base
                .clone()
                .or_else(|| earlier.tertiary_base.clone()),
            pre_base: later.pre_base.clone().or_else(|| earlier.pre_base.clone()),
            ..self.clone()
        }
    }

    fn finish(self) -> ReorderRule {
        let n = self.from.len();
        ReorderRule {
            order: self.order.unwrap_or_else(|| vec![0; n]),
            tertiary: self.tertiary.unwrap_or_else(|| vec![0; n]),
            tertiary_base: self.tertiary_base.unwrap_or_else(|| vec![false; n]),
            pre_base: self.pre_base.unwrap_or_else(|| vec![false; n]),
            before: self.before,
            from: self.from,
        }
    }
}

/// Splits and merges rules in document order. The pieces kept so far are
/// pairwise disjoint, so each new rule overrides exactly its intersection
/// with each of them, and what remains of it is added.
fn merge(rules: Vec<Draft>) -> Vec<Draft> {
    let mut out: Vec<Draft> = Vec::new();
    for later in rules {
        let mut next = Vec::with_capacity(out.len() + 1);
        let mut left = vec![later.clone()];
        for earlier in out {
            match earlier.intersection(&later) {
                None => next.push(earlier),
                Some(both) => {
                    next.push(both.merged(&earlier, &later));
                    next.extend(earlier.minus(&later));
                    left = left
                        .iter()
                        .flat_map(|piece| piece.minus(&earlier))
                        .collect();
                }
            }
        }
        next.extend(left);
        out = next;
    }
    out
}

fn classes(ctx: &mut Ctx, el: &El, attribute: &str) -> Result<Vec<ReorderClass>> {
    let raw = el.attr(attribute).unwrap_or("");
    let substituted = substitute_strings(raw, &|id| ctx.vars.string_raw(id))
        .map_err(|e| el.attr_error(attribute, e.to_string()))?;
    let elems = parse_reorder(&substituted, &|id| ctx.vars.uset(id))
        .map_err(|e| el.attr_error(attribute, e.to_string()))?;
    let mut out = Vec::with_capacity(elems.len());
    for elem in elems {
        out.push(match elem {
            ReorderElem::Char(c) => {
                if ctx.enabled() && !is_nfd_char(c) {
                    return Err(el.attr_error(
                        attribute,
                        format!(
                            "U+{:04X} is not NFD; reorder operates on NFD text",
                            u32::from(c)
                        ),
                    ));
                }
                ReorderClass::Char(c)
            }
            ReorderElem::Ranges(r) => {
                let r = nfd_ranges(ctx, el, attribute, r, false)?;
                if r.is_empty() {
                    return Err(el.attr_error(attribute, "an element matches no NFD character"));
                }
                ReorderClass::Ranges(r)
            }
        });
    }
    Ok(out)
}

fn draft(ctx: &mut Ctx, el: &El) -> Result<Draft> {
    let from = classes(ctx, el, "from")?;
    if from.is_empty() {
        return Err(el.attr_error("from", "reorder@from is empty"));
    }
    let before = classes(ctx, el, "before")?;
    let n = from.len();
    let ints = |a: &str| -> Result<Option<Vec<i8>>> {
        el.attr(a)
            .map(|v| parse_reorder_ints(Some(v), n).map_err(|e| el.attr_error(a, e.message)))
            .transpose()
    };
    let bools = |a: &str| -> Result<Option<Vec<bool>>> {
        el.attr(a)
            .map(|v| parse_reorder_bools(Some(v), n).map_err(|e| el.attr_error(a, e.message)))
            .transpose()
    };
    let draft = Draft {
        before,
        from,
        order: ints("order")?,
        tertiary: ints("tertiary")?,
        tertiary_base: bools("tertiaryBase")?,
        pre_base: bools("preBase")?,
    };
    let zeros = vec![0; n];
    let falses = vec![false; n];
    let order = draft.order.as_ref().unwrap_or(&zeros);
    let tertiary = draft.tertiary.as_ref().unwrap_or(&zeros);
    for i in 0..n {
        let o = order.get(i).copied().unwrap_or(0);
        let t = tertiary.get(i).copied().unwrap_or(0);
        let base = draft
            .tertiary_base
            .as_ref()
            .unwrap_or(&falses)
            .get(i)
            .copied()
            .unwrap_or(false);
        let pre = draft
            .pre_base
            .as_ref()
            .unwrap_or(&falses)
            .get(i)
            .copied()
            .unwrap_or(false);
        if t != 0 && o != 0 {
            return Err(el.attr_error("tertiary", "a tertiary character must have order 0"));
        }
        if t != 0 && (base || pre) {
            return Err(el.error("a tertiary character may not be tertiaryBase or preBase"));
        }
        if pre && o == 0 {
            return Err(el.attr_error("preBase", "a prebase character needs a non-zero order"));
        }
    }
    Ok(draft)
}

// [spec:kbdgen:sem:ldml.xml.resolve]
/// A reorder group: rules parsed, split and merged, then sorted into match
/// priority, longest `from` then longest `before`, keeping document order
/// among equals.
pub(super) fn group(ctx: &mut Ctx, group: &El) -> Result<Vec<ReorderRule>> {
    let drafts = group
        .children_named("reorder")
        .map(|el| draft(ctx, el))
        .collect::<Result<Vec<_>>>()?;
    let mut rules: Vec<ReorderRule> = merge(drafts).into_iter().map(Draft::finish).collect();
    rules.sort_by_key(|rule| std::cmp::Reverse(rule.priority()));
    Ok(rules)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draft(from: Vec<ReorderClass>, order: Option<i8>, pre: Option<bool>) -> Draft {
        let n = from.len();
        Draft {
            before: vec![],
            from,
            order: order.map(|o| vec![o; n]),
            tertiary: None,
            tertiary_base: None,
            pre_base: pre.map(|p| vec![p; n]),
        }
    }

    fn span(lo: char, hi: char) -> ReorderClass {
        ReorderClass::Ranges(vec![ClassRange { lo, hi }])
    }

    // [spec:kbdgen:sem:ldml.xml.resolve/test]
    #[test]
    fn later_rule_overrides_only_the_intersection() {
        // The Myanmar example of §Using <import> with <reorder>: an
        // imported [U+1031 U+1084] order 30, then a local U+1031 preBase.
        let imported = draft(
            vec![ReorderClass::Ranges(vec![
                ClassRange::single('\u{1031}'),
                ClassRange::single('\u{1084}'),
            ])],
            Some(30),
            None,
        );
        let local = draft(vec![ReorderClass::Char('\u{1031}')], None, Some(true));
        let merged: Vec<ReorderRule> = merge(vec![imported, local])
            .into_iter()
            .map(Draft::finish)
            .collect();
        assert_eq!(merged.len(), 2);
        assert_eq!(
            merged[0].from,
            [ReorderClass::Ranges(vec![ClassRange::single('\u{1031}')])]
        );
        assert_eq!((merged[0].order[0], merged[0].pre_base[0]), (30, true));
        assert_eq!(
            merged[1].from,
            [ReorderClass::Ranges(vec![ClassRange::single('\u{1084}')])]
        );
        assert_eq!((merged[1].order[0], merged[1].pre_base[0]), (30, false));
    }

    #[test]
    fn disjoint_or_differently_shaped_rules_stay() {
        let a = draft(vec![span('a', 'c')], Some(1), None);
        let b = draft(vec![span('d', 'f')], Some(2), None);
        let c = draft(vec![span('a', 'z'), span('a', 'z')], Some(3), None);
        let merged = merge(vec![a.clone(), b.clone(), c.clone()]);
        assert_eq!(merged, vec![a, b, c]);
    }

    #[test]
    fn partial_overlap_splits_into_three() {
        let a = draft(vec![span('a', 'm')], Some(1), None);
        let b = draft(vec![span('h', 'z')], Some(2), None);
        let merged: Vec<ReorderRule> = merge(vec![a, b]).into_iter().map(Draft::finish).collect();
        let summary: Vec<(Vec<ClassRange>, i8)> = merged
            .iter()
            .map(|r| (ranges(&r.from[0]), r.order[0]))
            .collect();
        assert_eq!(
            summary,
            vec![
                (vec![ClassRange { lo: 'h', hi: 'm' }], 2),
                (vec![ClassRange { lo: 'a', hi: 'g' }], 1),
                (vec![ClassRange { lo: 'n', hi: 'z' }], 2),
            ]
        );
        let again = merge(
            merged
                .iter()
                .map(|r| Draft {
                    before: r.before.clone(),
                    from: r.from.clone(),
                    order: Some(r.order.clone()),
                    tertiary: None,
                    tertiary_base: None,
                    pre_base: None,
                })
                .collect(),
        );
        assert_eq!(
            again.len(),
            3,
            "merged rules are disjoint and merge to themselves"
        );
    }
}
