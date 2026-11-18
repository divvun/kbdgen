//! The layouts with ChromeOS output (`ldml.chromeos.input`): the
//! `chromeOS` keyboard of each v4 layout, and of the in-memory migration of
//! each v3 layout with a `chromeOS` section, encoded as the engine reads
//! it.

use std::path::Path;
use std::str::FromStr;

use anyhow::{Context, Result, anyhow, bail};
use indexmap::IndexMap;
use kbd_model::Host;
use language_tags::LanguageTag;

use super::classify;
use crate::build::diag::{Diagnostics, Platform};
use crate::bundle::KbdgenBundle;
use crate::ldml::compile::{Selection, encode};
use crate::ldml::layouts::{HostDocument, LayoutFile, compiled_layouts, host_documents};
use crate::ldml::migrate::defect::Code;
use crate::ldml::migrate::migrate_bundle_layout;
use crate::ldml::yaml::LayoutFormat;

/// The input component's language and XKB layout when the layout's
/// `targets.chromeOS` names none.
pub const DEFAULT_LANGUAGE: &str = "en-US";
pub const DEFAULT_XKB_LAYOUT: &str = "us";

/// One layout's ChromeOS keyboard and what its input component needs.
#[derive(Debug, Clone)]
pub struct ChromeOsLayout {
    pub tag: LanguageTag,
    pub display_names: IndexMap<LanguageTag, String>,
    pub language: String,
    pub xkb_layout: String,
    /// The `chromeOS` keyboard in the encoding of `ldml.model.encoding`.
    pub model: Vec<u8>,
}

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

/// The v3 layouts with a `chromeOS` section and every v4 layout, in bundle
/// layout order.
fn sources(bundle: &KbdgenBundle) -> Vec<Source<'_>> {
    let mut sources: Vec<Source> = bundle
        .layouts
        .iter()
        .filter_map(|(tag, layout)| match &layout.chrome_os {
            Some(_) => Some(Source::V3(tag)),
            None => {
                tracing::debug!("Skipping {tag}: no chromeOS section");
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

/// The host documents of a v3 layout's migration, which must not block
/// and must have a `chromeOS` document, since the extension has no other
/// way to type the layout.
fn migrated_documents(bundle: &KbdgenBundle, tag: &LanguageTag) -> Result<Vec<HostDocument>> {
    let migration = migrate_bundle_layout(&bundle.path, tag.as_str())
        .with_context(|| format!("{tag}: migrate the v3 layout"))?
        .ok_or_else(|| anyhow!("{tag}: the bundle has no v3 layout file to migrate"))?;
    if migration.blocked() {
        let codes: Vec<&str> = migration
            .blocking_codes()
            .into_iter()
            .map(Code::name)
            .collect();
        bail!(
            "{tag}: the v3 layout's migration is blocked by defects {}; `kbdgen ldml migrate --dry-run` reports them",
            codes.join(", ")
        );
    }
    let documents = migration
        .host_documents()
        .with_context(|| format!("{tag}: lower the migrated layout"))?
        .ok_or_else(|| anyhow!("{tag}: the v3 layout migrated to no layout"))?;
    if !documents.iter().any(|d| d.host == Host::ChromeOs) {
        bail!("{tag}: the v3 layout's migration has no chromeOS keyboard");
    }
    Ok(documents)
}

/// The value of `targets.chromeOS.<name>` among a document's kbdgen data.
fn target(extensions: &kbd_ldml::Extensions, name: &str) -> Option<String> {
    extensions
        .targets
        .iter()
        .find(|t| t.host == Host::ChromeOs.name() && t.name == name)
        .map(|t| t.value.clone())
}

/// Compiles the `chromeOS` document among `documents`; none when there is
/// no such document.
fn compile(tag: &LanguageTag, documents: Vec<HostDocument>) -> Result<Option<ChromeOsLayout>> {
    let Some(document) = documents.into_iter().find(|d| d.host == Host::ChromeOs) else {
        return Ok(None);
    };
    let extensions = kbd_ldml::resolve(&document.source)
        .with_context(|| format!("{tag}: resolve the chromeOS keyboard"))?
        .extensions;
    let layout = compiled_layouts(std::slice::from_ref(&document))?
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("{tag}: the chromeOS document compiled to no layout"))?;
    let keyboard = layout
        .keyboard_for(Host::ChromeOs)
        .cloned()
        .ok_or_else(|| anyhow!("{tag}: the compiled layout has no chromeOS keyboard"))?;
    if keyboard.hardware.is_none() {
        bail!("{tag}: the chromeOS keyboard has no hardware layers");
    }
    let selection = Selection {
        layouts: vec![layout.tag.clone()],
        hosts: vec![Host::ChromeOs],
    };
    let model = encode(std::slice::from_ref(&layout), &selection)?
        .into_iter()
        .next()
        .map(|(_, bytes)| bytes)
        .ok_or_else(|| anyhow!("{tag}: no chromeOS model was encoded"))?;
    let display_names = layout
        .display_names
        .iter()
        .map(|(language, name)| {
            LanguageTag::from_str(language)
                .map(|language| (language, name.clone()))
                .map_err(|e| anyhow!("{tag}: display name tag {language:?}: {e}"))
        })
        .collect::<Result<_>>()?;
    let mut diag = Diagnostics::new(Platform::ChromeOs, tag.to_string());
    classify::report(&classify::classify(&keyboard), &mut diag);
    Ok(Some(ChromeOsLayout {
        tag: tag.clone(),
        display_names,
        language: target(&extensions, "locale").unwrap_or_else(|| DEFAULT_LANGUAGE.to_owned()),
        xkb_layout: target(&extensions, "xkbLayout")
            .unwrap_or_else(|| DEFAULT_XKB_LAYOUT.to_owned()),
        model,
    }))
}

/// Every layout of the bundle with ChromeOS output, in bundle layout order.
/// A v3 layout whose migration fails is an error, and a v4 layout without
/// a `chromeOS` keyboard has no output.
// [spec:kbdgen:def:ldml.chromeos.input]
pub fn layouts(bundle: &KbdgenBundle) -> Result<Vec<ChromeOsLayout>> {
    let mut layouts = Vec::new();
    for source in sources(bundle) {
        let tag = source.tag();
        tracing::info!("Compiling {tag} for ChromeOS");
        let documents = match source {
            Source::V3(tag) => migrated_documents(bundle, tag)?,
            Source::V4(tag, path) => {
                let file = LayoutFile {
                    tag: tag.to_string(),
                    path: path.to_path_buf(),
                    format: LayoutFormat::V4,
                };
                host_documents(&[file], &[]).with_context(|| format!("{tag}: load"))?
            }
        };
        match compile(tag, documents)? {
            Some(layout) => layouts.push(layout),
            None => tracing::debug!("Skipping {tag}: no chromeOS keyboard"),
        }
    }
    Ok(layouts)
}
