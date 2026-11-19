//! The layouts with Linux output (`ldml.linux.input`): the `linux` keyboard
//! of each v4 layout that has one, and of the in-memory migration of each
//! v3 layout with a `windows` or `chromeOS` section, encoded as the engine
//! reads it.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result, anyhow, bail};
use kbd_model::{Host, Keyboard};
use language_tags::LanguageTag;

use crate::build::migration;
use crate::bundle::KbdgenBundle;
use crate::ldml::compile::{Selection, encode};
use crate::ldml::layouts::{HostDocument, LayoutFile, compiled_layouts, host_documents};
use crate::ldml::yaml::LayoutFormat;

/// One layout's `linux` keyboard K, the adapter's input.
///
/// The on-screen layouts present K's hardware set as touch
/// (`ldml.linux.touch`). The `linux` host has no touch chain
/// (`ldml.yaml.hosts`), so K has touch sets only from an LDML file a v4
/// layout names (`ldml.yaml.ldml-ref`), and those are not shown.
// [spec:kbdgen:def:ldml.linux.adapter]
#[derive(Debug, Clone)]
pub struct LinuxLayout {
    pub tag: LanguageTag,
    /// The layout's file name in `layouts/`.
    pub file: String,
    /// Tag → display name.
    pub display_names: BTreeMap<String, String>,
    /// K, which has hardware layers.
    pub keyboard: Keyboard,
    /// K in the encoding of `ldml.model.encoding`.
    pub model: Vec<u8>,
}

#[derive(Clone, Copy)]
enum Source<'a> {
    V3(&'a LanguageTag),
    V4(&'a LanguageTag, &'a Path),
}

impl Source<'_> {
    fn tag(&self) -> &LanguageTag {
        match self {
            Source::V3(tag) | Source::V4(tag, _) => tag,
        }
    }
}

/// The v3 layouts with a `windows` or `chromeOS` section and every v4
/// layout, in bundle layout order.
fn sources(bundle: &KbdgenBundle) -> Vec<Source<'_>> {
    let mut sources: Vec<Source> = bundle
        .layouts
        .iter()
        .filter_map(|(tag, layout)| {
            if layout.windows.is_some() || layout.chrome_os.is_some() {
                Some(Source::V3(tag))
            } else {
                tracing::debug!("Skipping {tag}: no windows or chromeOS section");
                None
            }
        })
        .chain(
            bundle
                .v4_layouts
                .iter()
                .map(|(tag, path)| Source::V4(tag, path)),
        )
        .collect();
    sources.sort_by(|a, b| a.tag().as_str().cmp(b.tag().as_str()));
    sources
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// The migrated file's name and the host documents of its migration,
/// whose `linux` variant inherits its `windows` one, else its `chromeOS`
/// one (`ldml.migrate.platforms`).
// [spec:kbdgen:def:ldml.linux.input+1]
fn migrated_documents(
    bundle: &KbdgenBundle,
    tag: &LanguageTag,
) -> Result<(String, Vec<HostDocument>)> {
    let migration = migration::unblocked(bundle, tag)?;
    let documents = migration::host_documents(tag, &migration)?;
    Ok((file_name(&migration.path), documents))
}

/// Compiles the `linux` document among `documents`; none when there is no
/// such document.
// [spec:kbdgen:def:ldml.linux.input+1]
fn compile(
    tag: &LanguageTag,
    file: String,
    documents: Vec<HostDocument>,
) -> Result<Option<LinuxLayout>> {
    let Some(document) = documents.into_iter().find(|d| d.host == Host::Linux) else {
        return Ok(None);
    };
    let layout = compiled_layouts(std::slice::from_ref(&document))
        .with_context(|| format!("{tag}: compile the linux keyboard"))?
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("{tag}: the linux document compiled to no layout"))?;
    let keyboard = layout
        .keyboard_for(Host::Linux)
        .cloned()
        .ok_or_else(|| anyhow!("{tag}: the compiled layout has no linux keyboard"))?;
    if keyboard.hardware.is_none() {
        bail!("{tag}: the linux keyboard has no hardware layers");
    }
    let selection = Selection {
        layouts: vec![layout.tag.clone()],
        hosts: vec![Host::Linux],
    };
    let model = encode(std::slice::from_ref(&layout), &selection)?
        .into_iter()
        .next()
        .map(|(_, bytes)| bytes)
        .ok_or_else(|| anyhow!("{tag}: no linux model was encoded"))?;
    Ok(Some(LinuxLayout {
        tag: tag.clone(),
        file,
        display_names: layout.display_names,
        keyboard,
        model,
    }))
}

/// How a v4 layout without a `linux` keyboard would get one: by inheriting
/// its `windows` variant, else its `chromeOS` one, as a migrated v3
/// layout does (`ldml.migrate.platforms`), else with a variant of its own.
fn hint(documents: &[HostDocument]) -> String {
    let has = |host: Host| documents.iter().any(|d| d.host == host);
    match [Host::Windows, Host::ChromeOs]
        .into_iter()
        .find(|&host| has(host))
    {
        Some(host) => format!(
            "`hardware.linux: {{inherits: {}}}` would give it its {} one",
            host.name(),
            if host == Host::Windows {
                "Windows"
            } else {
                "ChromeOS"
            }
        ),
        None => "a `linux` or `default` hardware variant would give it one".to_owned(),
    }
}

/// Every layout of the bundle with Linux output, in bundle layout order.
/// A v3 layout whose migration fails is an error, and a v4 layout without
/// a `linux` keyboard has no output.
// [spec:kbdgen:def:ldml.linux.input+1]
pub fn layouts(bundle: &KbdgenBundle) -> Result<Vec<LinuxLayout>> {
    let mut layouts = Vec::new();
    for source in sources(bundle) {
        let tag = source.tag();
        let (file, documents) = match source {
            Source::V3(_) => migrated_documents(bundle, tag)?,
            Source::V4(_, path) => {
                let file = LayoutFile {
                    tag: tag.to_string(),
                    path: path.to_path_buf(),
                    format: LayoutFormat::V4,
                };
                let documents =
                    host_documents(&[file], &[]).with_context(|| format!("{tag}: load"))?;
                (file_name(path), documents)
            }
        };
        let hint = hint(&documents);
        match compile(tag, file, documents)? {
            Some(layout) => layouts.push(layout),
            None => tracing::debug!("Skipping {tag}: no linux keyboard; {hint}"),
        }
    }
    Ok(layouts)
}
