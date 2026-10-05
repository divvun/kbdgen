//! Conformance tests: the golden vectors and CLDR's, vector files, bundle
//! discovery, robustness of the engine on fixture models, and round trips
//! of the fixture keyboards.

use std::path::{Path, PathBuf};

use kbd_engine::Model;

use super::run::vector_model;
use super::vectors::{VectorFile, parse};

mod bundle;
mod cldr;
mod golden;
mod robust;
// [spec:kbdgen:req:ldml.test.roundtrip]
mod roundtrip;
mod vectors;

// [spec:kbdgen:req:ldml.test.golden]
/// kbdgen's golden vectors, laid out as a bundle: `tests/*.yaml` run on
/// `layouts/*.yaml` and `keyboards/*.xml`, so `kbdgen ldml test -b` runs
/// them too.
fn golden() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("crates/kbd-engine/tests/golden")
}

fn vector_file(yaml: &str) -> Result<VectorFile, String> {
    let value: serde_yaml::Value = serde_yaml::from_str(yaml).map_err(|e| e.to_string())?;
    parse("t.yaml", &value).map_err(|e| e.to_string())
}

fn golden_model(yaml: &str) -> (VectorFile, Model) {
    let file = vector_file(yaml).unwrap_or_else(|e| panic!("{e}"));
    let model = vector_model(&file, &golden()).unwrap_or_else(|e| panic!("{e}"));
    (file, model)
}

fn golden_files(dir: &str, ext: &str) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(golden().join(dir))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == ext))
        .collect();
    paths.sort();
    paths
}
