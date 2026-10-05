//! A bundle's own vectors (`ldml.test.bundle`): `tests/*.yaml` in the
//! golden-vector format, with paths relative to the bundle.

use std::path::{Path, PathBuf};

use super::Report;
use super::cldr::run_cldr;
use super::run::run_vector_file;
use crate::ldml::LdmlError;

// [spec:kbdgen:req:ldml.test.bundle+1]
/// The bundle's `tests/*.yaml`, regular files only, in byte-wise file-name
/// order. A bundle without `tests/` has none. Bundle loading
/// (`bundle.structure`) reads only `layouts/`, `targets/` and
/// `resources/`, so it never sees these files.
pub fn bundle_tests(bundle: &Path) -> Result<Vec<PathBuf>, LdmlError> {
    if !bundle.is_dir() {
        return Err(LdmlError::Io {
            path: bundle.to_path_buf(),
            source: std::io::Error::new(std::io::ErrorKind::NotFound, "no bundle directory"),
        });
    }
    let dir = bundle.join("tests");
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
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
    Ok(paths)
}

// [spec:kbdgen:req:ldml.cli.test]
/// Runs every vector file of the bundle, then, with `cldr`, the vendored
/// CLDR vectors. Each file is named by its path relative to the bundle.
pub fn run_bundle(bundle: &Path, cldr: bool) -> Result<Report, LdmlError> {
    let mut report = Report::default();
    for path in bundle_tests(bundle)? {
        let label = path
            .strip_prefix(bundle)
            .unwrap_or(&path)
            .display()
            .to_string();
        report.absorb(run_vector_file(&path, bundle, &label)?);
    }
    if cldr {
        report.absorb(run_cldr()?);
    }
    Ok(report)
}
