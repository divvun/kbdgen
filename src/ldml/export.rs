//! `kbdgen ldml export`: host documents to `<OUT>/<tag>.<host>.xml`.

use std::path::{Path, PathBuf};

use super::LdmlError;
use super::compile::Selection;
use super::layouts::HostDocument;

/// The selected documents as (file name, XML), in the given order. Each
/// document's `kbdgen:keyboard@host` is set to its host, so hosts that
/// share a model get files that differ only in that attribute. Every
/// document is written before any file is, so a failure writes nothing.
pub fn render(
    documents: &[HostDocument],
    selection: &Selection,
) -> Result<Vec<(String, String)>, LdmlError> {
    if let Some(tag) = selection
        .layouts
        .iter()
        .find(|t| !documents.iter().any(|d| &d.tag == *t))
    {
        return Err(LdmlError::UnknownLayout(tag.clone()));
    }
    let mut files = Vec::new();
    for document in documents {
        let layout_ok = selection.layouts.is_empty() || selection.layouts.contains(&document.tag);
        let host_ok = selection.hosts.is_empty() || selection.hosts.contains(&document.host);
        if !(layout_ok && host_ok) {
            continue;
        }
        let mut doc = document.source.document.clone();
        kbd_ldml::set_host(&mut doc, document.host);
        files.push((
            format!("{}.{}.xml", document.tag, document.host.name()),
            kbd_ldml::write(&doc),
        ));
    }
    Ok(files)
}

// [spec:kbdgen:req:ldml.cli.export]
/// Writes `<out>/<tag>.<host>.xml` for each selected document, creating
/// `out` with its parents, and returns the written paths in order.
pub fn export(
    documents: &[HostDocument],
    selection: &Selection,
    out: &Path,
) -> Result<Vec<PathBuf>, LdmlError> {
    let files = render(documents, selection)?;
    std::fs::create_dir_all(out).map_err(|source| LdmlError::Io {
        path: out.to_path_buf(),
        source,
    })?;
    files
        .into_iter()
        .map(|(name, xml)| {
            let path = out.join(name);
            std::fs::write(&path, xml).map_err(|source| LdmlError::Io {
                path: path.clone(),
                source,
            })?;
            Ok(path)
        })
        .collect()
}
