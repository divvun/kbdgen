//! Imports, implied keys, and overrides (`ldml.xml.import`,
//! `ldml.xml.implied`).

use std::path::{Path, PathBuf};

use crate::cldr;
use crate::diag::{Diagnostic, Result};
use crate::read::{SourceDocument, read_import};
use crate::tree::El;

/// The files imported so far: `cldr:<path>` or a canonical local path.
type Seen = Vec<String>;

fn load(import: &El, dir: Option<&Path>) -> Result<(String, SourceDocument)> {
    let path = import
        .attr("path")
        .ok_or_else(|| import.attr_error("path", "import needs a path"))?;
    match import.attr("base") {
        Some("cldr") => {
            let text = cldr::file(path).ok_or_else(|| {
                import.attr_error(
                    "path",
                    format!("{path} is not an embedded CLDR import (releases 45 to 48)"),
                )
            })?;
            let name = format!("cldr:{path}");
            let mut source = read_import(&name, None, text.as_bytes())?;
            source.warnings.clear();
            Ok((name, source))
        }
        Some(other) => Err(import.attr_error("base", format!("unknown base {other:?}"))),
        None => {
            let dir = dir.ok_or_else(|| {
                import.attr_error("path", "a local import needs a document read from a file")
            })?;
            let file: PathBuf = dir.join(path);
            let bytes = std::fs::read(&file).map_err(|e| {
                import.attr_error("path", format!("cannot read {}: {e}", file.display()))
            })?;
            let identity = std::fs::canonicalize(&file)
                .unwrap_or_else(|_| file.clone())
                .display()
                .to_string();
            let source = read_import(&file.display().to_string(), Some(&file), &bytes)?;
            Ok((identity, source))
        }
    }
}

/// Replaces every `import` below `el` by the children of the imported
/// root, recursively. `special` content is never searched.
fn splice(
    el: &mut El,
    dir: Option<&Path>,
    seen: &mut Seen,
    warnings: &mut Vec<Diagnostic>,
) -> Result<()> {
    if el.prefix.is_some() || el.name == "special" {
        return Ok(());
    }
    let mut out = Vec::with_capacity(el.children.len());
    for mut child in std::mem::take(&mut el.children) {
        if child.prefix.is_some() || child.name != "import" {
            splice(&mut child, dir, seen, warnings)?;
            out.push(child);
            continue;
        }
        let (identity, source) = load(&child, dir)?;
        if seen.contains(&identity) {
            return Err(child.error(format!(
                "{identity} is imported twice; each file is imported at most once"
            )));
        }
        seen.push(identity);
        warnings.extend(source.warnings.iter().cloned());
        let mut imported = El::from_document(&source.name, &source.document)?;
        if imported.prefix.is_some() || imported.name != el.name {
            return Err(child.error(format!(
                "imported root {} does not match the importing {}",
                imported.qualified(),
                el.name
            )));
        }
        let imported_dir = source.path.as_deref().and_then(Path::parent);
        splice(&mut imported, imported_dir.or(dir), seen, warnings)?;
        out.extend(imported.children);
    }
    el.children = out;
    Ok(())
}

// [spec:kbdgen:sem:ldml.xml.implied]
/// Prepends the implied keys of `keys-Latn-implied.xml`, at the version
/// the keyboard conforms to, to the keyboard's `keys`.
fn implied_keys(root: &mut El) -> Result<()> {
    let version = root
        .attr("conformsTo")
        .and_then(|v| v.parse::<u8>().ok())
        .map_or(cldr::VERSIONS[0], cldr::implied_version);
    let path = format!("{version}/keys-Latn-implied.xml");
    let text = cldr::file(&path).ok_or_else(|| root.error(format!("{path} is not embedded")))?;
    let source = read_import(&format!("cldr:{path}"), None, text.as_bytes())?;
    let implied = El::from_document(&source.name, &source.document)?;
    let index = match root
        .children
        .iter()
        .position(|c| c.prefix.is_none() && c.name == "keys")
    {
        Some(i) => i,
        None => {
            let keys = El {
                prefix: None,
                name: "keys".to_string(),
                attrs: Vec::new(),
                children: Vec::new(),
                file: root.file.clone(),
                path: format!("{}/keys", root.path),
            };
            let at = root
                .children
                .iter()
                .position(|c| {
                    c.prefix.is_none()
                        && matches!(
                            c.name.as_str(),
                            "flicks" | "forms" | "layers" | "variables" | "transforms" | "special"
                        )
                })
                .unwrap_or(root.children.len());
            root.children.insert(at, keys);
            at
        }
    };
    if let Some(keys) = root.children.get_mut(index) {
        let own = std::mem::take(&mut keys.children);
        keys.children = implied.children;
        keys.children.extend(own);
    }
    Ok(())
}

/// The identity by which a later element replaces an earlier one, if
/// `el` has one.
fn identity(parent: &str, el: &El) -> Option<String> {
    if el.prefix.is_some() {
        return None;
    }
    match (parent, el.name.as_str()) {
        ("keys", "key")
        | ("flicks", "flick")
        | ("forms", "form")
        | ("variables", "string" | "set" | "uset") => {
            el.attr("id").map(|id| format!("{}#{id}", el.name))
        }
        ("layers", "layer") => match el.attr("id") {
            Some(id) => Some(format!("layer#{id}")),
            None => el.attr("modifiers").map(|m| format!("modifiers#{m}")),
        },
        _ => None,
    }
}

/// A later element with the same identity replaces the earlier one in
/// place; displays are matched on the model, after decoding.
fn overrides(el: &mut El) {
    if el.prefix.is_some() || el.name == "special" {
        return;
    }
    let mut out: Vec<El> = Vec::with_capacity(el.children.len());
    let mut ids: Vec<Option<String>> = Vec::new();
    for mut child in std::mem::take(&mut el.children) {
        overrides(&mut child);
        let id = identity(&el.name, &child);
        if let Some(id) = &id
            && let Some(i) = ids.iter().position(|x| x.as_ref() == Some(id))
            && let Some(slot) = out.get_mut(i)
        {
            *slot = child;
            continue;
        }
        ids.push(id);
        out.push(child);
    }
    el.children = out;
}

// [spec:kbdgen:sem:ldml.xml.import]
/// Splices imports, adds the implied keys and applies overrides. Each
/// file is imported at most once per keyboard; a repeat is an error.
pub(super) fn expand(
    source: &SourceDocument,
    mut root: El,
    warnings: &mut Vec<Diagnostic>,
) -> Result<El> {
    let dir = source.path.as_deref().and_then(Path::parent);
    let mut seen = Seen::new();
    splice(&mut root, dir, &mut seen, warnings)?;
    implied_keys(&mut root)?;
    overrides(&mut root);
    Ok(root)
}
