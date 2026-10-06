//! Derives the keylayout input of a v4 layout from its compiled model: the
//! `macOS` keyboard's hardware layers and its dead keys run through the
//! engine.

mod classify;
mod dead_keys;
mod layers;
mod modifiers;

#[cfg(test)]
pub(crate) mod tests;

use std::path::Path;
use std::str::FromStr;

use anyhow::{Result, anyhow};
use indexmap::IndexMap;
use kbd_model::{Host, Layout};
use language_tags::LanguageTag;

use super::input::{Binding, Key, KeyMap, KeylayoutInput, NONE_STATE, When};
use super::keymap::MACOS_HARDCODED;
use super::util::keyboard_name;
use crate::build::windows::kbdl::diag::Diagnostics;
use crate::ldml::layouts::{LayoutFile, compiled_layouts, host_documents};
use crate::ldml::yaml::LayoutFormat;
use crate::util::decode_unicode_escapes;

pub use classify::{Category, Classification};
pub use layers::{DECIMAL_CODE, SPACE, positions};

use dead_keys::DeadKeys;
use layers::{Cell, Derived, MapPlan, Value};

/// A v4 layout compiled for macOS: the compiled layout, which has a
/// `macOS` keyboard.
#[derive(Debug, Clone)]
pub struct ModelLayout {
    pub tag: LanguageTag,
    pub layout: Layout,
}

/// A layout's input with the warnings raised deriving it and its
/// classification.
#[derive(Debug, Clone)]
pub struct Adapted {
    pub input: KeylayoutInput,
    pub diag: Diagnostics,
    pub classification: Classification,
}

/// Loads and compiles the v4 layout at `path`. A layout without a `macOS`
/// document has no macOS output, which is `None`.
// [spec:kbdgen:def:ldml.macos.adapter]
pub fn load(tag: &LanguageTag, path: &Path) -> Result<Option<ModelLayout>> {
    let file = LayoutFile {
        tag: tag.to_string(),
        path: path.to_path_buf(),
        format: LayoutFormat::V4,
    };
    let documents = host_documents(&[file], &[])?;
    if !documents
        .iter()
        .any(|document| document.host == Host::MacOs)
    {
        return Ok(None);
    }
    let layout = compiled_layouts(&documents)?
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("layout {tag} compiled to no layout"))?;
    Ok(Some(ModelLayout {
        tag: tag.clone(),
        layout,
    }))
}

/// The layout's display names as language tags, in tag order.
// [spec:kbdgen:req:ldml.macos.target]
fn display_names(layout: &ModelLayout) -> Result<IndexMap<LanguageTag, String>> {
    layout
        .layout
        .display_names
        .iter()
        .map(|(tag, name)| {
            LanguageTag::from_str(tag)
                .map(|tag| (tag, name.clone()))
                .map_err(|e| anyhow!("layout {}: display name tag {tag:?}: {e}", layout.tag))
        })
        .collect()
}

/// Allocates action ids in key emission order.
struct Ids(usize);

impl Ids {
    fn next(&mut self) -> String {
        let id = format!("action{:03}", self.0);
        self.0 += 1;
        id
    }
}

/// The binding of a value: a plain output, or an action whose `none` when
/// types the value or enters its dead-key state, followed by its `when`s
/// for the dead-key states.
// [spec:kbdgen:sem:ldml.macos.dead-keys]
fn binding(derived: &Derived, dead: &DeadKeys, value: usize, ids: &mut Ids) -> Binding {
    let whens = &dead.whens[value];
    let first = match &derived.values[value].value {
        Value::Text(text) if whens.is_empty() => return Binding::Output(text.clone()),
        Value::Text(text) => When::output(NONE_STATE, text),
        Value::Dead(marker) => When::next(NONE_STATE, dead.id(*marker)),
    };
    Binding::Action {
        id: ids.next(),
        whens: std::iter::once(first)
            .chain(whens.iter().cloned())
            .collect(),
    }
}

/// One key map: its ISO keys grouped by value in order of first
/// occurrence, then the fixed keys, the decimal key and the space bar.
// [spec:kbdgen:sem:ldml.macos.keys]
fn key_map(plan: &MapPlan, derived: &Derived, dead: &DeadKeys, ids: &mut Ids) -> KeyMap {
    let mut groups: IndexMap<usize, Vec<&Cell>> = IndexMap::new();
    for cell in &plan.cells {
        groups.entry(cell.value).or_default().push(cell);
    }
    let mut keys = Vec::new();
    for cell in groups.values().flatten() {
        let binding = binding(derived, dead, cell.value, ids);
        keys.push(Key {
            code: cell.code,
            binding,
        });
    }
    for (code, output) in MACOS_HARDCODED {
        keys.push(Key {
            code: *code as u16,
            binding: Binding::Output(decode_unicode_escapes(output)),
        });
    }
    let decimal = match derived.decimal {
        Some(value) => binding(derived, dead, value, ids),
        None => Binding::Output(".".to_owned()),
    };
    keys.push(Key {
        code: DECIMAL_CODE,
        binding: decimal,
    });
    if let Some(cell) = &plan.space {
        let binding = binding(derived, dead, cell.value, ids);
        keys.push(Key {
            code: cell.code,
            binding,
        });
    }
    KeyMap {
        modifiers: plan.modifiers.clone(),
        keys,
    }
}

/// The input, diagnostics and classification of one compiled layout.
// [spec:kbdgen:def:ldml.macos.adapter]
pub fn adapt(layout: &ModelLayout) -> Result<Adapted> {
    let keyboard = layout
        .layout
        .keyboard_for(Host::MacOs)
        .ok_or_else(|| anyhow!("layout {} has no macOS keyboard", layout.tag))?;
    let name = keyboard_name(layout.tag.as_str());
    let mut diag = Diagnostics::new(name.clone());
    let hardware = keyboard.hardware.as_ref().ok_or_else(|| {
        anyhow!(
            "{name}: the macOS keyboard of layout {} has no hardware layers",
            layout.tag
        )
    })?;
    let derived = layers::derive(keyboard, hardware, &mut diag);
    let dead = dead_keys::derive(keyboard, &derived, &mut diag)?;
    let classification = classify::classify(keyboard, hardware, &derived, &dead);
    classify::report(&classification, &mut diag);
    let mut ids = Ids(0);
    let maps = derived
        .maps
        .iter()
        .map(|plan| key_map(plan, &derived, &dead, &mut ids))
        .collect();
    let input = KeylayoutInput {
        name,
        tag: layout.tag.clone(),
        display_names: display_names(layout)?,
        default_index: derived.default_index,
        maps,
        terminators: dead
            .states
            .iter()
            .map(|state| When::output(&state.id, &state.terminator))
            .collect(),
    };
    Ok(Adapted {
        input,
        diag,
        classification,
    })
}
