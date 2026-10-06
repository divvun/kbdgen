//! Builds the generated layout crates into DLLs with `cargo` and `rust-lld`
//! for every Windows variant, and verifies each image.

use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use anyhow::{Context, Result, bail};

use super::{CRATES_DIR, image};

/// One DLL flavour a layout is built in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Variant {
    /// Output directory under `<out>/`, and the variant's name in messages.
    pub name: &'static str,
    pub triple: &'static str,
    pub wow64: bool,
    /// Cargo's target directory inside the crate.
    pub target_dir: &'static str,
    /// The PE machine type the DLL must have.
    pub machine: u16,
}

/// The four variants, in build order. `wow64` is 32-bit x86 code with the
/// 64-bit table layout, installed to `SysWOW64` beside the `x64` DLL in
/// `System32` under the same file name, because a 32-bit process's
/// `LoadKeyboardLayout` hands that file to the 64-bit kernel. A plain `x86`
/// DLL in its place does not load.
// [spec:kbdgen:req:kbdl.build]
// [spec:kbdgen:req:kbdl.wow64]
pub const VARIANTS: [Variant; 4] = [
    Variant {
        name: "x86",
        triple: "i686-pc-windows-msvc",
        wow64: false,
        target_dir: "target",
        machine: 0x014c,
    },
    Variant {
        name: "x64",
        triple: "x86_64-pc-windows-msvc",
        wow64: false,
        target_dir: "target",
        machine: 0x8664,
    },
    Variant {
        name: "arm64",
        triple: "aarch64-pc-windows-msvc",
        wow64: false,
        target_dir: "target",
        machine: 0xaa64,
    },
    Variant {
        name: "wow64",
        triple: "i686-pc-windows-msvc",
        wow64: true,
        target_dir: "target-wow64",
        machine: 0x014c,
    },
];

/// The distinct target triples the variants need.
const TRIPLES: [&str; 3] = [
    "i686-pc-windows-msvc",
    "x86_64-pc-windows-msvc",
    "aarch64-pc-windows-msvc",
];

/// The first Rust release with edition 2024.
const MIN_RUST: (u32, u32) = (1, 85);

/// Linker arguments after the `.res` file's position is filled in. Code and
/// data share one read-execute `.data` section, which `LoadKeyboardLayout`
/// requires; `/NODEFAULTLIB` drops `msvcrt.lib`, which non-Windows hosts
/// lack; `/DEBUG:NONE` overrides rustc's `/DEBUG` so no PDB is written; and
/// `/Brepro` replaces the timestamp with a content hash.
// [spec:kbdgen:req:kbdl.build]
// [spec:kbdgen:req:kbdl.image]
fn link_args(res: &Path) -> Vec<OsString> {
    let mut args: Vec<OsString> = ["/NOENTRY", "/NODEFAULTLIB", "/SUBSYSTEM:NATIVE"]
        .iter()
        .map(OsString::from)
        .collect();
    args.push(res.as_os_str().to_owned());
    args.extend(
        [
            "/MERGE:.rdata=.data",
            "/MERGE:.text=.data",
            "/MERGE:.bss=.data",
            "/SECTION:.data,RE",
            "/DEBUG:NONE",
            "/Brepro",
        ]
        .iter()
        .map(OsString::from),
    );
    args
}

/// Whether cargo must not inherit `key`, because it would add to or
/// replace the link arguments, the target or the release profile.
// [spec:kbdgen:req:kbdl.build.environment]
pub fn is_scrubbed(key: &str) -> bool {
    matches!(
        key,
        "RUSTFLAGS" | "CARGO_ENCODED_RUSTFLAGS" | "CARGO_BUILD_RUSTFLAGS" | "CARGO_BUILD_TARGET"
    ) || key.starts_with("CARGO_TARGET_")
        || key.starts_with("CARGO_PROFILE_")
}

/// Removes every scrubbed variable of the current environment from
/// `command`'s environment.
// [spec:kbdgen:req:kbdl.build.environment]
pub fn scrub(command: &mut Command) {
    for (key, _) in std::env::vars_os() {
        if key.to_str().is_some_and(is_scrubbed) {
            command.env_remove(key);
        }
    }
}

/// The `cargo rustc` invocation for one crate and variant.
// [spec:kbdgen:req:kbdl.build]
pub fn cargo_command(crate_dir: &Path, name: &str, variant: &Variant) -> Command {
    let mut command = Command::new("cargo");
    command.current_dir(crate_dir).args([
        "rustc",
        "--release",
        "--lib",
        "--target",
        variant.triple,
    ]);
    if variant.wow64 {
        command.args(["--features", "wow64"]);
    }
    command
        .arg("--target-dir")
        .arg(crate_dir.join(variant.target_dir))
        .args(["--", "-C", "linker=rust-lld"]);
    for arg in link_args(&crate_dir.join(format!("{name}.res"))) {
        let mut link_arg = OsString::from("link-arg=");
        link_arg.push(arg);
        command.arg("-C").arg(link_arg);
    }
    scrub(&mut command);
    command
}

/// Where cargo leaves a crate's DLL for a variant.
pub fn built_dll(crate_dir: &Path, name: &str, variant: &Variant) -> PathBuf {
    crate_dir
        .join(variant.target_dir)
        .join(variant.triple)
        .join("release")
        .join(format!("{}.dll", name.replace('-', "_")))
}

/// Where kbdgen places a layout's DLL for a variant: `<out>/<variant>/`.
// [spec:kbdgen:req:kbdl.build]
// [spec:kbdgen:req:kbdl.wow64]
pub fn output_dll(output_path: &Path, name: &str, variant: &Variant) -> PathBuf {
    output_path.join(variant.name).join(format!("{name}.dll"))
}

/// Runs a probe command, describing it in errors.
type Probe<'a> = dyn Fn(&str, &[&str]) -> std::io::Result<Output> + 'a;

fn probe_stdout(probe: &Probe, program: &str, args: &[&str]) -> Result<String> {
    let output = probe(program, args).with_context(|| {
        format!(
            "`{program}` could not be started; install Rust with rustup (https://rustup.rs) and make sure `{program}` is on PATH"
        )
    })?;
    if !output.status.success() {
        bail!(
            "`{program} {}` failed with {}: {}",
            args.join(" "),
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

/// The (major, minor) of a `rustc --version` line such as
/// `rustc 1.99.0 (b940084d7 2026-09-28)`.
fn rust_version(line: &str) -> Option<(u32, u32)> {
    let version = line.split_whitespace().nth(1)?;
    let mut parts = version.split(['.', '-']);
    Some((parts.next()?.parse().ok()?, parts.next()?.parse().ok()?))
}

/// Checks that `cargo` and `rustc` start, that rustc supports edition
/// 2024, that every Windows target's standard library is installed, and
/// that the toolchain ships `rust-lld`, whose path it returns.
// [spec:kbdgen:req:kbdl.build.toolchain+1]
fn check_toolchain_with(probe: &Probe) -> Result<PathBuf> {
    probe_stdout(probe, "cargo", &["--version"])?;
    let version = probe_stdout(probe, "rustc", &["--version"])?;
    match rust_version(&version) {
        Some(found) if found >= MIN_RUST => {}
        _ => bail!(
            "`{version}` is too old to build layout crates, which use edition 2024; Rust {}.{} or later is required (`rustup update`)",
            MIN_RUST.0,
            MIN_RUST.1
        ),
    }
    let mut missing = Vec::new();
    for triple in TRIPLES {
        let libdir = probe_stdout(
            probe,
            "rustc",
            &["--print", "target-libdir", "--target", triple],
        )?;
        if !Path::new(&libdir).is_dir() {
            missing.push(triple);
        }
    }
    if !missing.is_empty() {
        bail!(
            "the Rust standard library is missing for {}; install it with `rustup target add {}`",
            missing.join(", "),
            missing.join(" ")
        );
    }
    let sysroot = probe_stdout(probe, "rustc", &["--print", "sysroot"])?;
    let host = probe_stdout(probe, "rustc", &["-vV"])?
        .lines()
        .find_map(|line| line.strip_prefix("host: ").map(str::to_owned))
        .context("`rustc -vV` does not name the host triple")?;
    let bin = Path::new(&sysroot)
        .join("lib")
        .join("rustlib")
        .join(&host)
        .join("bin");
    match ["rust-lld", "rust-lld.exe"]
        .map(|name| bin.join(name))
        .into_iter()
        .find(|path| path.is_file())
    {
        Some(lld) => Ok(lld),
        None => bail!(
            "the toolchain has no rust-lld in {}; use a rustup-installed toolchain, which ships it",
            bin.display()
        ),
    }
}

/// [`check_toolchain_with`] against the real `cargo` and `rustc`, run in
/// `dir` so that the same rustup toolchain override applies as to the
/// builds.
// [spec:kbdgen:req:kbdl.build.toolchain+1]
pub fn check_toolchain(dir: &Path) -> Result<PathBuf> {
    check_toolchain_with(&|program, args| {
        let mut command = Command::new(program);
        command.args(args).current_dir(dir);
        scrub(&mut command);
        command.output()
    })
}

/// Builds one layout's crate for every variant, verifies each DLL and
/// copies it to `<out>/<variant>/<name>.dll`.
// [spec:kbdgen:req:kbdl.build]
// [spec:kbdgen:req:kbdl.image.verify]
pub fn build_layout(output_path: &Path, name: &str) -> Result<()> {
    let crate_dir = output_path.join(CRATES_DIR).join(name);
    let crate_dir = dunce::canonicalize(&crate_dir)
        .with_context(|| format!("{name}: crate directory {} is missing", crate_dir.display()))?;
    for variant in &VARIANTS {
        tracing::info!("Building {name} for {}", variant.name);
        let status = cargo_command(&crate_dir, name, variant)
            .status()
            .with_context(|| format!("{name} ({}): cargo could not be started", variant.name))?;
        if !status.success() {
            bail!("{name} ({}): cargo failed with {status}", variant.name);
        }
        let built = built_dll(&crate_dir, name, variant);
        let bytes = std::fs::read(&built)
            .with_context(|| format!("{name} ({}): read {}", variant.name, built.display()))?;
        image::verify(&bytes, variant.machine).with_context(|| {
            format!(
                "{name} ({}): {} is not a valid layout DLL",
                variant.name,
                built.display()
            )
        })?;
        let target = output_dll(output_path, name, variant);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create {}", parent.display()))?;
        }
        std::fs::write(&target, &bytes).with_context(|| format!("write {}", target.display()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // [spec:kbdgen:req:kbdl.build/test]
    // [spec:kbdgen:req:kbdl.wow64/test]
    #[test]
    fn cargo_invocation_per_variant() {
        let dir = Path::new("/out/build/kbdse-FI");
        let wow64 = VARIANTS.iter().find(|variant| variant.wow64).unwrap();
        let command = cargo_command(dir, "kbdse-FI", wow64);
        assert_eq!(command.get_program(), "cargo");
        assert_eq!(command.get_current_dir(), Some(dir));
        let args: Vec<String> = command
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        let path = |tail: &str| dir.join(tail).to_string_lossy().into_owned();
        let mut expected: Vec<String> = [
            "rustc",
            "--release",
            "--lib",
            "--target",
            "i686-pc-windows-msvc",
            "--features",
            "wow64",
            "--target-dir",
        ]
        .map(str::to_owned)
        .to_vec();
        expected.push(path("target-wow64"));
        for arg in ["--", "-C", "linker=rust-lld"] {
            expected.push(arg.to_owned());
        }
        for link_arg in [
            "/NOENTRY".to_owned(),
            "/NODEFAULTLIB".to_owned(),
            "/SUBSYSTEM:NATIVE".to_owned(),
            path("kbdse-FI.res"),
            "/MERGE:.rdata=.data".to_owned(),
            "/MERGE:.text=.data".to_owned(),
            "/MERGE:.bss=.data".to_owned(),
            "/SECTION:.data,RE".to_owned(),
            "/DEBUG:NONE".to_owned(),
            "/Brepro".to_owned(),
        ] {
            expected.push("-C".to_owned());
            expected.push(format!("link-arg={link_arg}"));
        }
        assert_eq!(args, expected);

        let x64 = cargo_command(dir, "kbdse-FI", &VARIANTS[1]);
        let args: Vec<_> = x64.get_args().collect();
        assert_eq!(args[4], "x86_64-pc-windows-msvc");
        assert_eq!(args[5], "--target-dir");
        assert_eq!(args[6], dir.join("target").as_os_str());

        assert_eq!(
            built_dll(dir, "kbdse-FI", wow64),
            dir.join("target-wow64")
                .join("i686-pc-windows-msvc")
                .join("release")
                .join("kbdse_FI.dll")
        );
        let names: Vec<_> = VARIANTS
            .iter()
            .map(|variant| output_dll(Path::new("/out"), "kbdse-FI", variant))
            .collect();
        assert_eq!(
            names,
            ["x86", "x64", "arm64", "wow64"]
                .map(|variant| Path::new("/out").join(variant).join("kbdse-FI.dll"))
        );
        assert_eq!(
            VARIANTS.map(|variant| (variant.triple, variant.machine)),
            [
                ("i686-pc-windows-msvc", 0x014c),
                ("x86_64-pc-windows-msvc", 0x8664),
                ("aarch64-pc-windows-msvc", 0xaa64),
                ("i686-pc-windows-msvc", 0x014c),
            ]
        );
    }

    // [spec:kbdgen:req:kbdl.build.environment/test]
    #[test]
    fn inherited_flags_and_profiles_are_scrubbed() {
        for key in [
            "RUSTFLAGS",
            "CARGO_ENCODED_RUSTFLAGS",
            "CARGO_BUILD_RUSTFLAGS",
            "CARGO_BUILD_TARGET",
            "CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER",
            "CARGO_TARGET_DIR",
            "CARGO_PROFILE_RELEASE_LTO",
        ] {
            assert!(is_scrubbed(key), "{key}");
        }
        for key in [
            "PATH",
            "CARGO_HOME",
            "RUSTUP_TOOLCHAIN",
            "CARGO_BUILD_JOBS",
            "RUSTDOCFLAGS",
        ] {
            assert!(!is_scrubbed(key), "{key}");
        }

        let inherited: Vec<String> = std::env::vars()
            .map(|(key, _)| key)
            .filter(|key| is_scrubbed(key))
            .collect();
        let command = cargo_command(Path::new("/c"), "k", &VARIANTS[0]);
        for key in inherited {
            assert!(
                command
                    .get_envs()
                    .any(|(name, value)| name == key.as_str() && value.is_none()),
                "{key} is not removed"
            );
        }
    }

    #[cfg(unix)]
    fn status(code: i32) -> std::process::ExitStatus {
        use std::os::unix::process::ExitStatusExt as _;
        std::process::ExitStatus::from_raw(code << 8)
    }

    #[cfg(windows)]
    fn status(code: i32) -> std::process::ExitStatus {
        use std::os::windows::process::ExitStatusExt as _;
        std::process::ExitStatus::from_raw(code as u32)
    }

    fn output(stdout: &str, code: i32) -> std::io::Result<Output> {
        Ok(Output {
            status: status(code),
            stdout: stdout.as_bytes().to_vec(),
            stderr: b"boom".to_vec(),
        })
    }

    /// A fake toolchain in which only the triples in `libdirs` have an
    /// existing library directory.
    fn fake(
        version: &'static str,
        libdirs: &'static [&'static str],
        sysroot: PathBuf,
    ) -> impl Fn(&str, &[&str]) -> std::io::Result<Output> {
        move |program, args| match (program, args) {
            ("cargo", _) => output("cargo 1.99.0", 0),
            ("rustc", ["--version"]) => output(version, 0),
            ("rustc", ["--print", "target-libdir", "--target", triple]) => {
                if libdirs.contains(triple) {
                    output(&sysroot.to_string_lossy(), 0)
                } else {
                    output("/nonexistent/kbdgen/libdir", 0)
                }
            }
            ("rustc", ["--print", "sysroot"]) => output(&sysroot.to_string_lossy(), 0),
            ("rustc", ["-vV"]) => output("rustc 1.99.0\nhost: aarch64-apple-darwin\n", 0),
            _ => output("", 1),
        }
    }

    // [spec:kbdgen:req:kbdl.build.toolchain+1/test]
    #[test]
    fn toolchain_check_names_what_is_missing() {
        let sysroot = tempfile::tempdir().unwrap();
        let bin = sysroot
            .path()
            .join("lib")
            .join("rustlib")
            .join("aarch64-apple-darwin")
            .join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        let all: &'static [&'static str] = &TRIPLES;

        let error = check_toolchain_with(&fake("rustc 1.99.0 (x)", all, sysroot.path().into()))
            .unwrap_err()
            .to_string();
        assert!(error.contains("rust-lld"), "{error}");

        std::fs::write(bin.join("rust-lld"), "").unwrap();
        let lld =
            check_toolchain_with(&fake("rustc 1.99.0 (x)", all, sysroot.path().into())).unwrap();
        assert_eq!(lld, bin.join("rust-lld"));
        check_toolchain_with(&fake("rustc 1.85.0", all, sysroot.path().into())).unwrap();

        let error = check_toolchain_with(&fake(
            "rustc 1.99.0",
            &["x86_64-pc-windows-msvc"],
            sysroot.path().into(),
        ))
        .unwrap_err()
        .to_string();
        assert!(
            error.contains("rustup target add i686-pc-windows-msvc aarch64-pc-windows-msvc"),
            "{error}"
        );

        let error = check_toolchain_with(&fake("rustc 1.84.1 (x)", all, sysroot.path().into()))
            .unwrap_err()
            .to_string();
        assert!(error.contains("1.85"), "{error}");

        let error = check_toolchain_with(&|program, _| {
            if program == "cargo" {
                Err(std::io::Error::from(std::io::ErrorKind::NotFound))
            } else {
                output("", 0)
            }
        })
        .unwrap_err()
        .to_string();
        assert!(error.contains("`cargo` could not be started"), "{error}");

        let error = check_toolchain_with(&|program, _| match program {
            "cargo" => output("cargo 1.99.0", 0),
            _ => Err(std::io::Error::from(std::io::ErrorKind::NotFound)),
        })
        .unwrap_err()
        .to_string();
        assert!(error.contains("`rustc` could not be started"), "{error}");
    }
}
