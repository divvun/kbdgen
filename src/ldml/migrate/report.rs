//! The migration report (`ldml.migrate.report`): one entry per defect,
//! and a summary counting defects per code and listing blocked layouts.

use std::collections::BTreeMap;

use serde::Serialize;

use super::defect::{Code, Defect};

/// What became of one layout file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// Written in place as v4.
    Written,
    /// Migrated, and not written because of `--dry-run`.
    Checked,
    /// Not written because of a blocking defect.
    Blocked,
}

impl Outcome {
    pub fn name(self) -> &'static str {
        match self {
            Outcome::Written => "written",
            Outcome::Checked => "migrated (dry run)",
            Outcome::Blocked => "blocked",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutReport {
    pub file: String,
    pub tag: String,
    pub outcome: Outcome,
    pub defects: Vec<Defect>,
}

/// The report of one run over a bundle, layouts in bundle layout order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    pub layouts: Vec<LayoutReport>,
}

#[derive(Serialize)]
struct Entry<'a> {
    code: &'static str,
    file: &'a str,
    path: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    row: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    token: Option<&'a str>,
    message: &'a str,
    action: &'static str,
}

#[derive(Serialize)]
struct LayoutEntry<'a> {
    file: &'a str,
    outcome: &'static str,
}

#[derive(Serialize)]
struct Summary<'a> {
    counts: BTreeMap<&'static str, usize>,
    blocked: Vec<String>,
    layouts: Vec<LayoutEntry<'a>>,
}

#[derive(Serialize)]
struct Document<'a> {
    defects: Vec<Entry<'a>>,
    summary: Summary<'a>,
}

impl Report {
    pub fn blocked(&self) -> Vec<&LayoutReport> {
        self.layouts
            .iter()
            .filter(|l| l.outcome == Outcome::Blocked)
            .collect()
    }

    fn defects(&self) -> impl Iterator<Item = &Defect> {
        self.layouts.iter().flat_map(|l| l.defects.iter())
    }

    /// Defects per code, in code order, codes with none left out.
    pub fn counts(&self) -> Vec<(Code, usize)> {
        Code::ALL
            .iter()
            .map(|c| (*c, self.defects().filter(|d| d.code == *c).count()))
            .filter(|(_, n)| *n > 0)
            .collect()
    }

    /// The summary lines: counts per code, each layout's outcome, and the
    /// blocked layouts.
    pub fn summary(&self) -> Vec<String> {
        let counts: Vec<String> = self
            .counts()
            .iter()
            .map(|(c, n)| format!("{c} {n}"))
            .collect();
        let mut lines = vec![format!(
            "summary: {} layout(s), {} defect(s){}{}",
            self.layouts.len(),
            self.defects().count(),
            if counts.is_empty() { "" } else { ": " },
            counts.join(", ")
        )];
        for layout in &self.layouts {
            lines.push(format!("{}: {}", layout.file, layout.outcome.name()));
        }
        let blocked: Vec<&str> = self.blocked().iter().map(|l| l.file.as_str()).collect();
        if !blocked.is_empty() {
            lines.push(format!("blocked: {}", blocked.join(", ")));
        }
        lines
    }

    /// The report as text: every defect, then the summary.
    pub fn text(&self) -> String {
        let mut out = String::new();
        for defect in self.defects() {
            out.push_str(&defect.to_string());
            out.push('\n');
        }
        for line in self.summary() {
            out.push_str(&line);
            out.push('\n');
        }
        out
    }

    /// The report as YAML: `defects`, then `summary`.
    pub fn yaml(&self) -> String {
        let document = Document {
            defects: self
                .defects()
                .map(|d| Entry {
                    code: d.code.name(),
                    file: &d.file,
                    path: &d.path,
                    row: d.row,
                    token: d.token.as_deref(),
                    message: &d.message,
                    action: d.action().name(),
                })
                .collect(),
            summary: Summary {
                counts: self
                    .counts()
                    .into_iter()
                    .map(|(c, n)| (c.name(), n))
                    .collect(),
                blocked: self.blocked().iter().map(|l| l.file.clone()).collect(),
                layouts: self
                    .layouts
                    .iter()
                    .map(|l| LayoutEntry {
                        file: &l.file,
                        outcome: l.outcome.name(),
                    })
                    .collect(),
            },
        };
        serde_yaml::to_string(&document).unwrap_or_default()
    }
}
