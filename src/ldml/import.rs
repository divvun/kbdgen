//! `kbdgen ldml import`: keyboard3 files grouped into layouts.

use std::path::{Path, PathBuf};

use kbd_ldml::{Diagnostic, Resolved, SourceDocument};
use kbd_model::Host;

use super::LdmlError;
use super::layouts::normalise_tag;

/// One XML file being imported: its source document, its resolution, the
/// host it claims (`kbdgen:keyboard@host`, none for the default variant),
/// and the comments it carries, which v4 YAML cannot keep.
#[derive(Debug, Clone)]
pub struct ImportedFile {
    pub path: PathBuf,
    pub host: Option<Host>,
    pub source: SourceDocument,
    pub resolved: Resolved,
    pub comments: usize,
}

/// The files of one layout tag, in the order given.
#[derive(Debug, Clone)]
pub struct ImportGroup {
    pub tag: String,
    pub files: Vec<ImportedFile>,
}

impl ImportGroup {
    /// The reader's and resolver's warnings of every file.
    pub fn warnings(&self) -> impl Iterator<Item = &Diagnostic> {
        self.files.iter().flat_map(|f| f.resolved.warnings.iter())
    }
}

fn read(path: &Path) -> Result<ImportedFile, LdmlError> {
    let bytes = std::fs::read(path).map_err(|source| LdmlError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let name = path.display().to_string();
    let source = kbd_ldml::read_keyboard(&name, Some(path), &bytes)?;
    let resolved = kbd_ldml::resolve(&source)?;
    let comments = kbd_ldml::read::count_comments(&name, &bytes)?;
    Ok(ImportedFile {
        path: path.to_path_buf(),
        host: resolved.keyboard.host,
        source,
        resolved,
        comments,
    })
}

/// The layout tag of a file: `kbdgen:keyboard@tag` if present, otherwise
/// its `locale` normalised as a BCP 47 tag.
fn tag_of(file: &ImportedFile) -> Result<String, LdmlError> {
    let raw = file
        .resolved
        .extensions
        .tag
        .clone()
        .unwrap_or_else(|| file.resolved.keyboard.locale.clone());
    normalise_tag(&raw).ok_or(LdmlError::InvalidTag { tag: raw })
}

// [spec:kbdgen:req:ldml.cli.import]
/// Reads and resolves each file (`ldml.xml.read`, `ldml.xml.resolve`) and
/// groups them by layout tag, in bundle layout order. Two files claiming
/// the same tag and host, or both no host, are an error naming both.
pub fn group(paths: &[PathBuf]) -> Result<Vec<ImportGroup>, LdmlError> {
    let mut groups: Vec<ImportGroup> = Vec::new();
    for path in paths {
        let file = read(path)?;
        let tag = tag_of(&file)?;
        match groups.iter_mut().find(|g| g.tag == tag) {
            Some(group) => {
                if let Some(other) = group.files.iter().find(|f| f.host == file.host) {
                    return Err(LdmlError::DuplicateHost {
                        tag,
                        host: file.host.map_or("default", Host::name),
                        first: other.path.clone(),
                        second: file.path,
                    });
                }
                group.files.push(file);
            }
            None => groups.push(ImportGroup {
                tag,
                files: vec![file],
            }),
        }
    }
    groups.sort_by(|a, b| a.tag.as_bytes().cmp(b.tag.as_bytes()));
    Ok(groups)
}

/// The `layouts/<tag>.yaml` each group writes. An existing file is an
/// error unless `force` is given.
pub fn destinations(
    bundle: &Path,
    groups: &[ImportGroup],
    force: bool,
) -> Result<Vec<PathBuf>, LdmlError> {
    groups
        .iter()
        .map(|g| {
            let path = bundle.join("layouts").join(format!("{}.yaml", g.tag));
            if path.exists() && !force {
                return Err(LdmlError::Exists { path });
            }
            Ok(path)
        })
        .collect()
}

/// What an import reports: per file, the count of dropped comments, then
/// every warning.
pub fn report(groups: &[ImportGroup]) -> Vec<String> {
    let mut lines = Vec::new();
    for group in groups {
        for file in &group.files {
            lines.push(format!(
                "{}: {} comment(s) dropped",
                file.path.display(),
                file.comments
            ));
        }
        lines.extend(group.warnings().map(|w| format!("warning: {w}")));
    }
    lines
}
