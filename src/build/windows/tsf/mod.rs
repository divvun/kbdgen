//! Builds the Windows text service for release (`kbdgen tsf`): the three
//! DLLs cargo builds from `crates/kbd-tsf` and the Arm64X forwarder that
//! `rust-lld` links from generated `.def` files and load configuration
//! objects. The text service is released separately from keyboards, so no
//! `kbdgen target` pipeline runs this.

use std::{
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context, Result, bail};

use super::kbdl::build::{check_toolchain, scrub};

pub mod coff;
pub mod verify;

/// The text service crate, relative to the kbdgen workspace.
pub const CRATE_DIR: &str = "crates/kbd-tsf";

/// The COM entry points the text service exports, in export table order.
pub const EXPORTS: [&str; 4] = [
    "DllCanUnloadNow",
    "DllGetClassObject",
    "DllRegisterServer",
    "DllUnregisterServer",
];

/// The Arm64X forwarder's file name.
pub const FORWARDER: &str = "divvun_tip.dll";

/// One DLL of the text service.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Architecture {
    /// The DLL's module name; its file is `<module>.dll`.
    pub module: &'static str,
    pub triple: &'static str,
    /// The PE machine type the DLL must have.
    pub machine: u16,
}

// [spec:kbdgen:req:tsf.arch.builds+1]
pub const ARCHITECTURES: [Architecture; 3] = [
    Architecture {
        module: "divvun_tip_x86",
        triple: "i686-pc-windows-msvc",
        machine: 0x014c,
    },
    Architecture {
        module: "divvun_tip_x64",
        triple: "x86_64-pc-windows-msvc",
        machine: 0x8664,
    },
    Architecture {
        module: "divvun_tip_arm64",
        triple: "aarch64-pc-windows-msvc",
        machine: coff::MACHINE_ARM64,
    },
];

/// The `cargo rustc` invocation that builds the text service for one
/// architecture. The C runtime is linked statically, since the text
/// service loads into processes on machines without the Visual C++
/// redistributable.
// [spec:kbdgen:req:tsf.arch.builds+1]
pub fn cargo_command(workspace: &Path, architecture: &Architecture) -> Command {
    let mut command = Command::new("cargo");
    command
        .current_dir(workspace)
        .args([
            "rustc",
            "--package",
            "kbd-tsf",
            "--release",
            "--lib",
            "--target",
            architecture.triple,
            "--target-dir",
        ])
        .arg(workspace.join("target"))
        .args(["--", "-C", "target-feature=+crt-static"]);
    scrub(&mut command);
    command
}

/// The `.def` file of one view of the forwarder: every export forwarded
/// to the same name in `module`.
// [spec:kbdgen:req:tsf.arch.arm64x+1]
pub fn forwarder_def(module: &str) -> String {
    let mut def = String::from("LIBRARY divvun_tip\nEXPORTS\n");
    for name in EXPORTS {
        def.push_str(&format!("    {name}={module}.{name}\n"));
    }
    def
}

/// The `rust-lld` invocation that links the forwarder in `dir`, which
/// holds `native.def`, `ec.def`, `native.obj` and `ec.obj`.
// [spec:kbdgen:req:tsf.arch.arm64x+1]
pub fn forwarder_command(lld: &Path, dir: &Path) -> Command {
    let mut command = Command::new(lld);
    command.args(["-flavor", "link", "-dll", "-noentry", "-machine:arm64x"]);
    for (prefix, file) in [
        ("-defarm64native:", "native.def"),
        ("-def:", "ec.def"),
        ("", "native.obj"),
        ("", "ec.obj"),
        ("-out:", FORWARDER),
    ] {
        let mut arg = std::ffi::OsString::from(prefix);
        arg.push(dir.join(file));
        command.arg(arg);
    }
    command.arg("-brepro");
    command
}

/// Links the forwarder with `lld` in a temporary directory and returns it.
// [spec:kbdgen:req:tsf.arch.arm64x+1]
pub fn link_forwarder(lld: &Path) -> Result<Vec<u8>> {
    let [_, x64, arm64] = &ARCHITECTURES;
    let dir = tempfile::tempdir().context("create a directory for the forwarder")?;
    for (file, bytes) in [
        ("native.def", forwarder_def(arm64.module).into_bytes()),
        ("ec.def", forwarder_def(x64.module).into_bytes()),
        (
            "native.obj",
            coff::object(coff::MACHINE_ARM64, coff::NATIVE),
        ),
        ("ec.obj", coff::object(coff::MACHINE_ARM64EC, coff::EC)),
    ] {
        let path = dir.path().join(file);
        std::fs::write(&path, bytes).with_context(|| format!("write {}", path.display()))?;
    }
    let output = forwarder_command(lld, dir.path())
        .output()
        .with_context(|| format!("`{}` could not be started", lld.display()))?;
    if !output.status.success() {
        bail!(
            "`{}` failed to link {FORWARDER} with {}: {}",
            lld.display(),
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    for (stream, bytes) in [("stdout", &output.stdout), ("stderr", &output.stderr)] {
        if !bytes.is_empty() {
            tracing::trace!(
                "rust-lld {stream}:\n{}",
                String::from_utf8_lossy(bytes).trim_end()
            );
        }
    }
    let path = dir.path().join(FORWARDER);
    std::fs::read(&path).with_context(|| format!("read {}", path.display()))
}

/// Builds and verifies the three DLLs and the forwarder, then writes them
/// with [`write_release`]. Nothing is written unless all four pass.
// [spec:kbdgen:req:tsf.arch.builds+1]
// [spec:kbdgen:req:tsf.arch.arm64x+1]
// [spec:kbdgen:req:tsf.security.signing+2]
pub fn build(workspace: &Path, output_path: &Path) -> Result<Vec<PathBuf>> {
    let workspace = dunce::canonicalize(workspace)
        .with_context(|| format!("workspace {} is missing", workspace.display()))?;
    if !workspace.join(CRATE_DIR).join("Cargo.toml").is_file() {
        bail!(
            "{} has no {CRATE_DIR}; pass the kbdgen workspace",
            workspace.display()
        );
    }
    let lld = check_toolchain(&workspace)?;
    tracing::debug!("Using rust-lld at {}", lld.display());
    let mut files = Vec::new();
    for architecture in &ARCHITECTURES {
        let triple = architecture.triple;
        tracing::info!("Building the text service for {triple}");
        // Stdout carries only the paths of the files to sign.
        let status = cargo_command(&workspace, architecture)
            .stdout(std::io::stderr())
            .status()
            .with_context(|| format!("kbd-tsf ({triple}): cargo could not be started"))?;
        if !status.success() {
            bail!("kbd-tsf ({triple}): cargo failed with {status}");
        }
        let built = workspace
            .join("target")
            .join(triple)
            .join("release")
            .join("kbd_tsf.dll");
        let bytes = std::fs::read(&built).with_context(|| format!("read {}", built.display()))?;
        verify::verify_tip(&bytes, architecture.machine)
            .with_context(|| format!("{} is not a valid text service DLL", built.display()))?;
        tracing::debug!("{} passed its checks", built.display());
        files.push((format!("{}.dll", architecture.module), bytes));
    }
    tracing::info!("Linking the Arm64X forwarder {FORWARDER}");
    let forwarder = link_forwarder(&lld)?;
    verify::verify_forwarder(&forwarder)
        .with_context(|| format!("{FORWARDER} is not a valid Arm64X forwarder"))?;
    files.push((FORWARDER.to_owned(), forwarder));

    write_release(output_path, files)
}

/// Writes the release's PE files, which must be exactly the three DLLs
/// and the forwarder, to `output_path` and returns their absolute paths,
/// the list of files to sign.
// [spec:kbdgen:req:tsf.security.signing+2]
pub fn write_release(output_path: &Path, files: Vec<(String, Vec<u8>)>) -> Result<Vec<PathBuf>> {
    let mut expected: Vec<String> = ARCHITECTURES
        .iter()
        .map(|architecture| format!("{}.dll", architecture.module))
        .collect();
    expected.push(FORWARDER.to_owned());
    let names: Vec<&String> = files.iter().map(|(name, _)| name).collect();
    if names.iter().copied().ne(expected.iter()) {
        bail!("the release must be exactly {expected:?}, not {names:?}");
    }
    std::fs::create_dir_all(output_path)
        .with_context(|| format!("create {}", output_path.display()))?;
    let output_path = dunce::canonicalize(output_path)
        .with_context(|| format!("canonicalize {}", output_path.display()))?;
    let paths = files
        .into_iter()
        .map(|(name, bytes)| {
            let path = output_path.join(name);
            std::fs::write(&path, bytes).with_context(|| format!("write {}", path.display()))?;
            Ok(path)
        })
        .collect::<Result<_>>()?;
    tracing::info!("Wrote the text service to {}", output_path.display());
    Ok(paths)
}

#[cfg(test)]
mod tests;
