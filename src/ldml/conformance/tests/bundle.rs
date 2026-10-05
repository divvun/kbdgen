//! A bundle's `tests/*.yaml` (`ldml.test.bundle`).

use crate::bundle::{fixture, read_kbdgen_bundle};

use super::super::bundle::{bundle_tests, run_bundle};
use super::*;

const VRO_TEST: &str = "layout: layouts/vro.yaml\nhost: macOS\ntests:\n  - name: acute\n    steps:\n      - press: {key: E12}\n      - press: {key: C01}\n      - expect: {text: á}\n";

fn vro_bundle(root: &Path) -> PathBuf {
    let vro = std::fs::read_to_string(golden().join("layouts/vro.yaml")).unwrap();
    let bundle = fixture::write_bundle(root, "vro", &[("vro", &vro)], &[], &[]);
    fixture::write_file(&bundle, "tests/b.yaml", VRO_TEST);
    fixture::write_file(&bundle, "tests/a.yaml", VRO_TEST);
    fixture::write_file(&bundle, "tests/c.yml", "not: read");
    fixture::write_file(&bundle, "tests/nested/d.yaml", "not: read");
    bundle
}

// [spec:kbdgen:req:ldml.test.bundle/test]
#[test]
fn bundle_tests_are_sorted_yaml_files() {
    let root = tempfile::tempdir().unwrap();
    let bundle = vro_bundle(root.path());
    let names: Vec<String> = bundle_tests(&bundle)
        .unwrap()
        .iter()
        .map(|p| p.strip_prefix(&bundle).unwrap().display().to_string())
        .collect();
    assert_eq!(names, ["tests/a.yaml", "tests/b.yaml"]);
    let report = run_bundle(&bundle, false).unwrap();
    assert_eq!((report.files, report.tests, report.checks), (2, 2, 2));
    assert!(report.failures.is_empty(), "{:?}", report.failures);
}

// [spec:kbdgen:req:ldml.test.bundle/test]
#[test]
fn bundle_loading_ignores_the_tests_directory() {
    let root = tempfile::tempdir().unwrap();
    let bundle = vro_bundle(root.path());
    fixture::write_file(&bundle, "tests/broken.yaml", ": [ not yaml");
    let loaded = read_kbdgen_bundle(&bundle).unwrap();
    assert_eq!(loaded.v4_layouts.len(), 1);
    let err = run_bundle(&bundle, false).unwrap_err().to_string();
    assert!(err.contains("broken.yaml"), "{err}");
}

// [spec:kbdgen:req:ldml.test.bundle/test]
#[test]
fn bundles_without_tests_run_nothing() {
    let root = tempfile::tempdir().unwrap();
    let bundle = fixture::write_bundle(root.path(), "empty", &[], &[], &[]);
    assert!(bundle_tests(&bundle).unwrap().is_empty());
    assert_eq!(run_bundle(&bundle, false).unwrap().files, 0);
    let err = bundle_tests(&root.path().join("missing.kbdgen")).unwrap_err();
    assert!(err.to_string().contains("missing.kbdgen"), "{err}");
}

// [spec:kbdgen:req:ldml.cli.test/test]
#[test]
fn cldr_flag_adds_the_cldr_vectors() {
    let root = tempfile::tempdir().unwrap();
    let bundle = vro_bundle(root.path());
    let report = run_bundle(&bundle, true).unwrap();
    assert_eq!((report.files, report.tests), (7, 12));
    assert!(report.failures.is_empty(), "{:?}", report.failures);
}
