//! `kbdgen ldml migrate` (`ldml.migrate.*`): v3 layouts to v4.
//!
//! A v3 file is read with its raw strings (`source`), converted to a v4
//! layout and a trace of what each v3 key did (`convert`), and written as
//! fresh text (`out`). That text is then loaded as `kbdgen` loads any v4
//! layout, and run through the engine (`check`) against what the v3
//! platforms typed (`oracle`). Every defect found on the way is reported
//! (`defect`, `report`); a blocking one leaves the file unwritten.
//!
//! [`migrate_text`] works in memory, which the Windows build uses to embed
//! a v3 layout's model (`ldml.kbdl.model-resource`).

mod check;
mod convert;
pub mod defect;
pub mod oracle;
mod out;
pub mod report;
mod source;
mod text;

use std::path::{Path, PathBuf};

use kbd_model::{Host, Layout};
use serde_yaml::Value;

pub use self::check::CapsDiff;
pub use self::convert::Out;
use self::defect::{Code, Defect, Defects};
use self::report::{LayoutReport, Outcome, Report};
use super::LdmlError;
use super::layouts::{HostDocument, LayoutFile, compiled_layouts, layout_files};
use super::yaml::{Layout4, LayoutFormat, lower};

/// One v3 layout migrated in memory.
#[derive(Debug, Clone)]
pub struct Migration {
    pub path: PathBuf,
    pub tag: String,
    /// The v4 text; none for a v2 layout.
    pub yaml: Option<String>,
    /// The v4 layout loaded from that text, when it loads.
    pub layout: Option<Layout4>,
    pub defects: Vec<Defect>,
    /// The caps differences of `ldml.migrate.caps-diff`, also reported as
    /// M09.
    pub caps_diffs: Vec<CapsDiff>,
}

impl Migration {
    /// Whether a blocking defect leaves the layout unwritten.
    pub fn blocked(&self) -> bool {
        self.defects.iter().any(|d| d.code.blocks())
    }

    /// The distinct codes of the blocking defects, in code order.
    pub fn blocking_codes(&self) -> Vec<Code> {
        let mut codes: Vec<Code> = self
            .defects
            .iter()
            .map(|d| d.code)
            .filter(|c| c.blocks())
            .collect();
        codes.sort();
        codes.dedup();
        codes
    }

    /// The compiled layout of the migrated text, one keyboard per host
    /// (`ldml.model.layout`); none when the migration blocks.
    pub fn compiled(&self) -> Result<Option<Layout>, LdmlError> {
        let Some(layout) = self.layout.as_ref().filter(|_| !self.blocked()) else {
            return Ok(None);
        };
        let documents: Vec<HostDocument> = lower(layout)?
            .into_iter()
            .map(|(host, source)| HostDocument {
                tag: self.tag.clone(),
                host,
                source,
            })
            .collect();
        Ok(compiled_layouts(&documents)?.into_iter().next())
    }
}

/// Whether a load warning is one a reported defect accounts for: an
/// unreferenced dead key kept by M03.
fn explained_warning(warning: &str, defects: &Defects) -> bool {
    warning.contains("no \\d{") && defects.has(Code::M03)
}

// [spec:kbdgen:req:ldml.migrate.defects+1]
// [spec:kbdgen:req:ldml.migrate.equivalence+1]
/// Migrates the v3 layout `text`, read from `path` and tagged `tag`. A
/// file that is not v3 at all, such as one with a non-string value where
/// v3 needs a string, is an error; everything else is a defect.
pub fn migrate_text(path: &Path, tag: &str, text: &str) -> Result<Migration, LdmlError> {
    let yaml_error = |message: String| LdmlError::Yaml {
        path: path.to_path_buf(),
        message,
    };
    let value: Value = serde_yaml::from_str(text).map_err(|e| yaml_error(e.to_string()))?;
    let file = path.display().to_string();
    let mut defects = Defects::new(&file);
    let comments = source::comment_lines(text);
    if !comments.is_empty() {
        let lines: Vec<String> = comments.iter().map(usize::to_string).collect();
        defects.add(
            Code::M11,
            "",
            format!(
                "{} comment line(s) are dropped: lines {}",
                lines.len(),
                lines.join(", ")
            ),
        );
    }
    let mut migration = Migration {
        path: path.to_path_buf(),
        tag: tag.to_string(),
        yaml: None,
        layout: None,
        defects: Vec::new(),
        caps_diffs: Vec::new(),
    };
    let Some(src) = source::read(&value, &mut defects).map_err(yaml_error)? else {
        migration.defects = defects.into_list();
        return Ok(migration);
    };
    let converted = convert::convert(&src, &mut defects);
    let yaml = out::write(&converted.out);
    match check::load(&yaml, path, tag) {
        Ok((loaded, warnings)) => {
            for warning in warnings {
                if !explained_warning(&warning, &defects) {
                    defects.add(
                        Code::M99,
                        "",
                        format!("the migrated layout warns: {warning}"),
                    );
                }
            }
            let equivalence = !defects.blocked();
            migration.caps_diffs = check::run(&loaded, &converted.trace, equivalence, &mut defects);
            migration.layout = Some(loaded.layout);
        }
        Err(message) => defects.add(
            Code::M99,
            "",
            format!("the migrated layout does not load: {message}"),
        ),
    }
    migration.yaml = Some(yaml);
    migration.defects = defects.into_list();
    Ok(migration)
}

pub fn migrate_file(path: &Path, tag: &str) -> Result<Migration, LdmlError> {
    let text = std::fs::read_to_string(path).map_err(|source| LdmlError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    migrate_text(path, tag, &text)
}

/// The v3 layout of `bundle` tagged `tag`, migrated in memory; none when
/// the bundle has no v3 layout with that tag.
pub fn migrate_bundle_layout(bundle: &Path, tag: &str) -> Result<Option<Migration>, LdmlError> {
    let files = layout_files(bundle)?;
    match files
        .iter()
        .find(|f| f.tag == tag && f.format == LayoutFormat::V3)
    {
        Some(file) => migrate_file(&file.path, &file.tag).map(Some),
        None => Ok(None),
    }
}

/// The encoded `windows` keyboard (`ldml.model.encoding`) of a migration
/// that does not block; none when it has no `windows` keyboard.
// [spec:kbdgen:def:tsf.engine.model]
pub fn windows_model(migration: &Migration) -> Result<Option<Vec<u8>>, LdmlError> {
    let Some(layout) = migration.compiled()? else {
        return Ok(None);
    };
    let Some(keyboard) = layout.keyboard_for(Host::Windows) else {
        return Ok(None);
    };
    keyboard
        .to_bytes()
        .map(Some)
        .map_err(|e| LdmlError::Layout {
            tag: migration.tag.clone(),
            message: e.to_string(),
        })
}

// [spec:kbdgen:req:ldml.migrate.output+1]
// [spec:kbdgen:def:ldml.migrate.report+1]
/// Migrates every v3 layout of `bundle` in bundle layout order, then,
/// unless `dry_run`, writes each unblocked one in place under its v3 name.
/// Every layout is migrated before the first is written, so an error
/// writes nothing. v4 layouts are left alone.
pub fn migrate_bundle(bundle: &Path, dry_run: bool) -> Result<Report, LdmlError> {
    let files: Vec<LayoutFile> = layout_files(bundle)?
        .into_iter()
        .filter(|f| f.format == LayoutFormat::V3)
        .collect();
    let migrations = files
        .iter()
        .map(|f| migrate_file(&f.path, &f.tag))
        .collect::<Result<Vec<_>, _>>()?;
    let mut report = Report::default();
    for migration in migrations {
        let outcome = match &migration.yaml {
            _ if migration.blocked() => Outcome::Blocked,
            Some(_) if dry_run => Outcome::Checked,
            Some(yaml) => {
                std::fs::write(&migration.path, yaml).map_err(|source| LdmlError::Io {
                    path: migration.path.clone(),
                    source,
                })?;
                Outcome::Written
            }
            None => Outcome::Blocked,
        };
        report.layouts.push(LayoutReport {
            file: migration.path.display().to_string(),
            tag: migration.tag.clone(),
            outcome,
            defects: migration.defects,
        });
    }
    Ok(report)
}

#[cfg(test)]
mod tests;
