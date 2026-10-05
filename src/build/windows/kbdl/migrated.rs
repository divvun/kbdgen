//! A v3 layout's Windows layout DLL (`kbdl.input.bundle`) against the DLL
//! of its migration (`ldml.kbdl.adapter`): what each modifier state types
//! at each position, the dead-key trees and the metadata. Differences are
//! split into those the migration's defects explain and the rest.

use anyhow::{Context, Result, anyhow, bail};
use indexmap::IndexMap;
use language_tags::LanguageTag;

use super::adapter;
use super::bundle::layout_input;
use super::input::{DeadKeyNode, Layer, LayoutInput, POSITION_NAMES, STANDALONE};
use crate::bundle::KbdgenBundle;
use crate::ldml::migrate::defect::Code;
use crate::ldml::migrate::oracle::{CapsState, windows_caps};
use crate::ldml::migrate::{Migration, Out, migrate_bundle_layout};
use crate::util::decode_unicode_escapes;

/// The differences between the two DLLs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Comparison {
    pub explained: Vec<String>,
    pub unexplained: Vec<String>,
}

const STATES: [(&str, Option<CapsState>); 8] = [
    ("default", None),
    ("shift", None),
    ("ctrl", None),
    ("alt", None),
    ("alt+shift", None),
    ("caps", Some(CapsState::Caps)),
    ("caps+shift", Some(CapsState::CapsShift)),
    ("alt+caps", Some(CapsState::AltCaps)),
];

fn layer_of(name: &str) -> Option<Layer> {
    Some(match name {
        "default" => Layer::Default,
        "shift" => Layer::Shift,
        "ctrl" => Layer::Ctrl,
        "alt" => Layer::Alt,
        "alt+shift" => Layer::AltShift,
        "caps" => Layer::Caps,
        "caps+shift" => Layer::CapsShift,
        "alt+caps" => Layer::AltCaps,
        _ => return None,
    })
}

fn cell(input: &LayoutInput, name: &str, position: usize) -> Option<Out> {
    let value = input
        .layers
        .get(&layer_of(name)?)?
        .get(position)?
        .as_ref()?;
    Some(if value.dead {
        Out::Dead(value.text.clone())
    } else {
        Out::Text(value.text.clone())
    })
}

/// What the DLL types in a state at a position.
fn typed(input: &LayoutInput, name: &str, state: Option<CapsState>, position: usize) -> Out {
    match state {
        None => cell(input, name, position).unwrap_or(Out::NoKey),
        Some(state) => windows_caps(&|n| cell(input, n, position), state),
    }
}

/// Whether the migration reports this caps-state position under M09, or
/// a multi-scalar dead key under M08 that explains `v3` against `v4`.
fn explains(
    migration: &Migration,
    state: Option<CapsState>,
    position: &str,
    v3: &Out,
    v4: &Out,
) -> bool {
    let caps = state.is_some_and(|s| {
        migration
            .caps_diffs
            .iter()
            .any(|d| d.platform == "windows" && d.state == s && d.positions.contains(&position))
    });
    let ligature = match (v3, v4) {
        (Out::Text(a), Out::Dead(b)) | (Out::Dead(a), Out::Text(b)) => {
            a == b && a.chars().count() > 1
        }
        _ => false,
    };
    caps || (ligature && migration.defects.iter().any(|d| d.code == Code::M08))
}

fn leaf(node: &DeadKeyNode) -> Option<&str> {
    match node {
        DeadKeyNode::Leaf(text) => Some(text),
        DeadKeyNode::Branch(_) => None,
    }
}

/// The v3 children of the root whose decoded key is `identity`.
fn v3_branch<'a>(
    tree: &'a IndexMap<String, DeadKeyNode>,
    identity: &str,
) -> Option<&'a IndexMap<String, DeadKeyNode>> {
    tree.iter()
        .find(|(k, _)| decode_unicode_escapes(k) == identity)
        .and_then(|(_, node)| match node {
            DeadKeyNode::Branch(children) => Some(children),
            DeadKeyNode::Leaf(_) => None,
        })
}

/// The texts of every value of the input's layers.
fn values(input: &LayoutInput) -> Vec<String> {
    input
        .layers
        .values()
        .flatten()
        .flatten()
        .map(|v| v.text.clone())
        .collect()
}

/// Compares the dead-key trees. The migrated tree holds the dead keys
/// that its layers make dead, one entry per layer value whose result is
/// not Windows' own unmatched behaviour (`ldml.kbdl.dead-tree`). A v3
/// string that the migration decodes is explained by M14 or M06.
fn trees(v3: &LayoutInput, v4: &LayoutInput, migration: &Migration, out: &mut Comparison) {
    let decoded_reported = migration
        .defects
        .iter()
        .any(|d| d.code == Code::M14 || d.code == Code::M06);
    let layer_values = values(v4);
    for (identity, node) in &v4.dead_key_tree {
        let DeadKeyNode::Branch(children) = node else {
            continue;
        };
        let Some(old) = v3_branch(&v3.dead_key_tree, identity) else {
            out.unexplained
                .push(format!("dead key {identity:?}: v3 has no transform"));
            continue;
        };
        let standalone = children.get(STANDALONE).and_then(leaf).unwrap_or(identity);
        for (input, child) in children {
            let v3_child = old
                .iter()
                .find(|(k, _)| decode_unicode_escapes(k) == *input)
                .and_then(|(_, n)| leaf(n));
            let new = leaf(child);
            if v3_child == new {
                continue;
            }
            let message =
                format!("dead key {identity:?} then {input:?}: v3 {v3_child:?}, migrated {new:?}");
            let decoded = v3_child.map(decode_unicode_escapes);
            if decoded.as_deref() == new && decoded_reported {
                out.explained.push(message);
            } else {
                out.unexplained.push(message);
            }
        }
        for (raw, child) in old {
            let input = decode_unicode_escapes(raw);
            if children.contains_key(&input) || !layer_values.contains(&input) {
                continue;
            }
            let Some(text) = leaf(child).map(decode_unicode_escapes) else {
                continue;
            };
            if text != format!("{standalone}{input}") {
                out.unexplained.push(format!(
                    "dead key {identity:?} then {input:?}: v3 {text:?}, migrated has no entry"
                ));
            }
        }
    }
}

/// Builds the v3 layout `tag` of `bundle` both ways and compares the two
/// inputs. The migration must not block.
pub fn compare(bundle: &KbdgenBundle, tag: &LanguageTag) -> Result<Comparison> {
    let layout = bundle
        .layouts
        .get(tag)
        .ok_or_else(|| anyhow!("no v3 layout {tag}"))?;
    let target = layout
        .windows
        .as_ref()
        .ok_or_else(|| anyhow!("layout {tag} has no windows section"))?;
    let (v3, _) = layout_input(bundle, tag, layout, target)?;
    let migration = migrate_bundle_layout(&bundle.path, tag.as_str())?
        .ok_or_else(|| anyhow!("no v3 layout file for {tag}"))?;
    if migration.blocked() {
        bail!(
            "the migration of {tag} is blocked by {:?}",
            migration.blocking_codes()
        );
    }
    let yaml = migration
        .yaml
        .as_deref()
        .ok_or_else(|| anyhow!("{tag} migrated to no text"))?;
    let dir = tempfile::tempdir()?;
    let path = dir.path().join(format!("{tag}.yaml"));
    std::fs::write(&path, yaml).with_context(|| format!("write {}", path.display()))?;
    let model = adapter::load(tag, &path)?
        .ok_or_else(|| anyhow!("the migration of {tag} has no windows keyboard"))?;
    let v4 = adapter::adapt(bundle, &model)?.input;

    let mut out = Comparison::default();
    for (name, state) in STATES {
        for (position, position_name) in POSITION_NAMES.iter().enumerate() {
            let old = typed(&v3, name, state, position);
            let new = typed(&v4, name, state, position);
            if old == new {
                continue;
            }
            let message = format!("{name} {position_name}: v3 {old:?}, migrated {new:?}");
            if explains(&migration, state, position_name, &old, &new) {
                out.explained.push(message);
            } else {
                out.unexplained.push(message);
            }
        }
    }
    trees(&v3, &v4, &migration, &mut out);
    if v3.metadata != v4.metadata {
        out.unexplained.push(format!(
            "metadata: v3 {:?}, migrated {:?}",
            v3.metadata, v4.metadata
        ));
    }
    if v3.decimal != v4.decimal {
        out.unexplained.push(format!(
            "decimal: v3 {:?}, migrated {:?}",
            v3.decimal, v4.decimal
        ));
    }
    Ok(out)
}
