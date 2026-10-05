//! Problems in v4 layout files, addressed by file, YAML path and, inside
//! rows, row and token (`ldml.yaml.strict`).

use std::fmt;
use std::sync::Arc;

/// An error or warning: the file, the YAML path such as
/// `hardware.macOS.layers.shift`, the 1-based row and token for problems
/// inside rows, and what is wrong. List items are written `[n]`, 1-based.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YamlProblem {
    pub file: String,
    pub path: String,
    pub row: Option<usize>,
    pub token: Option<usize>,
    pub message: String,
}

impl fmt::Display for YamlProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.file)?;
        if !self.path.is_empty() {
            write!(f, ": {}", self.path)?;
        }
        if let Some(row) = self.row {
            write!(f, ", row {row}")?;
        }
        if let Some(token) = self.token {
            write!(f, ", token {token}")?;
        }
        write!(f, ": {}", self.message)
    }
}

/// A v4 layout that cannot be loaded or lowered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YamlError(pub Box<YamlProblem>);

impl YamlError {
    pub fn problem(&self) -> &YamlProblem {
        &self.0
    }
}

impl fmt::Display for YamlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl std::error::Error for YamlError {}

pub type Result<T, E = YamlError> = std::result::Result<T, E>;

/// A place in a layout file that problems are reported at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct At {
    file: Arc<str>,
    path: String,
    row: Option<usize>,
    token: Option<usize>,
}

impl At {
    pub fn file(file: &str) -> At {
        At {
            file: Arc::from(file),
            path: String::new(),
            row: None,
            token: None,
        }
    }

    /// The value of field `key` here.
    pub fn key(&self, key: &str) -> At {
        let path = if self.path.is_empty() {
            key.to_string()
        } else {
            format!("{}.{key}", self.path)
        };
        At {
            path,
            ..self.clone()
        }
    }

    /// The `index`-th (0-based) item of the list here.
    pub fn index(&self, index: usize) -> At {
        At {
            path: format!("{}[{}]", self.path, index + 1),
            ..self.clone()
        }
    }

    /// Row `row` (0-based) of the rows here.
    pub fn row(&self, row: usize) -> At {
        At {
            row: Some(row + 1),
            ..self.clone()
        }
    }

    /// Token `token` (0-based) of the row here.
    pub fn token(&self, token: usize) -> At {
        At {
            token: Some(token + 1),
            ..self.clone()
        }
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn file_name(&self) -> &str {
        &self.file
    }

    pub fn warning(&self, message: impl Into<String>) -> YamlProblem {
        YamlProblem {
            file: self.file.to_string(),
            path: self.path.clone(),
            row: self.row,
            token: self.token,
            message: message.into(),
        }
    }

    pub fn error(&self, message: impl Into<String>) -> YamlError {
        YamlError(Box::new(self.warning(message)))
    }
}
