//! `kbd-model` and `kbd-engine`, with default features, build without std
//! for the device triples.
//!
//! Each check runs a nested `cargo build` of only those two packages, so
//! their features resolve as a host embedding them would see them, without
//! the std features that the rest of the workspace unifies in. The nested
//! build uses its own target directory under `CARGO_TARGET_TMPDIR`, so it
//! neither contends for the outer build's lock nor invalidates its cache.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The triples of `ldml.crate.targets`.
const DEVICE_TRIPLES: &[&str] = &[
    "wasm32-unknown-unknown",
    "x86_64-pc-windows-msvc",
    "i686-pc-windows-msvc",
    "aarch64-pc-windows-msvc",
    "aarch64-apple-darwin",
    "x86_64-apple-darwin",
    "aarch64-apple-ios",
    "aarch64-linux-android",
    "x86_64-unknown-linux-gnu",
];

/// A triple whose sysroot has `core` and `alloc` but no `std`. Building for
/// it fails if either crate, or anything in its dependency tree, links std.
const BARE_METAL_TRIPLE: &str = "x86_64-unknown-none";

fn workspace_manifest() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Cargo.toml")
}

fn build_device_crates(triples: &[&str]) -> std::io::Result<()> {
    let mut command = Command::new(env!("CARGO"));
    command
        .arg("build")
        .arg("--manifest-path")
        .arg(workspace_manifest())
        .args(["--package", "kbd-model", "--package", "kbd-engine"])
        .arg("--target-dir")
        .arg(Path::new(env!("CARGO_TARGET_TMPDIR")).join("device-triples"));
    for triple in triples {
        command.args(["--target", triple]);
    }
    for key in ["RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "CARGO_BUILD_TARGET"] {
        command.env_remove(key);
    }

    let output = command.output()?;
    assert!(
        output.status.success(),
        "building kbd-model and kbd-engine for {triples:?} failed; each target \
         must be installed (`rustup target add <triple>`):\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}

// [spec:kbdgen:req:ldml.crate.targets/test]
// [spec:kbdgen:req:ldml.crate.model/test]
// [spec:kbdgen:req:ldml.crate.engine+1/test]
#[test]
fn device_crates_build_for_wasm_and_without_std() -> std::io::Result<()> {
    build_device_crates(&["wasm32-unknown-unknown", BARE_METAL_TRIPLE])
}

// [spec:kbdgen:req:ldml.crate.targets/test]
#[test]
#[ignore = "builds for every device triple; run with `cargo nextest run --run-ignored all`"]
fn device_crates_build_for_every_device_triple() -> std::io::Result<()> {
    build_device_crates(DEVICE_TRIPLES)
}
