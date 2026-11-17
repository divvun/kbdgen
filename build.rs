//! Builds `crates/kbd-wasm` for `wasm32-unknown-unknown` and leaves the
//! module at `$OUT_DIR/kbd_wasm.wasm`, which the ChromeOS target embeds
//! (`chromeos.engine`). Building it here, from the same workspace, is what
//! keeps the engine in every extension in step with the model encoding
//! this kbdgen writes, and lets a released binary generate an extension
//! without a Rust toolchain.
//!
//! The nested cargo gets its own target directory under `OUT_DIR`: sharing
//! the outer build's would wait forever on the lock the outer build holds.

use std::path::{Path, PathBuf};
use std::process::Command;

const TARGET: &str = "wasm32-unknown-unknown";

/// What the module is built from; any change rebuilds it.
const SOURCES: [&str; 6] = [
    "crates/kbd-wasm",
    "crates/kbd-engine/src",
    "crates/kbd-engine/Cargo.toml",
    "crates/kbd-model/src",
    "crates/kbd-model/Cargo.toml",
    "Cargo.lock",
];

/// Release settings for a small module, given on the command line so that
/// they apply to the nested build only.
const PROFILE: [&str; 5] = [
    "profile.release.opt-level=\"s\"",
    "profile.release.lto=true",
    "profile.release.codegen-units=1",
    "profile.release.panic=\"abort\"",
    "profile.release.strip=true",
];

/// Variables cargo sets for this script that would steer the nested build
/// wrongly: the outer build's rustc flags are for the host triple, and the
/// workspace wrapper is clippy's when this runs under `cargo clippy`.
const OUTER_ONLY: [&str; 6] = [
    "CARGO_ENCODED_RUSTFLAGS",
    "RUSTFLAGS",
    "CARGO_BUILD_RUSTFLAGS",
    "RUSTC_WORKSPACE_WRAPPER",
    "CARGO_TARGET_DIR",
    "CARGO_BUILD_TARGET",
];

fn env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("cargo sets {name} for build scripts"))
}

fn main() {
    let root = PathBuf::from(env("CARGO_MANIFEST_DIR"));
    for source in SOURCES {
        println!("cargo::rerun-if-changed={}", root.join(source).display());
    }
    let out = PathBuf::from(env("OUT_DIR"));
    let target_dir = out.join("kbd-wasm");

    let mut cargo = Command::new(env("CARGO"));
    cargo
        .args(["build", "--package", "kbd-wasm", "--release", "--locked"])
        .args(["--target", TARGET])
        .arg("--manifest-path")
        .arg(root.join("Cargo.toml"))
        .arg("--target-dir")
        .arg(&target_dir);
    for setting in PROFILE {
        cargo.args(["--config", setting]);
    }
    for name in OUTER_ONLY {
        cargo.env_remove(name);
    }
    let status = cargo
        .status()
        .unwrap_or_else(|e| panic!("kbdgen: could not run cargo to build kbd-wasm: {e}"));
    if !status.success() {
        panic!(
            "kbdgen: building kbd-wasm for {TARGET} failed with {status}. The ChromeOS target \
             embeds kbd-engine compiled to WebAssembly; if the target is missing, install it \
             with `rustup target add {TARGET}`"
        );
    }
    copy(
        &target_dir.join(TARGET).join("release/kbd_wasm.wasm"),
        &out.join("kbd_wasm.wasm"),
    );
}

fn copy(from: &Path, to: &Path) {
    std::fs::copy(from, to)
        .unwrap_or_else(|e| panic!("kbdgen: copy {} to {}: {e}", from.display(), to.display()));
}
