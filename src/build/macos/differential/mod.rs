//! `ldml.macos.test.typing` and `ldml.macos.test.differential`: the
//! written `.keylayout`, read back and simulated, against `kbd-engine` on
//! the same model; and a v3 layout's `.keylayout` against that of its
//! migration.

mod compare;
mod exceptions;
mod migrated;
mod parse;
mod simulate;
mod tests;

use std::path::{Path, PathBuf};

use kbd_engine::{BackspacePolicy, Model, Options, OutputForm};
use kbd_model::{Host, Keyboard};

use super::adapter::{self, Adapted};
use super::input::KeylayoutInput;
use super::writer;
use crate::bundle::{KbdgenBundle, fixture::write_bundle, read_kbdgen_bundle};
use crate::ldml::migrate::migrate_bundle_layout;

pub use parse::parse;
pub use simulate::{Keylayout, Mods};

pub const MACOS_TARGET: &str =
    "codeSignId: X\npackageId: com.example\nbundleName: Test\nversion: 1.0.0\nbuild: \"1\"\n";

/// A bundle on disk holding one layout and a macOS target.
pub struct FixtureBundle {
    _dir: tempfile::TempDir,
    pub bundle: KbdgenBundle,
}

/// A bundle holding `yaml` as layout `tag`.
pub fn fixture_bundle(tag: &str, yaml: &str) -> FixtureBundle {
    let dir = tempfile::tempdir().unwrap();
    let path = write_bundle(
        dir.path(),
        "test",
        &[(tag, yaml)],
        &[("macos", MACOS_TARGET)],
        &["macos"],
    );
    let bundle = read_kbdgen_bundle(&path).unwrap_or_else(|e| panic!("{tag}: {e}"));
    FixtureBundle { _dir: dir, bundle }
}

/// A v4 layout as its `.keylayout` ships: the adapter's output, the
/// written document read back, and the engine on the `macOS` keyboard.
pub struct Subject {
    pub label: String,
    pub adapted: Adapted,
    pub xml: String,
    pub parsed: KeylayoutInput,
    pub keyboard: Keyboard,
    pub model: Model,
}

/// The repository root, where fixture paths start.
pub const WORKSPACE: &str = env!("CARGO_MANIFEST_DIR");

/// The engine as a macOS host runs it.
pub fn macos_model(keyboard: &Keyboard) -> Model {
    let options = Options {
        output_form: OutputForm::Nfc,
        backspace: BackspacePolicy::CancelOrPass,
        host: Some(Host::MacOs),
    };
    Model::from_keyboard(keyboard.clone(), options).unwrap()
}

/// The subject of the v4 layout `yaml`, tagged `tag`.
pub fn subject(label: &str, tag: &str, yaml: &str) -> Subject {
    let fixture = fixture_bundle(tag, yaml);
    let (tag, path) = &fixture.bundle.v4_layouts[0];
    let layout = adapter::load(tag, path)
        .unwrap_or_else(|e| panic!("{label}: {e:#}"))
        .unwrap_or_else(|| panic!("{label}: no macOS document"));
    let adapted = adapter::adapt(&layout).unwrap_or_else(|e| panic!("{label}: {e:#}"));
    let xml = writer::write(&adapted.input);
    let parsed = parse(&xml, tag.as_str()).unwrap_or_else(|e| panic!("{label}: {e:#}"));
    let keyboard = layout.layout.keyboard_for(Host::MacOs).unwrap().clone();
    Subject {
        label: label.to_owned(),
        model: macos_model(&keyboard),
        adapted,
        xml,
        parsed,
        keyboard,
    }
}

/// The v4 text the migrator writes for the v3 layout `yaml`, which must
/// not block.
pub fn migrated_yaml(tag: &str, yaml: &str) -> String {
    let fixture = fixture_bundle(tag, yaml);
    let migration = migrate_bundle_layout(&fixture.bundle.path, tag)
        .unwrap()
        .unwrap_or_else(|| panic!("{tag}: no v3 layout"));
    assert!(
        !migration.blocked(),
        "{tag}: {:?}",
        migration.blocking_codes()
    );
    migration.yaml.unwrap()
}

/// Every fixture layout with a `macOS` document: the golden v4 layouts,
/// and the golden v3 layouts migrated in memory, as (label, tag, v4 text).
pub fn fixture_layouts() -> Vec<(String, String, String)> {
    let golden = Path::new(WORKSPACE).join("crates/kbd-engine/tests/golden");
    let mut layouts = Vec::new();
    for dir in ["layouts", "v3"] {
        let mut paths: Vec<PathBuf> = std::fs::read_dir(golden.join(dir))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.extension().is_some_and(|e| e == "yaml"))
            .collect();
        paths.sort();
        for path in paths {
            let tag = path.file_stem().unwrap().to_string_lossy().into_owned();
            let text = std::fs::read_to_string(&path).unwrap();
            let label = format!("{dir}/{tag}");
            let text = if dir == "v3" {
                migrated_yaml(&tag, &text)
            } else {
                text
            };
            layouts.push((label, tag, text));
        }
    }
    layouts
}
