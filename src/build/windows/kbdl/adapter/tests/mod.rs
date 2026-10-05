//! Adapter tests: the Võro layout in v4 against its v3 counterpart, and
//! each rule of `ldml.kbdl.*` on small layouts.

use super::*;
use crate::build::windows::kbdl::{
    input::{KeyValue, Layer, POSITION_NAMES},
    tables::{self, Tables},
};
use crate::bundle::{fixture::write_bundle, read_kbdgen_bundle};

mod features;
mod model;
mod vro;

/// The Võro example of `docs/spec/ldml/yaml.md` in v4.
const VRO4: &str =
    include_str!("../../../../../../crates/kbd-engine/tests/golden/layouts/vro.yaml");

/// The Windows parts of the v3 Võro layout.
const VRO3: &str = include_str!("../../testdata/vro-v3.yaml");

const WINDOWS_TARGET: &str = "appName: Test\nversion: 1.0.0\nurl: http://example.com\nuuid: 00000000-0000-0000-0000-000000000000\nbuild: 2\n";

/// A bundle on disk with the given layouts and a Windows target.
struct Fixture {
    _dir: tempfile::TempDir,
    bundle: KbdgenBundle,
}

fn fixture(layouts: &[(&str, &str)]) -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let path = write_bundle(
        dir.path(),
        "test",
        layouts,
        &[("windows", WINDOWS_TARGET)],
        &[],
    );
    let bundle = read_kbdgen_bundle(&path).unwrap();
    Fixture { _dir: dir, bundle }
}

impl Fixture {
    /// The compiled v4 layout tagged `tag`.
    fn model_layout(&self, tag: &str) -> Option<ModelLayout> {
        let (tag, path) = self
            .bundle
            .v4_layouts
            .iter()
            .find(|(t, _)| t.as_str() == tag)
            .unwrap();
        load(tag, path).unwrap()
    }

    fn adapt(&self, tag: &str) -> Result<Adapted> {
        adapt(&self.bundle, &self.model_layout(tag).unwrap())
    }
}

fn adapted(tag: &str, yaml: &str) -> Adapted {
    fixture(&[(tag, yaml)])
        .adapt(tag)
        .unwrap_or_else(|e| panic!("{e:#}"))
}

fn adapt_error(tag: &str, yaml: &str) -> String {
    format!("{:#}", fixture(&[(tag, yaml)]).adapt(tag).unwrap_err())
}

/// The tables of an input, with the warnings they raise.
fn tables_of(input: &LayoutInput) -> (Tables, Diagnostics) {
    let mut diag = Diagnostics::new(input.metadata.name.clone());
    let tables = tables::build(input, &mut diag).unwrap_or_else(|e| panic!("{e:#}"));
    (tables, diag)
}

fn position(name: &str) -> usize {
    POSITION_NAMES.iter().position(|p| *p == name).unwrap()
}

fn value(input: &LayoutInput, layer: Layer, name: &str) -> Option<KeyValue> {
    input
        .layers
        .get(&layer)
        .and_then(|values| values[position(name)].clone())
}

/// A `sme` v4 layout: `format`, the autonym, then `body`.
fn sme(body: &str) -> String {
    format!("format: 4\ndisplayNames: {{sme: Davvisámegiella}}\n{body}")
}

fn iso_rows(first: &str) -> String {
    format!(
        "{first} 1 2 3 4 5 6 7 8 9 0 - =\n        q w e r t y u i o p å ¨\n        a s d f g h j k l ö æ '\n        < z x c v b n m , . /\n"
    )
}
