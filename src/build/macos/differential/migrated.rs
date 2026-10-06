//! A v3 layout's `.keylayout` against that of its migration
//! (`ldml.macos.test.differential`). Key maps are matched by their modifier
//! terms and dead-key states by the keys that enter them; action ids are
//! not compared. Each difference is explained or reported.

use std::collections::BTreeMap;

use super::{fixture_bundle, migrated_yaml, parse, subject};
use crate::build::macos::input::{Binding, KeyMap, KeylayoutInput, NONE_STATE};
use crate::build::macos::{v3, writer};

/// A documented reason for the migrated `.keylayout` to differ.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Explanation {
    /// The model lists native-only layers after the others.
    NativeLast,
    /// macOS `\u{0}`, which typed U+0000, is now no key (M13).
    NulKey,
    /// The space bar follows `macOS.space` and composes with dead keys,
    /// where v3 always wrote a plain U+0020.
    Space,
    /// Dead keys compose on every layer, where v3 composed only on layers
    /// with a `deadKeys` list.
    UnlistedLayer,
    /// A dead key keeps its composition with a pending dead key, which v3
    /// replaced by its `next` action.
    DeadAfterDead,
    /// A transform root no macOS key makes dead has no state (M03).
    UnusedState,
}

/// Every explanation the rule lists.
// [spec:kbdgen:req:ldml.macos.test.differential]
pub const EXPLANATIONS: [Explanation; 6] = [
    Explanation::NativeLast,
    Explanation::NulKey,
    Explanation::Space,
    Explanation::UnlistedLayer,
    Explanation::DeadAfterDead,
    Explanation::UnusedState,
];

/// The differences between the two documents.
#[derive(Debug, Default)]
pub struct Comparison {
    pub identical: bool,
    pub explained: BTreeMap<Explanation, Vec<String>>,
    pub unexplained: Vec<String>,
}

impl Comparison {
    fn explain(&mut self, why: Option<Explanation>, line: String) {
        match why {
            Some(why) => self.explained.entry(why).or_default().push(line),
            None => self.unexplained.push(line),
        }
    }
}

/// A key's behaviour per state: (output, next), states renamed by `rename`.
type Behaviour = BTreeMap<String, (Option<String>, Option<String>)>;

fn behaviour(binding: &Binding, rename: &dyn Fn(&str) -> String) -> Behaviour {
    match binding {
        Binding::Output(output) => {
            BTreeMap::from([(NONE_STATE.to_owned(), (Some(output.clone()), None))])
        }
        Binding::Action { whens, .. } => whens
            .iter()
            .map(|w| {
                (
                    rename(&w.state),
                    (w.output.clone(), w.next.as_deref().map(rename)),
                )
            })
            .collect(),
    }
}

fn dead_next(binding: &Binding) -> Option<&str> {
    let Binding::Action { whens, .. } = binding else {
        return None;
    };
    whens
        .iter()
        .find(|w| w.state == NONE_STATE)
        .and_then(|w| w.next.as_deref())
}

fn is_native(map: &KeyMap) -> bool {
    map.modifiers.iter().all(|t| {
        t.split_whitespace()
            .any(|k| k == "command" || k == "anyControl")
    })
}

/// The v4 map with each v3 map's terms, in v3 order.
fn match_maps(
    old: &KeylayoutInput,
    new: &KeylayoutInput,
    out: &mut Comparison,
) -> Vec<(usize, usize)> {
    let mut pairs = Vec::new();
    for (i, map) in old.maps.iter().enumerate() {
        match new.maps.iter().position(|m| m.modifiers == map.modifiers) {
            Some(j) => pairs.push((i, j)),
            None => out.unexplained.push(format!(
                "key map {:?}: missing after migration",
                map.modifiers
            )),
        }
    }
    for map in new
        .maps
        .iter()
        .filter(|m| !old.maps.iter().any(|o| o.modifiers == m.modifiers))
    {
        out.unexplained
            .push(format!("key map {:?}: new after migration", map.modifiers));
    }
    let order: Vec<usize> = pairs.iter().map(|(_, j)| *j).collect();
    let mut native_last = order.clone();
    native_last.sort_by_key(|j| is_native(&new.maps[*j]));
    if !order.is_sorted() {
        let why = native_last.is_sorted().then_some(Explanation::NativeLast);
        out.explain(why, format!("key map order: v3 {order:?}"));
    }
    pairs
}

/// v3 state → migrated state, by the dead keys at the same code.
fn match_states(
    old: &KeylayoutInput,
    new: &KeylayoutInput,
    pairs: &[(usize, usize)],
    out: &mut Comparison,
) -> BTreeMap<String, String> {
    let mut states = BTreeMap::new();
    for (i, j) in pairs {
        for key in &old.maps[*i].keys {
            let Some(x) = dead_next(&key.binding) else {
                continue;
            };
            let other = new.maps[*j].keys.iter().find(|k| k.code == key.code);
            let Some(y) = other.and_then(|k| dead_next(&k.binding)) else {
                continue;
            };
            if let Some(previous) = states.insert(x.to_owned(), y.to_owned())
                && previous != y
            {
                out.unexplained.push(format!(
                    "state {x} is both {previous} and {y} after migration"
                ));
            }
        }
    }
    for terminator in &old.terminators {
        let Some(mapped) = states.get(&terminator.state) else {
            out.explain(
                Some(Explanation::UnusedState),
                format!("terminator of {}", terminator.state),
            );
            continue;
        };
        let new_output = new
            .terminators
            .iter()
            .find(|t| t.state == *mapped)
            .and_then(|t| t.output.clone());
        if new_output != terminator.output {
            out.unexplained.push(format!(
                "terminator of {}: v3 {:?}, migrated {new_output:?}",
                terminator.state, terminator.output
            ));
        }
    }
    states
}

/// Why one key differs, if a listed reason explains it.
fn key_reason(
    code: u16,
    old: &Behaviour,
    new: &Behaviour,
    old_map: &KeyMap,
) -> Option<Explanation> {
    if code == 49 {
        return Some(Explanation::Space);
    }
    let none_equal = old.get(NONE_STATE) == new.get(NONE_STATE);
    let only_added = old.iter().all(|(state, b)| new.get(state) == Some(b));
    let v3_actions = old_map
        .keys
        .iter()
        .any(|k| matches!(k.binding, Binding::Action { .. }));
    let dead = new.get(NONE_STATE).is_some_and(|(_, next)| next.is_some());
    match (none_equal && only_added, dead, v3_actions) {
        (true, true, _) => Some(Explanation::DeadAfterDead),
        (true, false, false) => Some(Explanation::UnlistedLayer),
        _ => None,
    }
}

fn compare_keys(
    old: &KeyMap,
    new: &KeyMap,
    states: &BTreeMap<String, String>,
    out: &mut Comparison,
) {
    let rename = |s: &str| states.get(s).cloned().unwrap_or_else(|| s.to_owned());
    let keep = |s: &str| s.to_owned();
    let label = &old.modifiers;
    for key in &old.keys {
        let mut old_b = behaviour(&key.binding, &rename);
        let unused = old_b.len();
        old_b.retain(|state, _| state == NONE_STATE || states.values().any(|s| s == state));
        if old_b.len() < unused {
            out.explain(
                Some(Explanation::UnusedState),
                format!(
                    "{label:?} code {}: whens for states no key enters",
                    key.code
                ),
            );
        }
        let Some(other) = new.keys.iter().find(|k| k.code == key.code) else {
            let nul = matches!(&key.binding, Binding::Output(o) if o == "\0");
            let why = nul.then_some(Explanation::NulKey);
            out.explain(
                why,
                format!("{label:?} code {}: no key after migration", key.code),
            );
            continue;
        };
        let new_b = behaviour(&other.binding, &keep);
        if old_b != new_b {
            let why = key_reason(key.code, &old_b, &new_b, old);
            out.explain(
                why,
                format!(
                    "{label:?} code {}: v3 {old_b:?}, migrated {new_b:?}",
                    key.code
                ),
            );
        }
    }
    for key in new
        .keys
        .iter()
        .filter(|k| !old.keys.iter().any(|o| o.code == k.code))
    {
        out.unexplained
            .push(format!("{label:?} code {}: new after migration", key.code));
    }
    let codes = |map: &KeyMap, other: &KeyMap| -> Vec<u16> {
        map.keys
            .iter()
            .map(|k| k.code)
            .filter(|c| other.keys.iter().any(|k| k.code == *c))
            .collect()
    };
    if codes(old, new) != codes(new, old) {
        out.unexplained
            .push(format!("{label:?}: keys in another order"));
    }
}

/// Builds the v3 layout `yaml` both ways and compares the documents.
// [spec:kbdgen:req:ldml.macos.test.differential]
pub fn compare(tag: &str, yaml: &str) -> Comparison {
    let fixture = fixture_bundle(tag, yaml);
    let (tag_v3, layout) = fixture.bundle.layouts.iter().next().unwrap();
    let old_xml = writer::write(&v3::layout_input(tag_v3, layout).unwrap());
    let migrated = subject(&format!("{tag} (migrated)"), tag, &migrated_yaml(tag, yaml));
    let mut out = Comparison {
        identical: old_xml == migrated.xml,
        ..Comparison::default()
    };
    let old = parse(&old_xml, tag).unwrap();
    let new = &migrated.parsed;
    if old.name != new.name {
        out.unexplained
            .push(format!("name: v3 {}, migrated {}", old.name, new.name));
    }
    let pairs = match_maps(&old, new, &mut out);
    if pairs.iter().all(|(i, _)| *i != old.default_index)
        || !pairs.contains(&(old.default_index, new.default_index))
    {
        out.unexplained.push("default index".to_owned());
    }
    let states = match_states(&old, new, &pairs, &mut out);
    for (i, j) in pairs {
        compare_keys(&old.maps[i], &new.maps[j], &states, &mut out);
    }
    out
}
