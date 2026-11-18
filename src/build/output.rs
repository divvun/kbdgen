//! Writing a target's files into the output directory.

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

pub fn write(path: &Path, contents: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    fs::write(path, contents).with_context(|| format!("write {}", path.display()))?;
    tracing::debug!("Wrote {}", path.display());
    Ok(())
}

/// Empties `dir`, which the generator owns, so that nothing of an earlier
/// run stays behind.
pub fn fresh_dir(dir: &Path) -> Result<()> {
    if dir.exists() {
        fs::remove_dir_all(dir).with_context(|| format!("remove stale {}", dir.display()))?;
    }
    fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))
}
