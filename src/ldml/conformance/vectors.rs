//! Golden vector files (`ldml.test.vectors`): which keyboard to load, for
//! which host and with which engine options, and the tests to run on it.

use std::path::PathBuf;

use kbd_engine::{BackspacePolicy, Options, OutputForm};
use kbd_model::Host;
use serde_yaml::Value;

use super::steps::{Step, steps};
use crate::ldml::yaml::node::{Fields, boolean, list, string};
use crate::ldml::yaml::{At, YamlError};

type Result<T> = std::result::Result<T, YamlError>;

/// The keyboard a vector file runs on, by a path relative to the bundle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// A v4 layout file, or a v3 one, migrated in memory.
    Layout(PathBuf),
    /// A keyboard3 XML file.
    Keyboard(PathBuf),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VectorTest {
    pub name: String,
    /// The document before the first step.
    pub context: String,
    /// Whether the document begins at a start of text.
    pub at_start: bool,
    pub steps: Vec<Step>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VectorFile {
    pub source: Source,
    /// The host document of a layout. For XML it is the keyboard's host,
    /// which must agree with the one the document names.
    pub host: Option<Host>,
    pub options: Options,
    pub tests: Vec<VectorTest>,
}

fn host(value: &Value, at: &At) -> Result<Host> {
    let name = string(value, at)?;
    Host::from_name(name).ok_or_else(|| {
        let names: Vec<&str> = Host::ALL.iter().map(|h| h.name()).collect();
        at.error(format!("{name} is not a host; one of {}", names.join(", ")))
    })
}

fn options(value: &Value, at: &At) -> Result<Options> {
    let mut f = Fields::new(value, at)?;
    let mut options = Options::default();
    if let Some((v, a)) = f.take("backspace") {
        options.backspace = match string(v, &a)? {
            "cancelOrPass" => BackspacePolicy::CancelOrPass,
            "codePoint" => BackspacePolicy::CodePoint,
            other => return Err(a.error(format!("{other} is not cancelOrPass or codePoint"))),
        };
    }
    if let Some((v, a)) = f.take("outputForm") {
        options.output_form = match string(v, &a)? {
            "nfc" => OutputForm::Nfc,
            "nfd" => OutputForm::Nfd,
            other => return Err(a.error(format!("{other} is not nfc or nfd"))),
        };
    }
    f.finish()?;
    Ok(options)
}

fn test(value: &Value, at: &At) -> Result<VectorTest> {
    let mut f = Fields::new(value, at)?;
    let (v, a) = f.require("name")?;
    let name = string(v, &a)?.to_string();
    let context = match f.take("context") {
        Some((v, a)) => {
            kbd_ldml::escape::decode_plain(string(v, &a)?).map_err(|e| a.error(e.to_string()))?
        }
        None => String::new(),
    };
    let at_start = match f.take("atStart") {
        Some((v, a)) => boolean(v, &a)?,
        None => true,
    };
    let (v, a) = f.require("steps")?;
    let steps = steps(v, &a)?;
    f.finish()?;
    Ok(VectorTest {
        name,
        context,
        at_start,
        steps,
    })
}

fn source(f: &mut Fields, at: &At) -> Result<(Source, At)> {
    match (f.take("layout"), f.take("keyboard")) {
        (Some((v, a)), None) => Ok((Source::Layout(PathBuf::from(string(v, &a)?)), a)),
        (None, Some((v, a))) => Ok((Source::Keyboard(PathBuf::from(string(v, &a)?)), a)),
        (Some(_), Some((_, a))) => Err(a.error("a vector file has layout or keyboard, not both")),
        (None, None) => Err(at.error("a vector file has layout or keyboard")),
    }
}

/// Parses a vector file's tree; `file` names it in errors.
pub fn parse(file: &str, value: &Value) -> Result<VectorFile> {
    let at = At::file(file);
    let mut f = Fields::new(value, &at)?;
    let (source, source_at) = source(&mut f, &at)?;
    let host = f.take("host").map(|(v, a)| host(v, &a)).transpose()?;
    if host.is_none() && matches!(source, Source::Layout(_)) {
        return Err(source_at.error("a layout needs host, which selects its host document"));
    }
    let options = match f.take("options") {
        Some((v, a)) => options(v, &a)?,
        None => Options::default(),
    };
    let (v, a) = f.require("tests")?;
    let tests = list(v, &a)?
        .into_iter()
        .map(|(v, a)| test(v, &a))
        .collect::<Result<Vec<_>>>()?;
    f.finish()?;
    Ok(VectorFile {
        source,
        host,
        options,
        tests,
    })
}
