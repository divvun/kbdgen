//! Conformance runs (`ldml.test.*`): kbdgen's golden vectors
//! (`ldml.test.vectors`), a bundle's `tests/*.yaml` (`ldml.test.bundle`)
//! and CLDR's keyboardTest3 vectors (`ldml.test.cldr`), all driven
//! through the engine's harness (`kbd_engine::harness`).
//!
//! A run never stops at a failing step: every failing step is reported
//! with what was expected and what the engine did. Files that cannot be
//! loaded are errors, which do stop the run.

pub mod bundle;
pub mod cldr;
mod cldr_data;
pub mod run;
pub mod steps;
pub mod vectors;

use std::fmt;

use kbd_engine::Action;
use kbd_engine::harness::Harness;

/// What the harness showed after a failing step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Observed {
    pub text: String,
    pub preedit: String,
    pub layer: Option<String>,
    pub action: Option<Action>,
}

impl Observed {
    pub fn of(harness: &Harness) -> Self {
        Observed {
            text: harness.document().to_string(),
            preedit: harness.preedit().to_string(),
            layer: harness.layer().map(str::to_string),
            action: harness.last_action().cloned(),
        }
    }
}

/// A failing step: where it is, the step as written, the expected values
/// it states, and what the harness showed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    pub file: String,
    pub test: String,
    /// 1-based.
    pub step: usize,
    pub written: String,
    /// `(field, value)` pairs, such as `("text", "\"á\"")`.
    pub expected: Vec<(String, String)>,
    pub observed: Observed,
}

fn action_text(action: Option<&Action>) -> String {
    match action {
        None => "none sent".to_string(),
        Some(Action::Pass) => "pass".to_string(),
        Some(Action::Edit {
            delete,
            insert,
            preedit,
            layer,
        }) => format!(
            "edit: delete {delete}, insert {insert:?}, preedit {preedit:?}, layer {}",
            layer
                .as_deref()
                .map_or("none".to_string(), |l| format!("{l:?}"))
        ),
    }
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "FAIL {}: {}: step {}: {}",
            self.file, self.test, self.step, self.written
        )?;
        for (field, value) in &self.expected {
            writeln!(f, "  expected {field:<8} {value}")?;
        }
        let o = &self.observed;
        writeln!(f, "  actual   text     {:?}", o.text)?;
        writeln!(f, "  actual   preedit  {:?}", o.preedit)?;
        writeln!(
            f,
            "  actual   layer    {}",
            o.layer
                .as_deref()
                .map_or("none".to_string(), |l| format!("{l:?}"))
        )?;
        write!(f, "  actual   action   {}", action_text(o.action.as_ref()))
    }
}

/// The result of a run: counts, failing steps, and informational notes
/// such as skipped repertoires.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    pub files: usize,
    pub tests: usize,
    /// Steps that check something: `expect` and `check`.
    pub checks: usize,
    pub failures: Vec<Failure>,
    pub notes: Vec<String>,
}

impl Report {
    pub fn absorb(&mut self, other: Report) {
        self.files += other.files;
        self.tests += other.tests;
        self.checks += other.checks;
        self.failures.extend(other.failures);
        self.notes.extend(other.notes);
    }

    /// The closing summary line.
    pub fn summary(&self) -> String {
        format!(
            "files: {}, tests: {}, checks: {}, failing steps: {}",
            self.files,
            self.tests,
            self.checks,
            self.failures.len()
        )
    }
}

#[cfg(test)]
mod tests;
