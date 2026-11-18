//! Models of kbdgen's fixture layouts, compiled for Linux as `target
//! linux` compiles them (`ldml.linux.input`).

use std::fs;
use std::path::Path;

use kbd_engine::Model;
use kbdgen::build::linux::source;
use kbdgen::bundle::read_kbdgen_bundle;

const WORKSPACE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");

pub fn fixture_text(relative: &str) -> String {
    let path = Path::new(WORKSPACE).join(relative);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// The v4 golden Võro layout: dead keys, a dead key without a standalone,
/// long-press candidates and a decimal separator.
pub fn vro() -> String {
    fixture_text("crates/kbd-engine/tests/golden/layouts/vro.yaml")
}

/// The extra modifiers, AltGr through `ctrl alt`, `other` and a native
/// `ctrl` layer, each differing only at `C01`.
pub fn mods() -> String {
    fixture_text("crates/kbd-engine/tests/golden/layouts/und-x-mods.yaml")
}

/// The Windows feature fixture as a `linux` variant: chained dead keys,
/// ligatures, Right Ctrl as an extra modifier, Shift Lock and LRM/RLM.
pub fn features() -> String {
    fixture_text("src/build/windows/kbdl/testdata/features.yaml")
        .replace("hardware:\n  windows:", "hardware:\n  linux:")
}

pub fn model(tag: &str, text: &str) -> Model {
    let dir = tempfile::tempdir().unwrap();
    let bundle = dir.path().join("test.kbdgen");
    for sub in ["layouts", "targets", "resources"] {
        fs::create_dir_all(bundle.join(sub)).unwrap();
    }
    fs::write(
        bundle.join("project.yaml"),
        "locales:\n  en:\n    name: Test\n    description: Test\nauthor: Test\ncopyright: Test\nemail: test@example.com\norganisation: Test\n",
    )
    .unwrap();
    fs::write(bundle.join(format!("layouts/{tag}.yaml")), text).unwrap();
    fs::write(
        bundle.join("targets/linux.yaml"),
        "packageId: no.divvun\nlayoutPrefix: divvun\nversion: 1.0\n",
    )
    .unwrap();
    let bundle = read_kbdgen_bundle(&bundle).unwrap();
    let layouts = source::layouts(&bundle).unwrap();
    let [layout] = layouts.as_slice() else {
        panic!("one Linux layout, got {}", layouts.len());
    };
    Model::from_bytes(&layout.model).unwrap()
}
