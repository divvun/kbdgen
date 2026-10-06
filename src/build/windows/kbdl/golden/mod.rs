//! Golden files of the layout generator: each fixture's crate (`Cargo.toml`,
//! `lib.rs` and the `.res` file) and diagnostics, and the `KBDTABLES` its
//! built DLLs hold, under `testdata/golden/<fixture>/`. `KBDGEN_BLESS=1`
//! rewrites them.

mod dll;
mod kbdtables;

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use super::{
    GeneratedLayout, diag::Diagnostics, differential::fixture_bundle, generate, resources,
    source_layouts,
};

/// The repository root, where fixture paths start.
const WORKSPACE: &str = env!("CARGO_MANIFEST_DIR");

/// The golden directory, relative to [`WORKSPACE`].
const GOLDEN_DIR: &str = "src/build/windows/kbdl/testdata/golden";

/// Lines of a mismatching region shown on each side of a text diff.
const DIFF_LINES: usize = 40;

/// A layout the golden files are generated from: its golden directory,
/// its language tag and its file, relative to [`WORKSPACE`].
struct Fixture {
    dir: &'static str,
    tag: &'static str,
    path: &'static str,
}

/// The v4 Võro golden layout, the two v3 layouts the migration tests build
/// DLLs from (tables from `kbdl.input.bundle`, model by migration), and the
/// Windows feature layout.
const FIXTURES: [Fixture; 4] = [
    Fixture {
        dir: "vro",
        tag: "vro",
        path: "crates/kbd-engine/tests/golden/layouts/vro.yaml",
    },
    Fixture {
        dir: "vro-v3",
        tag: "vro",
        path: "crates/kbd-engine/tests/golden/v3/vro.yaml",
    },
    Fixture {
        dir: "se-NO-v3",
        tag: "se-NO",
        path: "crates/kbd-engine/tests/golden/v3/se-NO.yaml",
    },
    Fixture {
        dir: "features",
        tag: "sme",
        path: "src/build/windows/kbdl/testdata/features.yaml",
    },
];

fn generated(fixture: &Fixture) -> (GeneratedLayout, Diagnostics) {
    let path = Path::new(WORKSPACE).join(fixture.path);
    let yaml = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", fixture.path));
    let bundle = fixture_bundle(fixture.path, fixture.tag, &yaml);
    let mut layouts = source_layouts(&bundle.bundle)
        .unwrap_or_else(|e| panic!("{}: {e:#}", fixture.path))
        .into_iter()
        .map(|mut layout| {
            let generated = generate(&layout.input, layout.model.as_deref(), &mut layout.diag)
                .unwrap_or_else(|e| panic!("{}: {e:#}", fixture.path));
            (generated, layout.diag)
        });
    let first = layouts.next().expect("the fixture has a Windows layout");
    assert!(layouts.next().is_none(), "{}: one layout", fixture.path);
    first
}

/// The warnings and information messages, one per line.
fn diagnostics(diag: &Diagnostics) -> String {
    let warnings = diag.warnings().iter().map(|w| format!("warning: {w}\n"));
    let infos = diag.infos().iter().map(|i| format!("info: {i}\n"));
    warnings.chain(infos).collect()
}

/// The path of the fixture's golden file `name` and its bytes, when they
/// differ from `actual`. With `KBDGEN_BLESS` set, the file is first
/// rewritten with `actual`.
fn mismatch(fixture: &Fixture, name: &str, actual: &[u8]) -> Option<(PathBuf, Vec<u8>)> {
    let path = Path::new(WORKSPACE)
        .join(GOLDEN_DIR)
        .join(fixture.dir)
        .join(name);
    if std::env::var_os("KBDGEN_BLESS").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, actual).unwrap();
    }
    let expected = std::fs::read(&path).unwrap_or_else(|e| {
        panic!(
            "{}: {e}; run with KBDGEN_BLESS=1 to create it",
            path.display()
        )
    });
    (expected != actual).then_some((path, expected))
}

/// The first region where `expected` and `actual` differ, as `-`/`+`
/// lines after the common prefix and before the common suffix.
fn line_diff(expected: &str, actual: &str) -> String {
    let expected: Vec<&str> = expected.lines().collect();
    let actual: Vec<&str> = actual.lines().collect();
    let prefix = expected
        .iter()
        .zip(&actual)
        .take_while(|(e, a)| e == a)
        .count();
    let suffix = expected[prefix..]
        .iter()
        .rev()
        .zip(actual[prefix..].iter().rev())
        .take_while(|(e, a)| e == a)
        .count();
    let mut out = format!("@@ line {} @@\n", prefix + 1);
    for (sign, lines) in [
        ('-', &expected[prefix..expected.len() - suffix]),
        ('+', &actual[prefix..actual.len() - suffix]),
    ] {
        for line in lines.iter().take(DIFF_LINES) {
            let _ = writeln!(out, "{sign}{line}");
        }
        if lines.len() > DIFF_LINES {
            let _ = writeln!(out, "{sign}… {} more", lines.len() - DIFF_LINES);
        }
    }
    out
}

fn golden_text(fixture: &Fixture, name: &str, actual: &str) {
    if let Some((path, expected)) = mismatch(fixture, name, actual.as_bytes()) {
        panic!(
            "{} differs from its golden file; rerun with KBDGEN_BLESS=1 to update it\n{}",
            path.display(),
            line_diff(&String::from_utf8_lossy(&expected), actual)
        );
    }
}

fn hex_window(bytes: &[u8], at: usize) -> String {
    let start = at.saturating_sub(16);
    let end = (at + 16).min(bytes.len());
    let hex: Vec<String> = bytes[start..end]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    format!("0x{start:x}: {}", hex.join(" "))
}

/// Each resource of a `.res` file as `type name language: size`.
fn res_summary(res: &[u8]) -> String {
    let mut out = String::new();
    for (kind, name, language, _, data) in resources::tests::entries(res) {
        let _ = writeln!(
            out,
            "  type {kind} name {name} language 0x{language:04x}: {} bytes",
            data.len()
        );
    }
    out
}

/// Compares the `.res` file `actual` with the fixture's golden file `name`,
/// reporting the first differing byte and both files' resources.
fn golden_res(fixture: &Fixture, name: &str, actual: &[u8]) {
    let Some((path, expected)) = mismatch(fixture, name, actual) else {
        return;
    };
    let at = expected
        .iter()
        .zip(actual)
        .take_while(|(e, a)| e == a)
        .count();
    panic!(
        "{} differs from its golden file at byte 0x{at:x} ({} bytes expected, {} actual); rerun with KBDGEN_BLESS=1 to update it\n-{}\n+{}\nexpected resources:\n{}actual resources:\n{}",
        path.display(),
        expected.len(),
        actual.len(),
        hex_window(&expected, at),
        hex_window(actual, at),
        res_summary(&expected),
        res_summary(actual)
    );
}

// [spec:kbdgen:syn:kbdl.source/test/golden]
// [spec:kbdgen:def:kbdl.crate/test/golden]
// [spec:kbdgen:syn:kbdl.escaping/test/golden]
// [spec:kbdgen:def:kbdl.structs/test/golden]
// [spec:kbdgen:req:kbdl.structs.pointers/test/golden]
// [spec:kbdgen:thm:kbdl.structs.layout/test/golden]
// [spec:kbdgen:req:kbdl.export/test/golden]
// [spec:kbdgen:def:kbdl.tables/test/golden]
// [spec:kbdgen:def:kbdl.layers/test/golden]
// [spec:kbdgen:req:kbdl.modifiers/test/golden]
// [spec:kbdgen:req:kbdl.vk-chars/test/golden]
// [spec:kbdgen:sem:kbdl.vk-chars.values/test/golden]
// [spec:kbdgen:def:kbdl.vk-chars.fixed/test/golden]
// [spec:kbdgen:req:kbdl.vk-chars.decimal/test/golden]
// [spec:kbdgen:sem:kbdl.caps/test/golden]
// [spec:kbdgen:req:kbdl.caps.sgcaps/test/golden]
// [spec:kbdgen:req:kbdl.dead-keys+2/test/golden]
// [spec:kbdgen:req:kbdl.dead-keys.table/test/golden]
// [spec:kbdgen:req:kbdl.dead-keys.chains/test/golden]
// [spec:kbdgen:req:kbdl.dead-keys.names+1/test/golden]
// [spec:kbdgen:req:kbdl.ligatures+1/test/golden]
// [spec:kbdgen:def:kbdl.scancodes/test/golden]
// [spec:kbdgen:def:kbdl.scancodes.extended/test/golden]
// [spec:kbdgen:req:kbdl.scancodes.extra-modifiers/test/golden]
// [spec:kbdgen:req:kbdl.scancodes.iso/test/golden]
// [spec:kbdgen:req:kbdl.key-names/test/golden]
// [spec:kbdgen:req:kbdl.locale/test/golden]
// [spec:kbdgen:req:kbdl.input.bundle+1/test/golden]
// [spec:kbdgen:req:ldml.kbdl.windows-inputs+1/test/golden]
#[test]
fn fixture_crates_match_their_golden_files() {
    for fixture in &FIXTURES {
        let (layout, diag) = generated(fixture);
        golden_text(fixture, "Cargo.toml.golden", &layout.cargo_toml);
        golden_text(fixture, "lib.rs.golden", &layout.lib_rs);
        golden_text(fixture, "diagnostics.txt", &diagnostics(&diag));
    }
}

// [spec:kbdgen:req:kbdl.resources/test/golden]
// [spec:kbdgen:syn:kbdl.resources.format+1/test/golden]
// [spec:kbdgen:def:kbdl.resources.fields/test/golden]
// [spec:kbdgen:syn:kbdl.resources.version/test/golden]
// [spec:kbdgen:req:kbdl.metadata.bundle/test/golden]
// [spec:kbdgen:req:kbdl.metadata.locale/test/golden]
// [spec:kbdgen:req:ldml.kbdl.model-resource+1/test/golden]
// [spec:kbdgen:req:tsf.data.resource/test/golden]
#[test]
fn fixture_res_files_match_their_golden_files() {
    for fixture in &FIXTURES {
        let (layout, _) = generated(fixture);
        golden_res(fixture, &format!("{}.res", layout.name), &layout.res);
    }
}
