//! The `kbdgen ldml` commands (`ldml.cli.*`): their argument shapes, and
//! the steps from a bundle's layouts to exported XML, imported layouts and
//! compiled models.

pub mod cli;
pub mod compile;
pub mod export;
pub mod import;
pub mod layouts;

use std::path::PathBuf;

/// Why a `kbdgen ldml` command failed. Each names the file, layout or host
/// concerned.
#[derive(Debug, thiserror::Error)]
pub enum LdmlError {
    #[error("{}: {source}", path.display())]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("{}: {message}", path.display())]
    Yaml { path: PathBuf, message: String },
    #[error("{tag} is not a well-formed BCP 47 tag")]
    InvalidTag { tag: String },
    #[error("{}: format {value} is not a layout format; v4 layouts have `format: 4`", path.display())]
    Format { path: PathBuf, value: String },
    #[error("no layout is tagged {0}")]
    UnknownLayout(String),
    #[error("layout {tag} is a v3 layout; convert it with `kbdgen ldml migrate`")]
    V3Layout { tag: String },
    #[error(
        "layout {tag} is a v4 layout, and this build cannot lower v4 layouts to host documents"
    )]
    LoweringUnavailable { tag: String },
    #[error("this build cannot write v4 layouts; {count} layout(s) were read and checked")]
    LayoutWriterUnavailable { count: usize },
    #[error(transparent)]
    Ldml(#[from] kbd_ldml::Error),
    #[error("layout {tag}: the {host} keyboard does not load in the engine: {source:?}")]
    Engine {
        tag: String,
        host: &'static str,
        source: kbd_engine::Error,
    },
    #[error("layout {tag}: {message}")]
    Layout { tag: String, message: String },
    #[error("{} and {} both claim layout {tag} for host {host}", first.display(), second.display())]
    DuplicateHost {
        tag: String,
        host: &'static str,
        first: PathBuf,
        second: PathBuf,
    },
    #[error("{} exists; pass --force to replace it", path.display())]
    Exists { path: PathBuf },
    #[error(transparent)]
    Compile(#[from] compile::CompileError),
}

#[cfg(test)]
mod tests;
