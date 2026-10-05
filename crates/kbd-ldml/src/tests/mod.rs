//! Tests over whole documents: shared fixtures here, one module per topic.

use std::path::PathBuf;

use crate::{Resolved, SourceDocument, read_keyboard, resolve};

mod cldr;
mod export;
mod read;
mod resolution;

fn testdata(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("testdata")
        .join(rel)
}

fn cldr_keyboards() -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(testdata("cldr/48/keyboards/3.0"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "xml"))
        .collect();
    paths.sort();
    paths
}

fn source(xml: &str) -> SourceDocument {
    read_keyboard("test.xml", None, xml.as_bytes()).unwrap()
}

fn resolved(xml: &str) -> Resolved {
    resolve(&source(xml)).unwrap_or_else(|e| panic!("{e}"))
}

/// The error message of resolving `xml`.
fn resolve_error(xml: &str) -> String {
    match read_keyboard("test.xml", None, xml.as_bytes()) {
        Err(e) => e.to_string(),
        Ok(s) => resolve(&s).map(|_| ()).unwrap_err().to_string(),
    }
}

/// A minimal keyboard3 document around `body`.
fn keyboard(attrs: &str, body: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<keyboard3 locale="sme" conformsTo="45" {attrs}>
  <info name="Test"/>
{body}
</keyboard3>
"#
    )
}

/// The engine over `keyboard`, with default options; a keyboard the
/// engine refuses fails the test with the engine's reason.
fn engine(keyboard: kbd_model::Keyboard) -> kbd_engine::Model {
    match kbd_engine::Model::from_keyboard(keyboard, kbd_engine::Options::default()) {
        Ok(model) => model,
        Err(e) => panic!("the engine refuses the keyboard: {e:?}"),
    }
}
