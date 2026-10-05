//! A bundle's layouts as the `kbdgen ldml` commands load them: layout files
//! in bundle layout order, the host documents of each layout, and compiled
//! layouts resolved from host documents.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use kbd_ldml::SourceDocument;
use kbd_model::{Host, Layout};
use language_tags::LanguageTag;

use super::LdmlError;

/// A source document lowered for one host of a layout
/// (`ldml.yaml.hosts`); its `kbdgen:keyboard@host` names `host`.
#[derive(Debug, Clone)]
pub struct HostDocument {
    pub tag: String,
    pub host: Host,
    pub source: SourceDocument,
}

/// The layout formats a bundle may mix (`bundle.layouts`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutFormat {
    /// A layout without `format`.
    V3,
    /// `format: 4`.
    V4,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutFile {
    /// The file stem as a normalised BCP 47 tag.
    pub tag: String,
    pub path: PathBuf,
    pub format: LayoutFormat,
}

/// Normalises a BCP 47 tag the way bundle layout stems are (`se-fi` →
/// `se-FI`).
pub fn normalise_tag(tag: &str) -> Option<String> {
    LanguageTag::parse(tag).ok().map(|t| t.to_string())
}

fn format_of(path: &Path) -> Result<LayoutFormat, LdmlError> {
    let text = std::fs::read_to_string(path).map_err(|source| LdmlError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let value: serde_yaml::Value = serde_yaml::from_str(&text).map_err(|e| LdmlError::Yaml {
        path: path.to_path_buf(),
        message: e.to_string(),
    })?;
    match value.get("format") {
        None => Ok(LayoutFormat::V3),
        Some(serde_yaml::Value::Number(n)) if n.as_u64() == Some(4) => Ok(LayoutFormat::V4),
        Some(other) => Err(LdmlError::Format {
            path: path.to_path_buf(),
            value: serde_yaml::to_string(other)
                .unwrap_or_default()
                .trim()
                .to_string(),
        }),
    }
}

/// The bundle's `layouts/*.yaml` in bundle layout order: ascending by the
/// normalised tag, compared byte-wise, a later file replacing an earlier
/// one whose stem normalises to the same tag.
pub fn layout_files(bundle: &Path) -> Result<Vec<LayoutFile>, LdmlError> {
    let dir = bundle.join("layouts");
    let entries = std::fs::read_dir(&dir).map_err(|source| LdmlError::Io {
        path: dir.clone(),
        source,
    })?;
    let mut paths: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.extension().is_some_and(|e| e == "yaml"))
        .collect();
    paths.sort_by(|a, b| {
        a.file_name()
            .map(|n| n.as_encoded_bytes())
            .cmp(&b.file_name().map(|n| n.as_encoded_bytes()))
    });
    let mut files: BTreeMap<String, LayoutFile> = BTreeMap::new();
    for path in paths {
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let tag = normalise_tag(&stem).ok_or(LdmlError::InvalidTag { tag: stem })?;
        let format = format_of(&path)?;
        files.insert(tag.clone(), LayoutFile { tag, path, format });
    }
    Ok(files.into_values().collect())
}

/// The host documents of the selected layouts, in bundle layout order and
/// host order. A v3 layout is an error suggesting `kbdgen ldml migrate`.
/// Lowering a v4 layout to its host documents is `ldml.yaml.lowering`,
/// which this build does not provide, so a v4 layout is an error naming
/// it.
pub fn host_documents(
    files: &[LayoutFile],
    layouts: &[String],
) -> Result<Vec<HostDocument>, LdmlError> {
    if let Some(tag) = layouts.iter().find(|t| !files.iter().any(|f| &f.tag == *t)) {
        return Err(LdmlError::UnknownLayout(tag.clone()));
    }
    let selected: Vec<&LayoutFile> = files
        .iter()
        .filter(|f| layouts.is_empty() || layouts.contains(&f.tag))
        .collect();
    if let Some(v3) = selected.iter().find(|f| f.format == LayoutFormat::V3) {
        return Err(LdmlError::V3Layout {
            tag: v3.tag.clone(),
        });
    }
    match selected.first() {
        None => Ok(Vec::new()),
        Some(file) => Err(LdmlError::LoweringUnavailable {
            tag: file.tag.clone(),
        }),
    }
}

/// Resolves each host document (`ldml.xml.resolve`), checks that the
/// engine loads it (`Model::from_keyboard`), and builds one compiled
/// layout per tag (`ldml.model.layout`), whose display names come from the
/// documents' `kbdgen:displayName`. Documents of one tag are consecutive.
pub fn compiled_layouts(documents: &[HostDocument]) -> Result<Vec<Layout>, LdmlError> {
    let mut layouts = Vec::new();
    let mut i = 0;
    while let Some(first) = documents.get(i) {
        let group: Vec<&HostDocument> = documents[i..]
            .iter()
            .take_while(|d| d.tag == first.tag)
            .collect();
        i += group.len();
        let mut keyboards = Vec::new();
        let mut display_names = BTreeMap::new();
        for document in group {
            let resolved = kbd_ldml::resolve(&document.source)?;
            let mut keyboard = resolved.keyboard;
            keyboard.host = Some(document.host);
            kbd_engine::Model::from_keyboard(keyboard.clone(), kbd_engine::Options::default())
                .map_err(|source| LdmlError::Engine {
                    tag: document.tag.clone(),
                    host: document.host.name(),
                    source,
                })?;
            display_names.extend(resolved.extensions.display_names);
            keyboards.push(keyboard);
        }
        let layout = Layout::from_host_documents(first.tag.clone(), display_names, keyboards)
            .map_err(|e| LdmlError::Layout {
                tag: first.tag.clone(),
                message: e.to_string(),
            })?;
        layouts.push(layout);
    }
    Ok(layouts)
}
