//! Transform groups: patterns, replacements and reorder rules.

use alloc::vec;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use crate::text::{MarkerIndex, Text};
use crate::validate::Invariant;

/// An index into `Keyboard::sets`.
pub type SetIndex = u16;
/// An index into `Keyboard::classes`.
pub type ClassIndex = u16;
/// An index into `Pattern::nodes`.
pub type NodeIndex = u16;

pub const MAX_CAPTURES: u8 = 9;
/// The largest quantifier bound.
pub const MAX_REPEAT: u8 = 9;

/// An inclusive range of scalar values, `lo ≤ hi`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ClassRange {
    pub lo: char,
    pub hi: char,
}

impl ClassRange {
    pub const fn single(c: char) -> Self {
        ClassRange { lo: c, hi: c }
    }

    pub fn contains(self, c: char) -> bool {
        self.lo <= c && c <= self.hi
    }
}

/// A character class of a pattern (`$[uset]` or `[…]`).
///
/// Markers match only through the explicit `markers` members, and a
/// negated class never matches a marker (`ldml.xml.from`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Class {
    pub ranges: Vec<ClassRange>,
    pub negated: bool,
    pub markers: Vec<MarkerIndex>,
}

/// An entry of `Keyboard::sets`: texts tried as alternatives in order.
pub type Set = Vec<Text>;

/// A fixed character class, with LDML's definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Fixed {
    /// `\s`
    Space,
    /// `\S`
    NotSpace,
    /// `\d`
    Digit,
    /// `\D`
    NotDigit,
    /// `\w`
    Word,
    /// `\W`
    NotWord,
    /// `\t`
    Tab,
    /// `\r`
    CarriageReturn,
    /// `\n`
    LineFeed,
    /// `\f`
    FormFeed,
    /// `\v`
    VerticalTab,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Atom {
    Char(char),
    /// `.`
    Any,
    Class(ClassIndex),
    Fixed(Fixed),
    Marker(MarkerIndex),
    /// `\m{.}`
    AnyMarker,
    Set(SetIndex),
    /// A non-capturing group: the alternation `Pattern::nodes[n]`.
    Group(NodeIndex),
    /// Capture `number` (1–9) of the alternation `Pattern::nodes[node]`.
    Capture {
        number: u8,
        node: NodeIndex,
    },
}

/// An atom with its quantifier `{min,max}`, where `0 ≤ min ≤ max ≤ 9` and
/// `max ≥ 1`. An unquantified atom has `{1,1}` and `?` is `{0,1}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Item {
    pub atom: Atom,
    pub min: u8,
    pub max: u8,
}

impl Item {
    /// The atom, unquantified.
    pub const fn one(atom: Atom) -> Self {
        Item {
            atom,
            min: 1,
            max: 1,
        }
    }
}

pub type Sequence = Vec<Item>;

/// Sequences tried left to right.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Alternation {
    /// Non-empty.
    pub alternatives: Vec<Sequence>,
}

// [spec:kbdgen:def:ldml.model.pattern+1]
/// A pattern: an optional start anchor plus an alternation of sequences.
///
/// Groups are stored flat, so no encoded type is recursive and decoding
/// never recurses: `nodes[0]` is the top-level alternation, and each
/// `Group` or `Capture` atom names a later node. Nodes are numbered in the
/// order their groups open, left to right, so every node but the first is
/// named by exactly one atom and a pattern has exactly one encoding.
/// [`Pattern::from_tree`] produces that numbering.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Pattern {
    /// `^`
    pub anchored: bool,
    pub nodes: Vec<Alternation>,
}

/// A pattern atom in tree form, for building a [`Pattern`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreeAtom {
    Char(char),
    Any,
    Class(ClassIndex),
    Fixed(Fixed),
    Marker(MarkerIndex),
    AnyMarker,
    Set(SetIndex),
    Group(Vec<Vec<TreeItem>>),
    /// Numbered from 1 in the order captures open.
    Capture(Vec<Vec<TreeItem>>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeItem {
    pub atom: TreeAtom,
    pub min: u8,
    pub max: u8,
}

impl TreeItem {
    /// The atom, unquantified.
    pub const fn one(atom: TreeAtom) -> Self {
        TreeItem {
            atom,
            min: 1,
            max: 1,
        }
    }
}

/// What [`Pattern::analyze`] finds in a well-formed pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatternInfo {
    /// `captures[n - 1]` is the node of capture `n`.
    pub captures: Vec<NodeIndex>,
    /// The fewest elements a match spans; none when nothing can match.
    pub min_len: Option<usize>,
    /// The most elements a match spans.
    pub max_len: usize,
}

/// A pattern defect found by [`Pattern::analyze`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PatternError {
    pub invariant: Invariant,
    /// The node holding the defect.
    pub node: usize,
}

/// One alternation being flattened by [`Pattern::from_tree`].
struct Frame {
    node: usize,
    alternatives: vec::IntoIter<Vec<TreeItem>>,
    sequence: Option<vec::IntoIter<TreeItem>>,
    done: Vec<Sequence>,
    current: Sequence,
}

impl Frame {
    fn new(node: usize, tree: Vec<Vec<TreeItem>>) -> Self {
        Frame {
            node,
            alternatives: tree.into_iter(),
            sequence: None,
            done: Vec::new(),
            current: Vec::new(),
        }
    }

    /// The next item in left-to-right order, closing finished sequences.
    fn next_item(&mut self) -> Option<TreeItem> {
        loop {
            if let Some(sequence) = &mut self.sequence {
                if let Some(item) = sequence.next() {
                    return Some(item);
                }
                self.done.push(core::mem::take(&mut self.current));
                self.sequence = None;
            }
            self.sequence = Some(self.alternatives.next()?.into_iter());
        }
    }
}

impl Pattern {
    /// Flattens a tree into node order, numbering captures as they open.
    /// Works without recursion, so any nesting depth is safe. Fails with
    /// `Invariant::PatternTree` past 65536 nodes and with
    /// `Invariant::Captures` past 255 captures; [`Pattern::analyze`]
    /// checks every other limit.
    pub fn from_tree(anchored: bool, top: Vec<Vec<TreeItem>>) -> Result<Pattern, Invariant> {
        let mut nodes = vec![Alternation::default()];
        let mut captures: u8 = 0;
        // On a limit the walk still runs to the end, so the remaining tree
        // is taken apart here, iteratively, rather than by a recursive drop.
        let mut failure: Option<Invariant> = None;
        let mut stack = vec![Frame::new(0, top)];
        while let Some(frame) = stack.last_mut() {
            let Some(item) = frame.next_item() else {
                if let Some(done) = stack.pop()
                    && let Some(node) = nodes.get_mut(done.node)
                {
                    node.alternatives = done.done;
                }
                continue;
            };
            let next = NodeIndex::try_from(nodes.len()).unwrap_or_else(|_| {
                failure.get_or_insert(Invariant::PatternTree);
                0
            });
            let (atom, child) = match item.atom {
                TreeAtom::Char(c) => (Atom::Char(c), None),
                TreeAtom::Any => (Atom::Any, None),
                TreeAtom::Class(i) => (Atom::Class(i), None),
                TreeAtom::Fixed(f) => (Atom::Fixed(f), None),
                TreeAtom::Marker(m) => (Atom::Marker(m), None),
                TreeAtom::AnyMarker => (Atom::AnyMarker, None),
                TreeAtom::Set(s) => (Atom::Set(s), None),
                TreeAtom::Group(sub) => (Atom::Group(next), Some(sub)),
                TreeAtom::Capture(sub) => {
                    captures = captures.checked_add(1).unwrap_or_else(|| {
                        failure.get_or_insert(Invariant::Captures);
                        u8::MAX
                    });
                    let number = captures;
                    (Atom::Capture { number, node: next }, Some(sub))
                }
            };
            frame.current.push(Item {
                atom,
                min: item.min,
                max: item.max,
            });
            if let Some(sub) = child {
                let index = nodes.len();
                if failure.is_none() {
                    nodes.push(Alternation::default());
                }
                stack.push(Frame::new(index, sub));
            }
        }
        if let Some(invariant) = failure {
            return Err(invariant);
        }
        Ok(Pattern { anchored, nodes })
    }

    /// Checks that the pattern is well formed and measures it.
    ///
    /// The checks: nodes are numbered in opening order and each has an
    /// alternative; quantifiers are in bounds; captures are numbered 1, 2,
    /// … in opening order, at most nine, none inside another; and set,
    /// class and marker indices are in range of `sets`, `classes` and a
    /// marker table of `markers` entries. The walk is iterative, and the
    /// lengths are computed bottom-up over the node order, so a pattern of
    /// any depth is checked without recursion.
    pub fn analyze(
        &self,
        sets: &[Set],
        classes: usize,
        markers: usize,
    ) -> Result<PatternInfo, PatternError> {
        let err = |invariant, node| PatternError { invariant, node };
        if self.nodes.is_empty() {
            return Err(err(Invariant::PatternTree, 0));
        }
        let mut captures: Vec<NodeIndex> = Vec::new();
        let mut next_node: usize = 1;
        // (node, alternative, item, inside a capture)
        let mut stack: Vec<(usize, usize, usize, bool)> = vec![(0, 0, 0, false)];
        while let Some(frame) = stack.last_mut() {
            let (node, alt, at, in_capture) = *frame;
            let Some(alternation) = self.nodes.get(node) else {
                return Err(err(Invariant::PatternTree, node));
            };
            if alternation.alternatives.is_empty() {
                return Err(err(Invariant::PatternTree, node));
            }
            let Some(sequence) = alternation.alternatives.get(alt) else {
                stack.pop();
                continue;
            };
            let Some(item) = sequence.get(at) else {
                *frame = (node, alt.saturating_add(1), 0, in_capture);
                continue;
            };
            *frame = (node, alt, at.saturating_add(1), in_capture);
            if item.max == 0 || item.min > item.max || item.max > MAX_REPEAT {
                return Err(err(Invariant::Quantifier, node));
            }
            match item.atom {
                Atom::Char(_) | Atom::Any | Atom::Fixed(_) | Atom::AnyMarker => {}
                Atom::Class(i) => {
                    if usize::from(i) >= classes {
                        return Err(err(Invariant::IndexRange, node));
                    }
                }
                Atom::Marker(m) => {
                    if usize::from(m) >= markers {
                        return Err(err(Invariant::IndexRange, node));
                    }
                }
                Atom::Set(s) => {
                    if usize::from(s) >= sets.len() {
                        return Err(err(Invariant::IndexRange, node));
                    }
                }
                Atom::Group(child) => {
                    if usize::from(child) != next_node {
                        return Err(err(Invariant::PatternTree, node));
                    }
                    next_node = next_node.saturating_add(1);
                    stack.push((usize::from(child), 0, 0, in_capture));
                }
                Atom::Capture {
                    number,
                    node: child,
                } => {
                    if usize::from(child) != next_node {
                        return Err(err(Invariant::PatternTree, node));
                    }
                    if in_capture {
                        return Err(err(Invariant::NestedCapture, node));
                    }
                    captures.push(child);
                    if captures.len() > usize::from(MAX_CAPTURES) {
                        return Err(err(Invariant::Captures, node));
                    }
                    if usize::from(number) != captures.len() {
                        return Err(err(Invariant::CaptureNumbering, node));
                    }
                    next_node = next_node.saturating_add(1);
                    stack.push((usize::from(child), 0, 0, true));
                }
            }
        }
        if next_node != self.nodes.len() {
            return Err(err(Invariant::PatternTree, next_node));
        }

        // Children follow their parents, so a reverse walk sees every
        // child's lengths before its parent needs them.
        let mut lens: Vec<(Option<usize>, usize)> = vec![(None, 0); self.nodes.len()];
        for (index, alternation) in self.nodes.iter().enumerate().rev() {
            let mut node_min: Option<usize> = None;
            let mut node_max: usize = 0;
            for sequence in &alternation.alternatives {
                let mut seq_min: Option<usize> = Some(0);
                let mut seq_max: usize = 0;
                for item in sequence {
                    let (atom_min, atom_max) = match item.atom {
                        Atom::Char(_)
                        | Atom::Any
                        | Atom::Class(_)
                        | Atom::Fixed(_)
                        | Atom::Marker(_)
                        | Atom::AnyMarker => (Some(1), 1),
                        Atom::Set(s) => {
                            let items = sets.get(usize::from(s)).map_or(&[][..], |v| v);
                            let min = items.iter().map(Text::len).min();
                            let max = items.iter().map(Text::len).max().unwrap_or(0);
                            (min, max)
                        }
                        Atom::Group(child) | Atom::Capture { node: child, .. } => {
                            lens.get(usize::from(child)).copied().unwrap_or((None, 0))
                        }
                    };
                    let item_min = if item.min == 0 {
                        Some(0)
                    } else {
                        atom_min.map(|m| m.saturating_mul(usize::from(item.min)))
                    };
                    let item_max = if atom_min.is_none() {
                        0
                    } else {
                        atom_max.saturating_mul(usize::from(item.max))
                    };
                    seq_min = match (seq_min, item_min) {
                        (Some(a), Some(b)) => Some(a.saturating_add(b)),
                        _ => None,
                    };
                    seq_max = seq_max.saturating_add(item_max);
                }
                if let Some(m) = seq_min {
                    node_min = Some(node_min.map_or(m, |n| n.min(m)));
                    node_max = node_max.max(seq_max);
                }
            }
            if let Some(slot) = lens.get_mut(index) {
                *slot = (node_min, node_max);
            }
        }
        let (min_len, max_len) = lens.first().copied().unwrap_or((None, 0));
        Ok(PatternInfo {
            captures,
            min_len,
            max_len,
        })
    }
}

/// One element of a replacement.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ReplacementItem {
    Text(Text),
    /// The elements captured by group `n`; 0 is the whole match.
    Group(u8),
    /// The item of set `to` at the index of the item of set `from` that
    /// capture `group` matched.
    MapSet {
        group: u8,
        from: SetIndex,
        to: SetIndex,
    },
}

/// A replacement; an absent `to` is the empty replacement.
pub type Replacement = Vec<ReplacementItem>;

/// A transform rule.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Rule {
    pub from: Pattern,
    pub to: Replacement,
}

/// A reorder class: a scalar value or a set of scalar ranges.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ReorderClass {
    Char(char),
    Ranges(Vec<ClassRange>),
}

impl ReorderClass {
    pub fn contains(&self, c: char) -> bool {
        match self {
            ReorderClass::Char(x) => *x == c,
            ReorderClass::Ranges(ranges) => ranges.iter().any(|r| r.contains(c)),
        }
    }
}

/// A reorder rule after LDML's split and merge. `order`, `tertiary`,
/// `tertiary_base` and `pre_base` are padded to the length of `from`,
/// which is non-empty.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ReorderRule {
    pub before: Vec<ReorderClass>,
    pub from: Vec<ReorderClass>,
    pub order: Vec<i8>,
    pub tertiary: Vec<i8>,
    pub tertiary_base: Vec<bool>,
    pub pre_base: Vec<bool>,
}

impl ReorderRule {
    /// The match priority key; a group's rules are sorted by it,
    /// descending: longest `from`, then longest `before`.
    pub fn priority(&self) -> (usize, usize) {
        (self.from.len(), self.before.len())
    }
}

// [spec:kbdgen:def:ldml.model.transforms+1]
/// A transform group of `simple` or `backspace`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TransformGroup {
    /// A non-empty ordered list of rules.
    Rules(Vec<Rule>),
    /// Reorder rules sorted into match priority ([`ReorderRule::priority`]).
    Reorder(Vec<ReorderRule>),
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    fn ch(c: char) -> TreeItem {
        TreeItem::one(TreeAtom::Char(c))
    }

    fn analyze(p: &Pattern) -> Result<PatternInfo, PatternError> {
        let sets = vec![vec![Text::from("ab"), Text::from("c")], vec![]];
        p.analyze(&sets, 1, 1)
    }

    // [spec:kbdgen:def:ldml.model.pattern+1/test]
    #[test]
    fn tree_flattens_in_opening_order() {
        // a((b)|(c(d)))
        let inner = TreeItem::one(TreeAtom::Group(vec![vec![ch('d')]]));
        let tree = vec![vec![
            ch('a'),
            TreeItem::one(TreeAtom::Group(vec![
                vec![TreeItem::one(TreeAtom::Capture(vec![vec![ch('b')]]))],
                vec![TreeItem::one(TreeAtom::Capture(vec![vec![ch('c'), inner]]))],
            ])),
        ]];
        let p = Pattern::from_tree(true, tree).unwrap();
        assert!(p.anchored);
        assert_eq!(p.nodes.len(), 5);
        assert_eq!(p.nodes[0].alternatives[0][1].atom, Atom::Group(1));
        assert_eq!(
            p.nodes[1].alternatives[0][0].atom,
            Atom::Capture { number: 1, node: 2 }
        );
        assert_eq!(
            p.nodes[1].alternatives[1][0].atom,
            Atom::Capture { number: 2, node: 3 }
        );
        assert_eq!(p.nodes[3].alternatives[0][1].atom, Atom::Group(4));
        let info = analyze(&p).unwrap();
        assert_eq!(info.captures, [2, 3]);
        assert_eq!(info.min_len, Some(2));
        assert_eq!(info.max_len, 3);
    }

    // [spec:kbdgen:def:ldml.model.pattern+1/test]
    #[test]
    fn lengths_follow_quantifiers_and_sets() {
        // \m{.}$[0]{0,9}x?
        let tree = vec![vec![
            TreeItem::one(TreeAtom::AnyMarker),
            TreeItem {
                atom: TreeAtom::Set(0),
                min: 0,
                max: 9,
            },
            TreeItem {
                atom: TreeAtom::Char('x'),
                min: 0,
                max: 1,
            },
        ]];
        let info = analyze(&Pattern::from_tree(false, tree).unwrap()).unwrap();
        assert_eq!(info.min_len, Some(1));
        assert_eq!(info.max_len, 1 + 18 + 1);

        let never = vec![vec![TreeItem::one(TreeAtom::Set(1))]];
        let info = analyze(&Pattern::from_tree(false, never).unwrap()).unwrap();
        assert_eq!(info.min_len, None);
    }

    // [spec:kbdgen:req:ldml.model.invariants+1/test]
    #[test]
    fn rejects_nested_and_excess_captures() {
        let nested = vec![vec![TreeItem::one(TreeAtom::Capture(vec![vec![
            TreeItem::one(TreeAtom::Group(vec![vec![TreeItem::one(
                TreeAtom::Capture(vec![vec![ch('a')]]),
            )]])),
        ]]))]];
        let p = Pattern::from_tree(false, nested).unwrap();
        assert_eq!(analyze(&p).unwrap_err().invariant, Invariant::NestedCapture);

        let ten = vec![
            (0..10)
                .map(|_| TreeItem::one(TreeAtom::Capture(vec![vec![ch('a')]])))
                .collect(),
        ];
        let p = Pattern::from_tree(false, ten).unwrap();
        assert_eq!(analyze(&p).unwrap_err().invariant, Invariant::Captures);
    }

    // [spec:kbdgen:req:ldml.model.invariants+1/test]
    #[test]
    fn rejects_malformed_trees_and_quantifiers() {
        let mut p = Pattern::from_tree(false, vec![vec![ch('a')]]).unwrap();
        p.nodes[0].alternatives[0][0].max = 10;
        assert_eq!(analyze(&p).unwrap_err().invariant, Invariant::Quantifier);
        p.nodes[0].alternatives[0][0] = Item {
            atom: Atom::Char('a'),
            min: 0,
            max: 0,
        };
        assert_eq!(analyze(&p).unwrap_err().invariant, Invariant::Quantifier);

        // A group naming itself would loop; numbering rejects it.
        p.nodes[0].alternatives[0][0] = Item::one(Atom::Group(0));
        assert_eq!(analyze(&p).unwrap_err().invariant, Invariant::PatternTree);

        // An unreferenced trailing node.
        let mut q = Pattern::from_tree(false, vec![vec![ch('a')]]).unwrap();
        q.nodes.push(Alternation {
            alternatives: vec![vec![]],
        });
        assert_eq!(analyze(&q).unwrap_err().invariant, Invariant::PatternTree);

        let empty = Pattern {
            anchored: false,
            nodes: vec![],
        };
        assert_eq!(
            analyze(&empty).unwrap_err().invariant,
            Invariant::PatternTree
        );

        let mut r = Pattern::from_tree(false, vec![vec![ch('a')]]).unwrap();
        r.nodes[0].alternatives[0][0] = Item::one(Atom::Set(7));
        assert_eq!(analyze(&r).unwrap_err().invariant, Invariant::IndexRange);
    }

    #[test]
    fn deep_nesting_needs_no_recursion() {
        let mut tree = vec![vec![ch('a')]];
        for _ in 0..20_000 {
            tree = vec![vec![TreeItem::one(TreeAtom::Group(tree))]];
        }
        let p = Pattern::from_tree(false, tree).unwrap();
        let info = analyze(&p).unwrap();
        assert_eq!(info.min_len, Some(1));
        assert_eq!(info.max_len, 1);
    }
}
