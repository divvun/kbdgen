//! `tsf.test.differential`: `kbd-engine` on the model a layout DLL embeds,
//! against what `ToUnicodeEx` returns for the DLL's tables.
//!
//! Every host runs the comparison against [`simulate::Dll`], which applies
//! the generated `KBDTABLES` (`aVkToBits`, `ModNumber`, the character
//! rows with their caps attributes, dead rows, `aDeadKey` and `aLigature`)
//! the way `ToUnicodeEx` does. On Windows, `windows.rs` loads each built
//! DLL and checks `ToUnicodeEx` itself against both the simulation and the
//! engine.
//!
//! The cases are every `kbdl.layers` layer at each of the 49 ISO positions
//! and the space bar from the reset state, then every dead-key path: each
//! case on which both sides agree and leave a dead key pending, continued
//! with every key that types or leaves something pending, up to
//! [`compare::MAX_DEPTH`] keys. Each side's committed text and pending dead
//! key must be equal, or the difference must be one of
//! [`exceptions::LISTED`].

mod compare;
mod exceptions;
mod simulate;
mod tests;
#[cfg(windows)]
mod windows;

use std::path::{Path, PathBuf};

use kbd_engine::Model;

use super::{
    GenerateKbdl, diag::Diagnostics, generate, input::LayoutInput, resources, source_layouts,
    tables::Tables,
};
use crate::bundle::{KbdgenBundle, fixture::write_bundle, read_kbdgen_bundle};
use crate::ldml::migrate::{Migration, migrate_bundle_layout};

const RT_RCDATA: u16 = 10;

const WINDOWS_TARGET: &str = "appName: Test\nversion: 1.0.0\nurl: http://example.com\nuuid: 00000000-0000-0000-0000-000000000000\nbuild: 2\n";

/// A bundle on disk holding one fixture layout and a Windows target.
pub(super) struct FixtureBundle {
    _dir: tempfile::TempDir,
    pub label: String,
    pub bundle: KbdgenBundle,
}

/// A layout as its DLL is built: the generator input, the DLL's tables and
/// the engine on the model its `.res` file embeds.
pub(super) struct Subject {
    pub name: String,
    /// The in-memory migration a v3 layout's model comes from.
    pub migration: Option<Migration>,
    pub input: LayoutInput,
    pub tables: Tables,
    pub model: Model,
}

fn workspace() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The `.yaml` files directly in `dir`, in file-name order.
fn yaml_files(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|e| e == "yaml"))
        .collect();
    files.sort();
    files
}

/// Every fixture layout kbdgen keeps: the golden v4 and v3 layouts of
/// `kbd-engine` and the v3 Võro layout of the `kbdl` tests, each with the
/// language tag its bundle gives it.
pub(super) fn fixture_layouts() -> Vec<(String, PathBuf)> {
    let golden = workspace().join("crates/kbd-engine/tests/golden");
    let mut layouts = Vec::new();
    for dir in ["layouts", "v3"] {
        for path in yaml_files(&golden.join(dir)) {
            let tag = path.file_stem().unwrap().to_string_lossy().into_owned();
            layouts.push((tag, path));
        }
    }
    layouts.push((
        "vro".to_owned(),
        workspace().join("src/build/windows/kbdl/testdata/vro-v3.yaml"),
    ));
    layouts
}

/// A bundle holding one layout, under `tag`.
pub(super) fn fixture_bundle(label: &str, tag: &str, yaml: &str) -> FixtureBundle {
    let dir = tempfile::tempdir().unwrap();
    let path = write_bundle(
        dir.path(),
        "test",
        &[(tag, yaml)],
        &[("windows", WINDOWS_TARGET)],
        &[],
    );
    let bundle = read_kbdgen_bundle(&path).unwrap_or_else(|e| panic!("{label}: {e}"));
    FixtureBundle {
        _dir: dir,
        label: label.to_owned(),
        bundle,
    }
}

/// One bundle per fixture layout.
pub(super) fn fixture_bundles() -> Vec<FixtureBundle> {
    fixture_layouts()
        .into_iter()
        .map(|(tag, path)| {
            let yaml = std::fs::read_to_string(&path).unwrap();
            let label = path
                .strip_prefix(workspace())
                .unwrap_or(&path)
                .display()
                .to_string();
            fixture_bundle(&label, &tag, &yaml)
        })
        .collect()
}

pub(super) fn embedded_model(res: &[u8]) -> Option<Vec<u8>> {
    resources::tests::entries(res)
        .into_iter()
        .find(|(kind, name, ..)| *kind == RT_RCDATA && *name == resources::MODEL_RESOURCE_ID)
        .map(|(.., data)| data)
}

// [spec:kbdgen:req:tsf.test.engine]
/// Every layout of `fixture` as its DLL is built. Each must embed a model,
/// since a layout without one has no text service to compare.
pub(super) fn subjects(fixture: &FixtureBundle) -> Vec<Subject> {
    let label = &fixture.label;
    let layouts = source_layouts(&fixture.bundle).unwrap_or_else(|e| panic!("{label}: {e:#}"));
    layouts
        .into_iter()
        .map(|mut layout| {
            let name = layout.input.metadata.name.clone();
            let generated = generate(&layout.input, layout.model.as_deref(), &mut layout.diag)
                .unwrap_or_else(|e| panic!("{label}: {e:#}"));
            let mut diag = Diagnostics::new(name.clone());
            let tables = super::tables::build(&layout.input, &mut diag).unwrap();
            let model = embedded_model(&generated.res)
                .unwrap_or_else(|| panic!("{label}: {name} embeds no model"));
            let migration = fixture
                .bundle
                .layouts
                .get(&layout.tag)
                .is_some()
                .then(|| migrate_bundle_layout(&fixture.bundle.path, layout.tag.as_str()))
                .and_then(|migration| migration.unwrap_or_else(|e| panic!("{label}: {e}")));
            Subject {
                name,
                migration,
                input: layout.input,
                tables,
                model: Model::from_bytes(&model).unwrap_or_else(|e| panic!("{label}: {e}")),
            }
        })
        .collect()
}

/// Writes the crates of `fixture` under `out`, as the build step does.
pub(super) async fn write_crates(fixture: &FixtureBundle, out: &Path) {
    use crate::build::BuildStep;
    GenerateKbdl
        .build(&fixture.bundle, out)
        .await
        .unwrap_or_else(|e| panic!("{}: {e:#}", fixture.label));
}
