//! Errors and warnings, addressed by file, element path and attribute.

use std::fmt;

/// Where a problem is and what it is. `path` is an element path such as
/// `keyboard3/keys/key[id=e-acute]`, empty for problems with the file as a
/// whole; `position` is a 1-based (line, column) where the raw input gives
/// one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub file: String,
    pub path: String,
    pub attribute: Option<String>,
    pub position: Option<(usize, usize)>,
    pub message: String,
}

impl Diagnostic {
    pub fn new(file: &str, path: &str, message: impl Into<String>) -> Self {
        Diagnostic {
            file: file.to_string(),
            path: path.to_string(),
            attribute: None,
            position: None,
            message: message.into(),
        }
    }

    pub fn at(mut self, attribute: &str) -> Self {
        self.attribute = Some(attribute.to_string());
        self
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.file)?;
        if let Some((line, column)) = self.position {
            write!(f, ":{line}:{column}")?;
        }
        if !self.path.is_empty() {
            write!(f, ": {}", self.path)?;
        }
        if let Some(attribute) = &self.attribute {
            write!(f, "@{attribute}")?;
        }
        write!(f, ": {}", self.message)
    }
}

/// A failure to read, resolve or write a document. It always names the
/// file, and the element path and attribute where there is one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error(pub Box<Diagnostic>);

impl Error {
    pub fn diagnostic(&self) -> &Diagnostic {
        &self.0
    }
}

impl From<Diagnostic> for Error {
    fn from(d: Diagnostic) -> Self {
        Error(Box::new(d))
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl std::error::Error for Error {}

pub type Result<T, E = Error> = std::result::Result<T, E>;
