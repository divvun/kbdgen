//! Transform groups: patterns and replacements (`ldml.xml.from`,
//! `ldml.xml.to`, `ldml.xml.resolve` step 8).

use kbd_model::{
    Class, ClassRange, Pattern, ReplacementItem, Rule, Text, TransformGroup, TransformList,
    TreeAtom, TreeItem,
};

use super::Ctx;
use crate::diag::Result;
use crate::escape::Piece;
use crate::nfd::{is_nfd_char, nfd_str};
use crate::special::{Generated, check_attrs, kbdgen_children};
use crate::syntax::{
    Atom, ClassMember, ClassSyntax, PatternSyntax, Quantified, ToItem, normalize_ranges,
    parse_pattern, parse_replacement, substitute_strings,
};
use crate::tree::El;

// [spec:kbdgen:req:ldml.xml.nfd-classes+1]
/// With normalization enabled, a class may list only NFD scalar values
/// (`ldml.xml.nfd-classes`): a listed non-NFD value is an error. A range
/// that merely contains non-NFD values warns, and those values are
/// removed, since the model's classes hold only NFD values and such values
/// never occur in NFD text.
pub(super) fn nfd_ranges(
    ctx: &mut Ctx,
    el: &El,
    attribute: &str,
    ranges: Vec<ClassRange>,
    listed_is_error: bool,
) -> Result<Vec<ClassRange>> {
    if !ctx.enabled() {
        return Ok(ranges);
    }
    let mut out = Vec::new();
    for r in ranges {
        if r.lo == r.hi {
            if is_nfd_char(r.lo) {
                out.push(r);
            } else if listed_is_error {
                return Err(el.attr_error(
                    attribute,
                    format!(
                        "U+{:04X} is not NFD and may not be listed in a class",
                        u32::from(r.lo)
                    ),
                ));
            } else {
                ctx.warn(
                    el.warning(format!(
                        "U+{:04X} is not NFD and never matches; removed",
                        u32::from(r.lo)
                    ))
                    .at(attribute),
                );
            }
            continue;
        }
        let mut stripped = false;
        let mut start: Option<char> = None;
        for c in r.lo..=r.hi {
            match (is_nfd_char(c), start) {
                (true, None) => start = Some(c),
                (false, Some(s)) => {
                    out.push(ClassRange { lo: s, hi: prev(c) });
                    start = None;
                    stripped = true;
                }
                (false, None) => stripped = true,
                (true, Some(_)) => {}
            }
        }
        if let Some(s) = start {
            out.push(ClassRange { lo: s, hi: r.hi });
        }
        if stripped {
            ctx.warn(
                el.warning(format!(
                    "range U+{:04X}-U+{:04X} contains values that are not NFD; they are removed",
                    u32::from(r.lo),
                    u32::from(r.hi)
                ))
                .at(attribute),
            );
        }
    }
    Ok(normalize_ranges(out))
}

fn prev(c: char) -> char {
    match c {
        '\u{E000}' => '\u{D7FF}',
        _ => char::from_u32(u32::from(c).saturating_sub(1)).unwrap_or(c),
    }
}

fn class_atom(ctx: &mut Ctx, el: &El, class: &ClassSyntax) -> Result<TreeAtom> {
    let mut ranges = Vec::new();
    let mut markers = Vec::new();
    let mut any_marker = false;
    for member in &class.members {
        match member {
            ClassMember::Range(lo, hi) => ranges.push(ClassRange { lo: *lo, hi: *hi }),
            ClassMember::Marker(m) => {
                let i = ctx.marker(el, m)?;
                if !markers.contains(&i) {
                    markers.push(i);
                }
            }
            ClassMember::AnyMarker => any_marker = true,
        }
    }
    let ranges = nfd_ranges(ctx, el, "from", normalize_ranges(ranges), true)?;
    if class.negated || !any_marker {
        let i = ctx.class_index(
            el,
            Class {
                ranges,
                negated: class.negated,
                markers,
            },
        )?;
        return Ok(TreeAtom::Class(i));
    }
    if ranges.is_empty() && markers.is_empty() {
        return Ok(TreeAtom::AnyMarker);
    }
    let i = ctx.class_index(
        el,
        Class {
            ranges,
            negated: false,
            markers,
        },
    )?;
    Ok(TreeAtom::Group(vec![
        vec![TreeItem::one(TreeAtom::AnyMarker)],
        vec![TreeItem::one(TreeAtom::Class(i))],
    ]))
}

fn var_atom(ctx: &mut Ctx, el: &El, id: &str) -> Result<TreeAtom> {
    if let Some(items) = ctx.vars.set(id) {
        let texts = items
            .iter()
            .map(|item| ctx.text(el, item))
            .collect::<Result<Vec<Text>>>()?;
        return Ok(TreeAtom::Set(ctx.set_index(el, texts)?));
    }
    if let Some(ranges) = ctx.vars.uset(id) {
        let ranges = nfd_ranges(ctx, el, "from", ranges, true)?;
        let i = ctx.class_index(
            el,
            Class {
                ranges,
                negated: false,
                markers: Vec::new(),
            },
        )?;
        return Ok(TreeAtom::Class(i));
    }
    Err(el.attr_error(
        "from",
        format!("$[{id}] names no set or uset (a string is written ${{{id}}})"),
    ))
}

fn sequence(ctx: &mut Ctx, el: &El, items: &[Quantified]) -> Result<Vec<TreeItem>> {
    let mut out = Vec::with_capacity(items.len());
    for q in items {
        let atom = match &q.atom {
            Atom::Char(c) => TreeAtom::Char(*c),
            Atom::Any => TreeAtom::Any,
            Atom::Fixed(f) => TreeAtom::Fixed(*f),
            Atom::Marker(m) => TreeAtom::Marker(ctx.marker(el, m)?),
            Atom::AnyMarker => TreeAtom::AnyMarker,
            Atom::Class(class) => class_atom(ctx, el, class)?,
            Atom::Var(id) => var_atom(ctx, el, id)?,
            Atom::Group(alternatives) => TreeAtom::Group(alternation(ctx, el, alternatives)?),
            Atom::Capture(items) => TreeAtom::Capture(vec![sequence(ctx, el, items)?]),
        };
        out.push(TreeItem {
            atom,
            min: q.min,
            max: q.max,
        });
    }
    if ctx.enabled() {
        out = nfd_sequence(out);
    }
    Ok(out)
}

fn alternation(
    ctx: &mut Ctx,
    el: &El,
    alternatives: &[Vec<Quantified>],
) -> Result<Vec<Vec<TreeItem>>> {
    alternatives.iter().map(|s| sequence(ctx, el, s)).collect()
}

fn chars(s: &str) -> impl Iterator<Item = TreeItem> + '_ {
    s.chars().map(|c| TreeItem::one(TreeAtom::Char(c)))
}

/// Puts each maximal run of unquantified `Char` items in NFD; a quantified
/// `Char` that is not NFD becomes a quantified group of its decomposition.
fn nfd_sequence(items: Vec<TreeItem>) -> Vec<TreeItem> {
    let mut out = Vec::with_capacity(items.len());
    let mut run = String::new();
    for item in items {
        match item.atom {
            TreeAtom::Char(c) if item.min == 1 && item.max == 1 => {
                run.push(c);
                continue;
            }
            TreeAtom::Char(c) if !is_nfd_char(c) => {
                out.extend(chars(&nfd_str(&std::mem::take(&mut run))));
                out.push(TreeItem {
                    atom: TreeAtom::Group(vec![chars(&nfd_str(&c.to_string())).collect()]),
                    ..item
                });
                continue;
            }
            _ => {}
        }
        out.extend(chars(&nfd_str(&std::mem::take(&mut run))));
        out.push(item);
    }
    out.extend(chars(&nfd_str(&run)));
    out
}

/// The capture groups of a pattern in opening order.
fn captures(alternatives: &[Vec<Quantified>]) -> Vec<&Vec<Quantified>> {
    let mut out = Vec::new();
    for q in alternatives.iter().flatten() {
        match &q.atom {
            Atom::Capture(items) => out.push(items),
            Atom::Group(inner) => out.extend(captures(inner)),
            _ => {}
        }
    }
    out
}

fn replacement(ctx: &mut Ctx, el: &El, syntax: &PatternSyntax) -> Result<Vec<ReplacementItem>> {
    let value = el.attr("to").unwrap_or("");
    let items = parse_replacement(value, &|id| ctx.vars.string_pieces(id))
        .map_err(|e| el.attr_error("to", e.to_string()))?;
    let groups = captures(&syntax.alternatives);
    let mut out = Vec::new();
    let mut run: Vec<Piece> = Vec::new();
    for item in items {
        let next = match item {
            ToItem::Char(c) => {
                run.push(Piece::Char(c));
                continue;
            }
            ToItem::Marker(m) => {
                run.push(Piece::Marker(m));
                continue;
            }
            ToItem::Group(n) => {
                if usize::from(n) > groups.len() {
                    return Err(el.attr_error("to", format!("${n} names a group that from lacks")));
                }
                ReplacementItem::Group(n)
            }
            ToItem::MapSet { group, set } => {
                let captured = groups.get(usize::from(group) - 1).ok_or_else(|| {
                    el.attr_error(
                        "to",
                        format!("$[{group}:{set}] names a group that from lacks"),
                    )
                })?;
                let from_id = match captured.as_slice() {
                    [
                        Quantified {
                            atom: Atom::Var(id),
                            min: 1,
                            max: 1,
                        },
                    ] if ctx.vars.set(id).is_some() => id.clone(),
                    _ => {
                        return Err(el.attr_error(
                            "to",
                            format!(
                                "$[{group}:{set}] needs capture {group} to hold exactly one set"
                            ),
                        ));
                    }
                };
                let TreeAtom::Set(from) = var_atom(ctx, el, &from_id)? else {
                    return Err(el.attr_error("to", "a mapped capture holds a set"));
                };
                let to_items = ctx.vars.set(&set).ok_or_else(|| {
                    el.attr_error(
                        "to",
                        format!("$[{group}:{set}]: {set} is not a set (usets are never mapped)"),
                    )
                })?;
                let TreeAtom::Set(to) = var_atom(ctx, el, &set)? else {
                    return Err(el.attr_error("to", "a mapped set is a set"));
                };
                let from_len = ctx.kb.sets.get(usize::from(from)).map_or(0, Vec::len);
                if from_len != to_items.len() {
                    return Err(el.attr_error(
                        "to",
                        format!(
                            "{from_id} has {from_len} items but {set} has {}",
                            to_items.len()
                        ),
                    ));
                }
                ReplacementItem::MapSet { group, from, to }
            }
        };
        if !run.is_empty() {
            out.push(ReplacementItem::Text(
                ctx.text(el, &std::mem::take(&mut run))?,
            ));
        }
        out.push(next);
    }
    if !run.is_empty() {
        out.push(ReplacementItem::Text(ctx.text(el, &run)?));
    }
    Ok(out)
}

fn rule(ctx: &mut Ctx, el: &El) -> Result<Rule> {
    let raw = el.attr("from").unwrap_or("");
    let substituted = substitute_strings(raw, &|id| ctx.vars.string_raw(id))
        .map_err(|e| el.attr_error("from", e.to_string()))?;
    let syntax = parse_pattern(&substituted).map_err(|e| el.attr_error("from", e.to_string()))?;
    let tree = alternation(ctx, el, &syntax.alternatives)?;
    let from = Pattern::from_tree(syntax.anchored, tree)
        .map_err(|i| el.attr_error("from", i.description()))?;
    let info = from
        .analyze(&ctx.kb.sets, ctx.kb.classes.len(), ctx.kb.markers.len())
        .map_err(|e| el.attr_error("from", e.invariant.description()))?;
    if info.min_len == Some(0) {
        return Err(el.attr_error("from", "the pattern can match the empty text"));
    }
    let to = replacement(ctx, el, &syntax)?;
    Ok(Rule { from, to })
}

// [spec:kbdgen:syn:ldml.xml.to]
/// Builds `simple` and `backspace` in document order, and records the
/// groups that kbdgen's v4 lowering marked as generated.
pub(super) fn transforms(ctx: &mut Ctx, root: &El) -> Result<()> {
    for transforms in root.children_named("transforms") {
        let list = match transforms.attr("type") {
            Some("backspace") => TransformList::Backspace,
            _ => TransformList::Simple,
        };
        for (index, group_el) in transforms.children_named("transformGroup").enumerate() {
            let prefix = ctx.kbdgen.clone();
            for mark in kbdgen_children(group_el, prefix.as_deref()) {
                if mark.name != "generated" {
                    return Err(mark.error(format!(
                        "{} is not allowed in transformGroup",
                        mark.qualified()
                    )));
                }
                check_attrs(mark, &["by"], &["by"])?;
                ctx.extensions.generated.push(Generated {
                    list,
                    group: index,
                    by: mark.attr("by").unwrap_or("").to_string(),
                });
            }
            let group = if group_el.children_named("reorder").next().is_some() {
                TransformGroup::Reorder(super::reorder::group(ctx, group_el)?)
            } else {
                let rules = group_el
                    .children_named("transform")
                    .map(|t| rule(ctx, t))
                    .collect::<Result<Vec<_>>>()?;
                TransformGroup::Rules(rules)
            };
            match list {
                TransformList::Simple => ctx.kb.simple.push(group),
                TransformList::Backspace => ctx.kb.backspace.push(group),
            }
        }
    }
    Ok(())
}
