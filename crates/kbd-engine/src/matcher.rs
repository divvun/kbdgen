//! Pattern matching (`ldml.engine.match`).
//!
//! A pattern matches the end of the working context C. The match used is
//! the one an ECMAScript `u`-flag search for `(?:P)$` finds: the smallest
//! start, and at that start the first success in backtracking order.
//!
//! Patterns have no backreferences, so whether a sub-pattern can continue
//! from a position never depends on captures. The matcher therefore works
//! in two passes, both iterative so that no pattern depth can exhaust the
//! stack:
//!
//! 1. Bottom-up over the node arena (children have larger indices than
//!    their parents), it tabulates for every sequence suffix, quantifier
//!    count and start position the end positions reachable, in
//!    backtracking order with repeats dropped. Dropping a repeated end
//!    never changes the first success: the second derivation reaching an
//!    end continues exactly as the first did, and the first already
//!    failed or won.
//! 2. Top-down from the chosen start, it follows the first table entry
//!    that still reaches the required end, recording captures.
//!
//! Positions range over a window of `max_len + 1` ending at |C|, because a
//! match ends at |C| and spans at most `max_len` elements. Work per rule is
//! therefore polynomial: O(items × repeats × window²) table entries.

use alloc::vec;
use alloc::vec::Vec;

use kbd_model::{
    Atom, Fixed, Item, Keyboard, MAX_CAPTURES, Pattern, PatternError, PatternInfo, Sequence,
    TextElem,
};

/// What the engine precomputes for a rule's pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuleInfo {
    /// The fewest elements a match spans; none when nothing can match.
    pub(crate) min_len: Option<usize>,
    /// The most elements a match spans.
    pub(crate) max_len: usize,
    /// For each node, the bit set of capture numbers inside it.
    pub(crate) capture_masks: Vec<u16>,
}

impl RuleInfo {
    pub(crate) fn new(keyboard: &Keyboard, pattern: &Pattern) -> Result<RuleInfo, PatternError> {
        let PatternInfo {
            min_len, max_len, ..
        } = pattern.analyze(
            &keyboard.sets,
            keyboard.classes.len(),
            keyboard.markers.len(),
        )?;
        // Children follow their parents, so a reverse walk sees every
        // child's mask before its parent needs it.
        let mut capture_masks = vec![0u16; pattern.nodes.len()];
        for index in (0..pattern.nodes.len()).rev() {
            let mut mask = 0u16;
            for item in pattern
                .nodes
                .get(index)
                .iter()
                .flat_map(|n| n.alternatives.iter().flatten())
            {
                mask |= atom_mask(&capture_masks, item.atom);
            }
            if let Some(slot) = capture_masks.get_mut(index) {
                *slot = mask;
            }
        }
        Ok(RuleInfo {
            min_len,
            max_len,
            capture_masks,
        })
    }
}

/// The capture numbers inside `atom`, itself included.
fn atom_mask(masks: &[u16], atom: Atom) -> u16 {
    let child = |node: u16| masks.get(usize::from(node)).copied().unwrap_or(0);
    match atom {
        Atom::Group(node) => child(node),
        Atom::Capture { number, node } => {
            child(node) | 1u16.checked_shl(u32::from(number)).unwrap_or(0)
        }
        _ => 0,
    }
}

/// Captures 0 to 9 as element ranges of C; 0 is the whole match.
pub(crate) type Groups = [Option<(usize, usize)>; MAX_CAPTURES as usize + 1];

/// A successful match: it always ends at |C|.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Match {
    pub(crate) start: usize,
    pub(crate) groups: Groups,
}

/// Positions reachable, in backtracking order, without repeats.
type Ends = Vec<usize>;

fn push_unique(ends: &mut Ends, e: usize) {
    if !ends.contains(&e) {
        ends.push(e);
    }
}

/// Whether the fixed class `f` contains `c`, with LDML's fixed definitions
/// (ECMAScript's, which do not change with Unicode versions).
pub(crate) fn fixed_contains(f: Fixed, c: char) -> bool {
    let space = matches!(
        c,
        '\t' | '\n' | '\u{B}' | '\u{C}' | '\r' | ' ' | '\u{A0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200A}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202F}'
                | '\u{205F}'
                | '\u{3000}'
                | '\u{FEFF}'
    );
    let digit = c.is_ascii_digit();
    let word = c.is_ascii_alphanumeric() || c == '_';
    match f {
        Fixed::Space => space,
        Fixed::NotSpace => !space,
        Fixed::Digit => digit,
        Fixed::NotDigit => !digit,
        Fixed::Word => word,
        Fixed::NotWord => !word,
        Fixed::Tab => c == '\t',
        Fixed::CarriageReturn => c == '\r',
        Fixed::LineFeed => c == '\n',
        Fixed::FormFeed => c == '\u{C}',
        Fixed::VerticalTab => c == '\u{B}',
    }
}

/// The match tables of one pattern over one context.
struct Tables<'a> {
    keyboard: &'a Keyboard,
    pattern: &'a Pattern,
    c: &'a [TextElem],
    /// The first position of the window; the last is |C|.
    w0: usize,
    /// `node[n][p - w0]`: the ends of node `n` from `p`.
    node: Vec<Vec<Ends>>,
    /// `seq[n][a][i][p - w0]`: the ends of items `i..` of alternative `a`.
    seq: Vec<Vec<Vec<Vec<Ends>>>>,
    /// `rep[n][a][i][k][p - w0]`: the ends of item `i` after `k` iterations.
    rep: Vec<Vec<Vec<Vec<Vec<Ends>>>>>,
}

/// The entry for position `p` of a table over the window from `w0`; empty
/// outside the window.
fn at(table: &[Ends], w0: usize, p: usize) -> &[usize] {
    p.checked_sub(w0)
        .and_then(|i| table.get(i))
        .map_or(&[], Vec::as_slice)
}

impl<'a> Tables<'a> {
    fn positions(&self) -> core::ops::RangeInclusive<usize> {
        self.w0..=self.c.len()
    }

    /// The ends of one unquantified `atom` from `p`.
    fn atom_ends(&self, atom: Atom, p: usize) -> Ends {
        let elem = self.c.get(p).copied();
        let next = p.saturating_add(1);
        let one = |ok: bool| if ok { vec![next] } else { Vec::new() };
        match atom {
            Atom::Char(x) => one(elem == Some(TextElem::Char(x))),
            Atom::Any => one(matches!(elem, Some(TextElem::Char(_)))),
            Atom::Fixed(f) => one(matches!(elem, Some(TextElem::Char(c)) if fixed_contains(f, c))),
            Atom::Marker(m) => one(elem == Some(TextElem::Marker(m))),
            Atom::AnyMarker => one(matches!(elem, Some(TextElem::Marker(_)))),
            Atom::Class(i) => {
                let Some(class) = self.keyboard.classes.get(usize::from(i)) else {
                    return Vec::new();
                };
                one(match elem {
                    Some(TextElem::Char(c)) => {
                        class.ranges.iter().any(|r| r.contains(c)) != class.negated
                    }
                    Some(TextElem::Marker(m)) => !class.negated && class.markers.contains(&m),
                    None => false,
                })
            }
            Atom::Set(s) => {
                let mut ends = Vec::new();
                let rest = self.c.get(p..).unwrap_or(&[]);
                for text in self.keyboard.sets.get(usize::from(s)).into_iter().flatten() {
                    if rest.starts_with(text.elements()) {
                        push_unique(&mut ends, p.saturating_add(text.len()));
                    }
                }
                ends
            }
            Atom::Group(node) | Atom::Capture { node, .. } => self
                .node
                .get(usize::from(node))
                .map_or(Vec::new(), |t| at(t, self.w0, p).to_vec()),
        }
    }

    /// The tables of a quantified item, indexed `[k][p - w0]`: from `p`,
    /// after `k` iterations. Greedy: another iteration is tried before
    /// stopping. As in ECMAScript, an iteration past the minimum that
    /// matches the empty text fails.
    fn item_tables(&self, item: &Item) -> Vec<Vec<Ends>> {
        let min = usize::from(item.min);
        let max = usize::from(item.max);
        let atom: Vec<Ends> = self
            .positions()
            .map(|p| self.atom_ends(item.atom, p))
            .collect();
        let mut next: Vec<Ends> = self.positions().map(|p| vec![p]).collect();
        let mut by_k = vec![next.clone()];
        for k in (0..max).rev() {
            let current: Vec<Ends> = self
                .positions()
                .map(|p| {
                    let mut out = Vec::new();
                    for &e in at(&atom, self.w0, p) {
                        if k >= min && e == p {
                            continue;
                        }
                        for &f in at(&next, self.w0, e) {
                            push_unique(&mut out, f);
                        }
                    }
                    if k >= min {
                        push_unique(&mut out, p);
                    }
                    out
                })
                .collect();
            by_k.push(current.clone());
            next = current;
        }
        by_k.reverse();
        by_k
    }

    /// The suffix tables of one sequence, indexed `[i][p - w0]` for
    /// `i` in `0..=len`, and its item tables.
    fn sequence_tables(&self, sequence: &Sequence) -> (Vec<Vec<Ends>>, Vec<Vec<Vec<Ends>>>) {
        let mut suffix: Vec<Ends> = self.positions().map(|p| vec![p]).collect();
        let mut suffixes = vec![suffix.clone()];
        let mut items = Vec::with_capacity(sequence.len());
        for item in sequence.iter().rev() {
            let reps = self.item_tables(item);
            let first: &[Ends] = reps.first().map_or(&[], Vec::as_slice);
            let current: Vec<Ends> = self
                .positions()
                .map(|p| {
                    let mut out = Vec::new();
                    for &e in at(first, self.w0, p) {
                        for &f in at(&suffix, self.w0, e) {
                            push_unique(&mut out, f);
                        }
                    }
                    out
                })
                .collect();
            suffixes.push(current.clone());
            items.push(reps);
            suffix = current;
        }
        suffixes.reverse();
        items.reverse();
        (suffixes, items)
    }

    fn build(keyboard: &'a Keyboard, pattern: &'a Pattern, c: &'a [TextElem], w0: usize) -> Self {
        let count = pattern.nodes.len();
        let mut tables = Tables {
            keyboard,
            pattern,
            c,
            w0,
            node: vec![Vec::new(); count],
            seq: vec![Vec::new(); count],
            rep: vec![Vec::new(); count],
        };
        for index in (0..count).rev() {
            let Some(alternation) = pattern.nodes.get(index) else {
                continue;
            };
            let mut node_ends: Vec<Ends> = tables.positions().map(|_| Vec::new()).collect();
            let mut seqs = Vec::with_capacity(alternation.alternatives.len());
            let mut reps = Vec::with_capacity(alternation.alternatives.len());
            for sequence in &alternation.alternatives {
                let (suffixes, items) = tables.sequence_tables(sequence);
                if let Some(whole) = suffixes.first() {
                    for (out, ends) in node_ends.iter_mut().zip(whole) {
                        for &e in ends {
                            push_unique(out, e);
                        }
                    }
                }
                seqs.push(suffixes);
                reps.push(items);
            }
            if let (Some(n), Some(s), Some(r)) = (
                tables.node.get_mut(index),
                tables.seq.get_mut(index),
                tables.rep.get_mut(index),
            ) {
                *n = node_ends;
                *s = seqs;
                *r = reps;
            }
        }
        tables
    }

    fn node_ends(&self, node: usize, p: usize) -> &[usize] {
        self.node.get(node).map_or(&[], |t| at(t, self.w0, p))
    }

    fn seq_ends(&self, node: usize, alt: usize, i: usize, p: usize) -> &[usize] {
        self.seq
            .get(node)
            .and_then(|a| a.get(alt))
            .and_then(|s| s.get(i))
            .map_or(&[], |t| at(t, self.w0, p))
    }

    fn rep_ends(&self, node: usize, alt: usize, i: usize, k: usize, p: usize) -> &[usize] {
        self.rep
            .get(node)
            .and_then(|a| a.get(alt))
            .and_then(|s| s.get(i))
            .and_then(|r| r.get(k))
            .map_or(&[], |t| at(t, self.w0, p))
    }

    fn item(&self, node: usize, alt: usize, i: usize) -> Option<&Item> {
        self.pattern.nodes.get(node)?.alternatives.get(alt)?.get(i)
    }

    /// The captures of the first derivation, in backtracking order, of
    /// node 0 from `start` to `end`. Iterative, with tasks popped in
    /// left-to-right order.
    fn captures(&self, info: &RuleInfo, start: usize, end: usize) -> Groups {
        enum Task {
            Node {
                node: usize,
                p: usize,
                t: usize,
            },
            Seq {
                node: usize,
                alt: usize,
                i: usize,
                p: usize,
                t: usize,
            },
            Rep {
                node: usize,
                alt: usize,
                i: usize,
                k: usize,
                p: usize,
                t: usize,
            },
            Atom {
                atom: Atom,
                p: usize,
                t: usize,
            },
            Clear(u16),
        }
        let mut groups: Groups = [None; MAX_CAPTURES as usize + 1];
        if let Some(g) = groups.first_mut() {
            *g = Some((start, end));
        }
        let mut stack = vec![Task::Node {
            node: 0,
            p: start,
            t: end,
        }];
        while let Some(task) = stack.pop() {
            match task {
                Task::Node { node, p, t } => {
                    let alternatives = self
                        .pattern
                        .nodes
                        .get(node)
                        .map_or(0, |n| n.alternatives.len());
                    if let Some(alt) =
                        (0..alternatives).find(|&a| self.seq_ends(node, a, 0, p).contains(&t))
                    {
                        stack.push(Task::Seq {
                            node,
                            alt,
                            i: 0,
                            p,
                            t,
                        });
                    }
                }
                Task::Seq { node, alt, i, p, t } => {
                    if self.item(node, alt, i).is_none() {
                        continue;
                    }
                    let next = i.saturating_add(1);
                    if let Some(&e) = self
                        .rep_ends(node, alt, i, 0, p)
                        .iter()
                        .find(|&&e| self.seq_ends(node, alt, next, e).contains(&t))
                    {
                        stack.push(Task::Seq {
                            node,
                            alt,
                            i: next,
                            p: e,
                            t,
                        });
                        stack.push(Task::Rep {
                            node,
                            alt,
                            i,
                            k: 0,
                            p,
                            t: e,
                        });
                    }
                }
                Task::Rep {
                    node,
                    alt,
                    i,
                    k,
                    p,
                    t,
                } => {
                    let Some(item) = self.item(node, alt, i) else {
                        continue;
                    };
                    if k >= usize::from(item.max) {
                        continue;
                    }
                    let past_min = k >= usize::from(item.min);
                    let next = k.saturating_add(1);
                    let chosen = self.atom_ends(item.atom, p).into_iter().find(|&e| {
                        !(past_min && e == p) && self.rep_ends(node, alt, i, next, e).contains(&t)
                    });
                    if let Some(e) = chosen {
                        stack.push(Task::Rep {
                            node,
                            alt,
                            i,
                            k: next,
                            p: e,
                            t,
                        });
                        stack.push(Task::Atom {
                            atom: item.atom,
                            p,
                            t: e,
                        });
                        stack.push(Task::Clear(atom_mask(&info.capture_masks, item.atom)));
                    }
                }
                Task::Atom { atom, p, t } => match atom {
                    Atom::Group(node) => stack.push(Task::Node {
                        node: usize::from(node),
                        p,
                        t,
                    }),
                    Atom::Capture { number, node } => {
                        if let Some(g) = groups.get_mut(usize::from(number)) {
                            *g = Some((p, t));
                        }
                        stack.push(Task::Node {
                            node: usize::from(node),
                            p,
                            t,
                        });
                    }
                    _ => {}
                },
                Task::Clear(mask) => {
                    for (n, g) in groups.iter_mut().enumerate().skip(1) {
                        if (mask >> n) & 1 == 1 {
                            *g = None;
                        }
                    }
                }
            }
        }
        groups
    }
}

// [spec:kbdgen:sem:ldml.engine.match+1]
/// The match of `pattern` against the end of `c`, if any: the smallest
/// start, and at it the first success in backtracking order. `^` matches
/// only at start 0, and only when `at_start` says C begins at a start of
/// text. Scalar atoms never match a marker; `Marker(m)` and `AnyMarker`
/// match only markers, and a negated class never does.
pub(crate) fn find(
    keyboard: &Keyboard,
    pattern: &Pattern,
    info: &RuleInfo,
    c: &[TextElem],
    at_start: bool,
) -> Option<Match> {
    let n = c.len();
    if n < info.min_len? {
        return None;
    }
    let w0 = n.saturating_sub(info.max_len);
    if pattern.anchored && (!at_start || w0 > 0) {
        return None;
    }
    let tables = Tables::build(keyboard, pattern, c, w0);
    let last = if pattern.anchored { w0 } else { n };
    let start = (w0..=last).find(|&s| tables.node_ends(0, s).contains(&n))?;
    Some(Match {
        start,
        groups: tables.captures(info, start, n),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;
    use kbd_model::{Class, ClassRange, Info, Text, TreeAtom, TreeItem};

    fn kb() -> Keyboard {
        let mut k = Keyboard::new("und", 45, Info::default());
        k.markers = vec!["x".to_string(), "y".to_string()];
        k.sets = vec![
            vec![Text::from("a"), Text::from("ab"), Text::from("abc")],
            vec![Text::from("ab"), Text::from("a")],
        ];
        k.classes = vec![
            Class {
                ranges: vec![ClassRange { lo: 'a', hi: 'c' }],
                negated: false,
                markers: vec![1],
            },
            Class {
                ranges: vec![ClassRange { lo: 'a', hi: 'c' }],
                negated: true,
                markers: vec![],
            },
        ];
        k
    }

    fn ch(c: char) -> TreeItem {
        TreeItem::one(TreeAtom::Char(c))
    }

    fn q(atom: TreeAtom, min: u8, max: u8) -> TreeItem {
        TreeItem { atom, min, max }
    }

    fn cap(alts: Vec<Vec<TreeItem>>) -> TreeItem {
        TreeItem::one(TreeAtom::Capture(alts))
    }

    fn group(alts: Vec<Vec<TreeItem>>) -> TreeItem {
        TreeItem::one(TreeAtom::Group(alts))
    }

    fn text(s: &str) -> Vec<TextElem> {
        s.chars().map(TextElem::Char).collect()
    }

    fn run(
        k: &Keyboard,
        anchored: bool,
        alts: Vec<Vec<TreeItem>>,
        c: &[TextElem],
        at_start: bool,
    ) -> Option<Match> {
        let p = Pattern::from_tree(anchored, alts).unwrap();
        let info = RuleInfo::new(k, &p).unwrap();
        find(k, &p, &info, c, at_start)
    }

    // [spec:kbdgen:sem:ldml.engine.match+1/test]
    #[test]
    fn match_must_end_at_context_end() {
        let k = kb();
        assert!(
            run(
                &k,
                false,
                vec![vec![ch('k'), ch('e')]],
                &text("awake"),
                false
            )
            .is_some()
        );
        assert!(
            run(
                &k,
                false,
                vec![vec![ch('k'), ch('e')]],
                &text("keyboard"),
                false
            )
            .is_none()
        );
    }

    // [spec:kbdgen:sem:ldml.engine.match+1/test]
    #[test]
    fn smallest_start_wins_over_priority() {
        // (a|ab)?b? on "xab": no match can start at the `x`.
        let k = kb();
        let alts = vec![vec![
            q(
                TreeAtom::Capture(vec![vec![ch('a')], vec![ch('a'), ch('b')]]),
                0,
                1,
            ),
            q(TreeAtom::Char('b'), 0, 1),
        ]];
        let m = run(&k, false, alts, &text("xab"), false).unwrap();
        assert_eq!(m.start, 1);
        // Backtracking order: `a` first, then `b?` takes the `b`.
        assert_eq!(m.groups[1], Some((1, 2)));
    }

    // [spec:kbdgen:sem:ldml.engine.match+1/test]
    #[test]
    fn greedy_quantifier_backtracks_in_order() {
        // (a{0,3})(a{1,2}) on "aaaa": greedy first group takes 3, second 1.
        let k = kb();
        let alts = vec![vec![
            cap(vec![vec![q(TreeAtom::Char('a'), 0, 3)]]),
            cap(vec![vec![q(TreeAtom::Char('a'), 1, 2)]]),
        ]];
        let m = run(&k, false, alts, &text("aaaa"), false).unwrap();
        assert_eq!(m.start, 0);
        assert_eq!(m.groups[1], Some((0, 3)));
        assert_eq!(m.groups[2], Some((3, 4)));
        assert_eq!(m.groups[0], Some((0, 4)));
    }

    // [spec:kbdgen:sem:ldml.engine.match+1/test]
    #[test]
    fn alternatives_and_set_items_try_in_order() {
        let k = kb();
        // ($[0])(c?) on "abc": set item `a` fails ($ not reached),
        // `ab` then `c?` succeeds before item `abc` is tried.
        let alts = vec![vec![
            cap(vec![vec![TreeItem::one(TreeAtom::Set(0))]]),
            cap(vec![vec![q(TreeAtom::Char('c'), 0, 1)]]),
        ]];
        let m = run(&k, false, alts, &text("abc"), false).unwrap();
        assert_eq!(m.groups[1], Some((0, 2)));
        assert_eq!(m.groups[2], Some((2, 3)));
        // (ab|abc)c? likewise prefers the first alternative.
        let alts = vec![vec![
            cap(vec![
                vec![ch('a'), ch('b')],
                vec![ch('a'), ch('b'), ch('c')],
            ]),
            q(TreeAtom::Char('c'), 0, 1),
        ]];
        let m = run(&k, false, alts, &text("abc"), false).unwrap();
        assert_eq!(m.groups[1], Some((0, 2)));
    }

    // [spec:kbdgen:sem:ldml.engine.match+1/test]
    #[test]
    fn anchor_needs_start_of_text() {
        let k = kb();
        let alts = || vec![vec![ch('a')]];
        assert!(run(&k, true, alts(), &text("a"), true).is_some());
        assert!(run(&k, true, alts(), &text("a"), false).is_none());
        assert!(run(&k, true, alts(), &text("ba"), true).is_none());
    }

    // [spec:kbdgen:sem:ldml.engine.match+1/test]
    #[test]
    fn scalar_atoms_never_match_markers() {
        let k = kb();
        let c = [TextElem::Char('a'), TextElem::Marker(0)];
        for atom in [
            TreeAtom::Any,
            TreeAtom::Class(1),
            TreeAtom::Fixed(Fixed::NotSpace),
            TreeAtom::Char('a'),
        ] {
            assert!(
                run(
                    &k,
                    false,
                    vec![vec![TreeItem::one(atom.clone())]],
                    &c,
                    false
                )
                .is_none(),
                "{atom:?}"
            );
        }
        let marker = |atom| {
            run(
                &k,
                false,
                vec![vec![ch('a'), TreeItem::one(atom)]],
                &c,
                false,
            )
        };
        assert!(marker(TreeAtom::AnyMarker).is_some());
        assert!(marker(TreeAtom::Marker(0)).is_some());
        assert!(marker(TreeAtom::Marker(1)).is_none());
        // Class 0 lists marker 1 only; the negated class 1 matches no marker.
        assert!(marker(TreeAtom::Class(0)).is_none());
        let c1 = [TextElem::Char('z'), TextElem::Marker(1)];
        let one = |atom| run(&k, false, vec![vec![TreeItem::one(atom)]], &c1, false);
        assert!(one(TreeAtom::Class(0)).is_some());
        assert!(one(TreeAtom::Class(1)).is_none());
    }

    // [spec:kbdgen:sem:ldml.engine.match+1/test]
    #[test]
    fn negated_class_and_fixed_classes() {
        let k = kb();
        let one = |atom, s: &str| {
            run(&k, false, vec![vec![TreeItem::one(atom)]], &text(s), false).is_some()
        };
        assert!(one(TreeAtom::Class(1), "z"));
        assert!(!one(TreeAtom::Class(1), "b"));
        assert!(one(TreeAtom::Class(0), "b"));
        assert!(one(TreeAtom::Fixed(Fixed::Space), "\u{3000}"));
        assert!(one(TreeAtom::Fixed(Fixed::Space), " "));
        assert!(!one(TreeAtom::Fixed(Fixed::Space), "\u{200B}"));
        assert!(one(TreeAtom::Fixed(Fixed::Digit), "7"));
        assert!(!one(TreeAtom::Fixed(Fixed::Digit), "\u{663}"));
        assert!(one(TreeAtom::Fixed(Fixed::Word), "_"));
        assert!(!one(TreeAtom::Fixed(Fixed::Word), "\u{E9}"));
        assert!(one(TreeAtom::Fixed(Fixed::NotWord), "-"));
        assert!(one(TreeAtom::Any, "\n"));
        assert!(one(TreeAtom::Any, "\u{104B5}"));
    }

    // [spec:kbdgen:sem:ldml.engine.match+1/test]
    #[test]
    fn empty_iteration_ends_quantifier() {
        // (?:a?){2,3}b: the empty iteration check stops extra empty loops,
        // and the match still succeeds on "b".
        let k = kb();
        let alts = vec![vec![
            q(
                TreeAtom::Group(vec![vec![q(TreeAtom::Char('a'), 0, 1)]]),
                2,
                3,
            ),
            ch('b'),
        ]];
        assert_eq!(
            run(&k, false, alts.clone(), &text("b"), false)
                .unwrap()
                .start,
            0
        );
        assert_eq!(run(&k, false, alts, &text("aab"), false).unwrap().start, 0);
    }

    // [spec:kbdgen:sem:ldml.engine.match+1/test]
    #[test]
    fn quantified_capture_keeps_last_iteration() {
        // (?:(a)|b){2}: the second iteration takes `b` and resets group 1,
        // as ECMAScript does.
        let k = kb();
        let alts = vec![vec![q(
            TreeAtom::Group(vec![vec![cap(vec![vec![ch('a')]])], vec![ch('b')]]),
            2,
            2,
        )]];
        let m = run(&k, false, alts.clone(), &text("ab"), false).unwrap();
        assert_eq!(m.groups[1], None);
        let m = run(&k, false, alts, &text("ba"), false).unwrap();
        assert_eq!(m.groups[1], Some((1, 2)));
    }

    // [spec:kbdgen:req:ldml.engine.contract+1/test]
    #[test]
    fn deep_nesting_matches_without_recursion() {
        let k = kb();
        let mut alts = vec![vec![ch('a')]];
        for _ in 0..20_000 {
            alts = vec![vec![group(alts)]];
        }
        assert!(run(&k, false, alts, &text("ba"), false).is_some());
    }

    // [spec:kbdgen:req:ldml.engine.contract+1/test]
    #[test]
    fn nested_quantifiers_stay_polynomial() {
        // ((?:(?:a?){9}){7})b over 63 `a`s would take exponential time in
        // a naive backtracker; the tables bound it.
        let k = kb();
        let inner = q(
            TreeAtom::Group(vec![vec![q(TreeAtom::Char('a'), 0, 1)]]),
            0,
            9,
        );
        let alts = vec![vec![q(TreeAtom::Group(vec![vec![inner]]), 0, 7), ch('b')]];
        let mut c = text(&"a".repeat(62));
        c.push(TextElem::Char('c'));
        assert!(run(&k, false, alts, &c, false).is_none());
    }
}
