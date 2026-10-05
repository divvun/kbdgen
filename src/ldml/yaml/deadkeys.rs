//! What reachable dead keys add to a host document
//! (`ldml.yaml.dead-keys.*`): flush outputs, names, displays, and the
//! generated compose, fallback and backspace groups.

use kbd_ldml::syntax::{
    Atom, PatternSyntax, Quantified, ToItem, encode_pattern, encode_replacement,
};
use kbd_ldml::{Compose, ComposeValue, DeadKey};

use super::schema::{ComposeTo, DeadNode};

/// A generated rule: `from` and `to`, in LDML syntax.
pub type Rule = (String, Option<String>);

/// The dead-key part of a host document.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeadOut {
    /// Reachable nodes, depth first: (marker, display, standalone).
    pub markers: Vec<(String, String, String)>,
    pub names: Vec<(String, String)>,
    pub compose: Vec<Rule>,
    pub fallback: Vec<Rule>,
    pub backspace: Vec<Rule>,
}

fn pattern(atoms: Vec<Atom>) -> String {
    encode_pattern(&PatternSyntax {
        anchored: false,
        alternatives: vec![atoms.into_iter().map(Quantified::one).collect()],
    })
}

fn text_to(s: &str) -> Vec<ToItem> {
    s.chars().map(ToItem::Char).collect()
}

fn walk<'a>(node: &'a DeadNode, out: &mut Vec<&'a DeadNode>) {
    out.push(node);
    for entry in &node.compose {
        if let ComposeTo::Node(child) = &entry.value {
            walk(child, out);
        }
    }
}

// [spec:kbdgen:sem:ldml.yaml.dead-keys.compose]
fn compose_rules(node: &DeadNode, tops: &[&DeadNode], out: &mut Vec<Rule>) {
    let m = Atom::Marker(node.marker.clone());
    for entry in &node.compose {
        let to = match &entry.value {
            ComposeTo::Output(s) => text_to(s),
            ComposeTo::Node(child) => vec![ToItem::Marker(child.marker.clone())],
        };
        let to = encode_replacement(&to);
        let mut atoms = vec![m.clone()];
        atoms.extend(entry.input.chars().map(Atom::Char));
        out.push((pattern(atoms), Some(to.clone())));
        if let Some(e) = tops.iter().find(|t| t.identity == entry.input) {
            out.push((
                pattern(vec![m.clone(), Atom::Marker(e.marker.clone())]),
                Some(to),
            ));
        }
    }
    out.push((
        format!("{}\\u{{20}}", pattern(vec![m])),
        Some(encode_replacement(&text_to(&node.standalone))),
    ));
    for entry in &node.compose {
        if let ComposeTo::Node(child) = &entry.value {
            compose_rules(child, tops, out);
        }
    }
}

// [spec:kbdgen:sem:ldml.yaml.dead-keys.fallback]
/// The fallback rules of one marker with standalone `s`: a following
/// reachable top-level dead key stays pending, and any other following
/// text of up to `longest` scalar values comes after `s`.
fn fallback_rules(marker: &str, s: &str, tops: &[&DeadNode], longest: usize, out: &mut Vec<Rule>) {
    let m = Atom::Marker(marker.to_string());
    for e in tops {
        let mut to = text_to(s);
        to.push(ToItem::Marker(e.marker.clone()));
        out.push((
            pattern(vec![m.clone(), Atom::Marker(e.marker.clone())]),
            Some(encode_replacement(&to)),
        ));
    }
    let mut capture = vec![Quantified {
        atom: Atom::Any,
        min: 1,
        max: 9,
    }];
    let extra = longest.saturating_sub(9).div_ceil(9);
    capture.extend((0..extra).map(|_| Quantified {
        atom: Atom::Any,
        min: 0,
        max: 9,
    }));
    let from = encode_pattern(&PatternSyntax {
        anchored: false,
        alternatives: vec![vec![
            Quantified::one(m),
            Quantified::one(Atom::Capture(capture)),
        ]],
    });
    let mut to = text_to(s);
    to.push(ToItem::Group(1));
    out.push((from, Some(encode_replacement(&to))));
}

// [spec:kbdgen:sem:ldml.yaml.dead-keys.keys]
// [spec:kbdgen:sem:ldml.yaml.dead-keys.backspace]
// [spec:kbdgen:sem:ldml.scope.macos-rules]
/// The dead-key part of a document whose `\d{}` tokens reached the
/// top-level dead keys `reached`. Only those, with their nested nodes, get
/// markers, flush outputs and rules. `longest` is the longest key output
/// of the document in scalar values, which the fallback rule must cover.
pub fn lower(dead_keys: &[DeadNode], reached: &[String], longest: usize) -> DeadOut {
    let tops: Vec<&DeadNode> = dead_keys
        .iter()
        .filter(|d| reached.contains(&d.identity))
        .collect();
    let mut out = DeadOut::default();
    if tops.is_empty() {
        return out;
    }
    let mut nodes = Vec::new();
    for top in &tops {
        walk(top, &mut nodes);
        if let Some(name) = &top.name {
            out.names.push((top.marker.clone(), name.clone()));
        }
        compose_rules(top, &tops, &mut out.compose);
    }
    for node in &nodes {
        out.markers.push((
            node.marker.clone(),
            node.display.clone(),
            node.standalone.clone(),
        ));
        fallback_rules(
            &node.marker,
            &node.standalone,
            &tops,
            longest,
            &mut out.fallback,
        );
    }
    out.backspace.push((pattern(vec![Atom::AnyMarker]), None));
    out
}

fn compose_metadata(node: &DeadNode) -> Vec<Compose> {
    node.compose
        .iter()
        .map(|entry| Compose {
            input: entry.input.clone(),
            value: match &entry.value {
                ComposeTo::Output(s) => ComposeValue::Output(s.clone()),
                ComposeTo::Node(child) => ComposeValue::Node {
                    marker: child.marker.clone(),
                    standalone: child.standalone.clone(),
                    compose: compose_metadata(child),
                },
            },
        })
        .collect()
}

/// The whole `deadKeys` table as authoring metadata (`kbdgen:deadKey`),
/// which import re-derives the generated groups from.
pub fn metadata(dead_keys: &[DeadNode]) -> Vec<DeadKey> {
    dead_keys
        .iter()
        .map(|d| DeadKey {
            identity: d.identity.clone(),
            marker: d.marker.clone(),
            display: d.display.clone(),
            standalone: d.standalone.clone(),
            name: d.name.clone(),
            compose: compose_metadata(d),
        })
        .collect()
}
