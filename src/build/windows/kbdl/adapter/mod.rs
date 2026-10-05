//! Derives the generator input of a v4 layout from its compiled model: the
//! `windows` keyboard's hardware layers, dead keys run through the engine,
//! its Windows options, and the layout's `targets.windows`.

mod classify;
mod dead_tree;
mod layers;

#[cfg(test)]
mod tests;

use std::path::Path;

use anyhow::{Context, Result, anyhow};
use indexmap::IndexMap;
use kbd_model::{Host, Keyboard, Layout};
use language_tags::LanguageTag;

use crate::{
    bundle::KbdgenBundle,
    ldml::{
        compile::{Selection, encode},
        layouts::{HostDocument, LayoutFile, compiled_layouts, host_documents},
        yaml::LayoutFormat,
    },
};

use super::{
    bundle::{keyboard_name, metadata},
    diag::Diagnostics,
    input::{ExtraModifierKey, KeyNameEntry, KeyNameTable, LayoutInput},
    tables::{KEY_NAMES, KEY_NAMES_EXT},
};

pub use classify::{Category, Classification};

/// A v4 layout compiled for Windows: the compiled layout, which has a
/// `windows` keyboard, and its `targets.windows` `id` and `locale`.
#[derive(Debug, Clone)]
pub struct ModelLayout {
    pub tag: LanguageTag,
    pub layout: Layout,
    pub id: Option<String>,
    pub locale: Option<String>,
}

/// A layout's input with the warnings raised deriving it, the
/// classification, and the encoded `windows` keyboard.
#[derive(Debug, Clone)]
pub struct Adapted {
    pub input: LayoutInput,
    pub diag: Diagnostics,
    pub classification: Classification,
    pub model: Vec<u8>,
}

/// The `targets.windows` value `name` of a host document's kbdgen data.
fn windows_target(extensions: &kbd_ldml::Extensions, name: &str) -> Option<String> {
    extensions
        .targets
        .iter()
        .find(|target| target.host == Host::Windows.name() && target.name == name)
        .map(|target| target.value.clone())
}

/// The host documents of the v4 layout at `path` and the kbdgen data of its
/// `windows` document, or `None` when it has none.
fn windows_documents(
    tag: &LanguageTag,
    path: &Path,
) -> Result<Option<(Vec<HostDocument>, kbd_ldml::Extensions)>> {
    let file = LayoutFile {
        tag: tag.to_string(),
        path: path.to_path_buf(),
        format: LayoutFormat::V4,
    };
    let documents = host_documents(&[file], &[])?;
    let Some(windows) = documents
        .iter()
        .find(|document| document.host == Host::Windows)
    else {
        return Ok(None);
    };
    let extensions = kbd_ldml::resolve(&windows.source)?.extensions;
    Ok(Some((documents, extensions)))
}

/// Loads and compiles the v4 layout at `path`. A layout without a `windows`
/// document has no Windows output, which is `None`.
// [spec:kbdgen:def:ldml.kbdl.adapter]
pub fn load(tag: &LanguageTag, path: &Path) -> Result<Option<ModelLayout>> {
    let Some((documents, extensions)) = windows_documents(tag, path)? else {
        return Ok(None);
    };
    let layout = compiled_layouts(&documents)?
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("layout {tag} compiled to no layout"))?;
    Ok(Some(ModelLayout {
        tag: tag.clone(),
        layout,
        id: windows_target(&extensions, "id"),
        locale: windows_target(&extensions, "locale"),
    }))
}

/// The keyboard name of the v4 layout at `path`, if it has Windows output.
/// Only lowering and resolution run, so nothing is reported twice.
// [spec:kbdgen:req:ldml.kbdl.metadata]
pub fn layout_name(tag: &LanguageTag, path: &Path) -> Result<Option<String>> {
    Ok(windows_documents(tag, path)?
        .map(|(_, extensions)| keyboard_name(tag, windows_target(&extensions, "id").as_deref())))
}

/// The input, diagnostics and model of one compiled layout.
// [spec:kbdgen:def:ldml.kbdl.adapter]
pub fn adapt(bundle: &KbdgenBundle, layout: &ModelLayout) -> Result<Adapted> {
    let keyboard = layout
        .layout
        .keyboard_for(Host::Windows)
        .ok_or_else(|| anyhow!("layout {} has no windows keyboard", layout.tag))?;
    let metadata = layout_metadata(bundle, layout)?;
    let mut diag = Diagnostics::new(metadata.name.clone());
    let Some(hardware) = &keyboard.hardware else {
        return Err(anyhow!(
            "{}: the windows keyboard of layout {} has no hardware layers",
            metadata.name,
            layout.tag
        ));
    };
    let derived = layers::derive(keyboard, hardware, &mut diag)?;
    let dead_key_tree = dead_tree::derive(keyboard, &derived, &mut diag)?;
    let classification = classify::classify(keyboard, &dead_key_tree, &derived.mixed);
    classify::report(&classification, &mut diag);

    let mut input = LayoutInput {
        metadata,
        layers: derived.layers,
        dead_key_tree,
        ..LayoutInput::default()
    };
    windows_inputs(keyboard, &derived.dead_keys, &mut input, &mut diag);

    let selection = Selection {
        layouts: vec![layout.layout.tag.clone()],
        hosts: vec![Host::Windows],
    };
    let model = encode(std::slice::from_ref(&layout.layout), &selection)?
        .into_iter()
        .next()
        .map(|(_, bytes)| bytes)
        .ok_or_else(|| anyhow!("layout {}: no windows model was encoded", layout.tag))?;
    Ok(Adapted {
        input,
        diag,
        classification,
        model,
    })
}

/// `kbdl.metadata.bundle` with the `id` and `locale` of `targets.windows`
/// and the compiled layout's display name for its primary language subtag.
// [spec:kbdgen:req:ldml.kbdl.metadata]
fn layout_metadata(bundle: &KbdgenBundle, layout: &ModelLayout) -> Result<super::input::Metadata> {
    let primary = layout.tag.primary_language();
    let display_name = layout
        .layout
        .display_names
        .iter()
        .find(|(tag, _)| tag.eq_ignore_ascii_case(primary))
        .map(|(_, name)| name.as_str());
    metadata(
        bundle,
        &layout.tag,
        display_name,
        layout.id.as_deref(),
        layout.locale.as_deref(),
    )
    .with_context(|| format!("Windows metadata of layout {}", layout.tag))
}

fn key_name_entry(name: &str) -> Option<KeyNameEntry> {
    let find = |table: &[(u8, &str)], kind| {
        table
            .iter()
            .find(|(_, entry)| *entry == name)
            .map(|(scan_code, _)| KeyNameEntry {
                table: kind,
                scan_code: *scan_code,
            })
    };
    find(&KEY_NAMES, KeyNameTable::Normal).or_else(|| find(&KEY_NAMES_EXT, KeyNameTable::Extended))
}

/// The Windows-only parts of the input: extra modifiers, flags, dead-key
/// names by dead identity, key-name overrides and the decimal separator.
fn windows_inputs(
    keyboard: &Keyboard,
    dead_keys: &[layers::DeadKey],
    input: &mut LayoutInput,
    diag: &mut Diagnostics,
) {
    let windows = &keyboard.windows;
    input.extra_modifiers = windows
        .extra_modifiers
        .iter()
        .map(|key| match key {
            kbd_model::ExtraModifierKey::RightCtrl => ExtraModifierKey::RightCtrl,
            kbd_model::ExtraModifierKey::CapsLock => ExtraModifierKey::CapsLock,
            kbd_model::ExtraModifierKey::B00 => ExtraModifierKey::B00,
        })
        .collect();
    input.shift_lock = windows.shift_lock;
    input.lrm_rlm = windows.lrm_rlm;
    input.decimal = keyboard.decimal.as_ref().map(|decimal| decimal.plain());
    let mut names = IndexMap::new();
    for dead in dead_keys {
        if let Some(name) = keyboard.dead_key_names.get(&dead.marker) {
            names.insert(dead.identity.clone(), name.clone());
        }
    }
    input.dead_key_names = names;
    for (english, name) in &windows.key_names {
        match key_name_entry(english) {
            Some(entry) => {
                input.key_name_overrides.insert(entry, name.clone());
            }
            None => diag.warn(format!(
                "key name {english:?}: no key-name table entry has that name; ignored"
            )),
        }
    }
}
