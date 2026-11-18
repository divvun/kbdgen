//! Layout format v4 (`ldml.yaml.*`): the LDML-shaped authoring format,
//! loaded strictly, lowered to one keyboard3 source document per host,
//! and written back from keyboard3 documents by import.
//!
//! Loading ([`load`]) checks every field (`node`), decodes escapes once
//! (`text`), parses rows into tokens (`tokens`), and resolves variant
//! inheritance (`variants`). Lowering ([`lower()`]) gives tokens key ids
//! (`keys`), checks rows against forms and adds implied layers (`layers`),
//! generates the dead-key groups (`deadkeys`), and builds the document
//! with `xmlem` (`document`), which `kbd_ldml::resolve` turns into the
//! model. Import ([`import`]) goes the other way and checks every bit of
//! sugar it writes by lowering it again.

mod deadkeys;
mod document;
mod error;
pub mod import;
mod keys;
mod layers;
mod lower;
pub(crate) mod node;
mod positions;
mod schema;
mod text;
mod tokens;
mod variants;

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::{Mutex, PoisonError};

use serde_yaml::Value;

pub use error::{At, YamlError, YamlProblem};
pub use lower::{HOSTS, lower};
pub use positions::position_scan_code;
pub use schema::Layout4;

use super::LdmlError;

/// The layout formats a bundle may mix (`bundle.layouts`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutFormat {
    /// A layout without `format`.
    V3,
    /// `format: 4`.
    V4,
}

// [spec:kbdgen:def:ldml.yaml.detect]
/// The format of a parsed layout file: v4 when its top-level mapping has
/// `format: 4`, an integer; v3 without `format`. Any other `format` is an
/// error carrying the value as written.
pub fn detect(value: &Value) -> Result<LayoutFormat, String> {
    match value.get("format") {
        None => Ok(LayoutFormat::V3),
        Some(Value::Number(n)) if n.as_u64() == Some(4) => Ok(LayoutFormat::V4),
        Some(other) => Err(serde_yaml::to_string(other)
            .unwrap_or_default()
            .trim()
            .to_string()),
    }
}

/// Reads a layout file as a YAML tree.
pub fn read_yaml(path: &Path) -> Result<Value, LdmlError> {
    let text = std::fs::read_to_string(path).map_err(|source| LdmlError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    serde_yaml::from_str(&text).map_err(|e| LdmlError::Yaml {
        path: path.to_path_buf(),
        message: e.to_string(),
    })
}

/// Loads the v4 layout file `path`, tagged `tag`: the schema with unknown
/// fields refused at any depth, escapes decoded, rows parsed into tokens,
/// and inheritance resolved. Its warnings are logged ([`report`]) and kept
/// in [`Layout4::warnings`].
pub fn load(path: &Path, tag: &str) -> Result<Layout4, LdmlError> {
    let value = read_yaml(path)?;
    let layout = load_value(path, tag, &value)?;
    report(&layout.warnings);
    Ok(layout)
}

// [spec:kbdgen:req:ldml.yaml.strict+1]
/// Logs each warning as a `tracing` warning, once per process: a command
/// may load a layout more than once, as `kbdgen target windows` does to
/// generate the crates and again to name the DLLs it builds.
fn report(warnings: &[YamlProblem]) {
    static REPORTED: Mutex<BTreeSet<String>> = Mutex::new(BTreeSet::new());
    let mut reported = REPORTED.lock().unwrap_or_else(PoisonError::into_inner);
    for warning in warnings {
        let line = warning.to_string();
        if !reported.contains(&line) {
            tracing::warn!("{line}");
            reported.insert(line);
        }
    }
}

/// Loads a layout already parsed as a YAML tree, as if read from `path`,
/// returning its warnings without logging them: how `kbdgen ldml migrate`
/// checks a layout before writing it, reporting them as defects instead.
pub fn load_value(path: &Path, tag: &str, value: &Value) -> Result<Layout4, LdmlError> {
    let file = path.display().to_string();
    match detect(value) {
        Ok(LayoutFormat::V4) => Ok(schema::parse(&file, path, tag, value)?),
        Ok(LayoutFormat::V3) => Err(LdmlError::V3Layout {
            tag: tag.to_string(),
        }),
        Err(value) => Err(LdmlError::Format {
            path: path.to_path_buf(),
            value,
        }),
    }
}

#[cfg(test)]
mod tests;
