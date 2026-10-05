//! The defects the migrator reports (`ldml.migrate.defects`): their codes,
//! what each one does to the layout, and where it was found.

use std::fmt;

/// A defect code of `ldml.migrate.defects`; `M99` is an unexplained
/// difference found by `ldml.migrate.equivalence`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Code {
    M01,
    M02,
    M03,
    M04,
    M05,
    M06,
    M07,
    M08,
    M09,
    M10,
    M11,
    M12,
    M13,
    M14,
    M15,
    M16,
    M17,
    M99,
}

impl Code {
    pub const ALL: [Code; 18] = [
        Code::M01,
        Code::M02,
        Code::M03,
        Code::M04,
        Code::M05,
        Code::M06,
        Code::M07,
        Code::M08,
        Code::M09,
        Code::M10,
        Code::M11,
        Code::M12,
        Code::M13,
        Code::M14,
        Code::M15,
        Code::M16,
        Code::M17,
        Code::M99,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Code::M01 => "M01",
            Code::M02 => "M02",
            Code::M03 => "M03",
            Code::M04 => "M04",
            Code::M05 => "M05",
            Code::M06 => "M06",
            Code::M07 => "M07",
            Code::M08 => "M08",
            Code::M09 => "M09",
            Code::M10 => "M10",
            Code::M11 => "M11",
            Code::M12 => "M12",
            Code::M13 => "M13",
            Code::M14 => "M14",
            Code::M15 => "M15",
            Code::M16 => "M16",
            Code::M17 => "M17",
            Code::M99 => "M99",
        }
    }

    /// What the migrator does about a defect with this code.
    pub fn action(self) -> Action {
        match self {
            Code::M01 | Code::M02 | Code::M15 => Action::Dropped,
            Code::M03 => Action::Kept,
            Code::M04 | Code::M05 | Code::M07 | Code::M99 => Action::Blocked,
            Code::M06 => Action::Rewritten,
            Code::M08 | Code::M12 | Code::M16 => Action::Info,
            Code::M09 | Code::M11 | Code::M13 | Code::M14 | Code::M17 => Action::Warned,
            Code::M10 => Action::Dropped,
        }
    }

    pub fn blocks(self) -> bool {
        self.action() == Action::Blocked
    }
}

impl fmt::Display for Code {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// The action taken on a defect, as the report names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// The construct is left out of the v4 layout.
    Dropped,
    /// The construct is kept as written.
    Kept,
    /// The v4 layout says it differently.
    Rewritten,
    /// Nothing changes; the report says what to review.
    Warned,
    /// For information only.
    Info,
    /// The layout is not written.
    Blocked,
}

impl Action {
    pub fn name(self) -> &'static str {
        match self {
            Action::Dropped => "dropped",
            Action::Kept => "kept",
            Action::Rewritten => "rewritten",
            Action::Warned => "warned",
            Action::Info => "info",
            Action::Blocked => "blocked",
        }
    }
}

/// One reported defect (`ldml.migrate.report`): the code, the layout file,
/// the YAML path in the v3 file, the 1-based row and the token where known,
/// and what is wrong.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Defect {
    pub code: Code,
    pub file: String,
    pub path: String,
    pub row: Option<usize>,
    pub token: Option<String>,
    pub message: String,
}

impl Defect {
    pub fn action(&self) -> Action {
        self.code.action()
    }
}

impl fmt::Display for Defect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.code, self.file)?;
        if !self.path.is_empty() {
            write!(f, ": {}", self.path)?;
        }
        if let Some(row) = self.row {
            write!(f, ", row {row}")?;
        }
        if let Some(token) = &self.token {
            write!(f, ", token {token}")?;
        }
        write!(f, ": {} ({})", self.message, self.action().name())
    }
}

/// The defects of one layout file as they are found, each addressed by
/// the file.
#[derive(Debug, Clone, Default)]
pub struct Defects {
    file: String,
    list: Vec<Defect>,
}

impl Defects {
    pub fn new(file: &str) -> Defects {
        Defects {
            file: file.to_string(),
            list: Vec::new(),
        }
    }

    pub fn file(&self) -> &str {
        &self.file
    }

    pub fn add(&mut self, code: Code, path: &str, message: impl Into<String>) {
        self.push(code, path, None, None, message);
    }

    pub fn add_at(
        &mut self,
        code: Code,
        path: &str,
        row: Option<usize>,
        token: Option<&str>,
        message: impl Into<String>,
    ) {
        self.push(code, path, row, token.map(str::to_string), message);
    }

    fn push(
        &mut self,
        code: Code,
        path: &str,
        row: Option<usize>,
        token: Option<String>,
        message: impl Into<String>,
    ) {
        let defect = Defect {
            code,
            file: self.file.clone(),
            path: path.to_string(),
            row,
            token,
            message: message.into(),
        };
        if !self.list.contains(&defect) {
            self.list.push(defect);
        }
    }

    pub fn has(&self, code: Code) -> bool {
        self.list.iter().any(|d| d.code == code)
    }

    /// Whether a defect with `code` was reported at `path` or below it.
    pub fn has_under(&self, code: Code, path: &str) -> bool {
        self.list.iter().any(|d| {
            d.code == code
                && (d.path == path
                    || d.path
                        .strip_prefix(path)
                        .is_some_and(|rest| rest.starts_with(['.', '['])))
        })
    }

    pub fn blocked(&self) -> bool {
        self.list.iter().any(|d| d.code.blocks())
    }

    pub fn list(&self) -> &[Defect] {
        &self.list
    }

    pub fn into_list(self) -> Vec<Defect> {
        self.list
    }
}
