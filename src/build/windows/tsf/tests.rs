use std::process::Command;

use super::*;
use crate::build::windows::kbdl::image::Image;

/// The toolchain's `rust-lld`, found the way `kbdl.build.toolchain` finds
/// it but without needing the Windows standard libraries.
fn rust_lld() -> PathBuf {
    let rustc = |args: &[&str]| {
        let output = Command::new("rustc").args(args).output().unwrap();
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
    };
    let host = rustc(&["-vV"])
        .lines()
        .find_map(|line| line.strip_prefix("host: ").map(str::to_owned))
        .unwrap();
    let bin = Path::new(&rustc(&["--print", "sysroot"]))
        .join("lib")
        .join("rustlib")
        .join(host)
        .join("bin");
    ["rust-lld", "rust-lld.exe"]
        .map(|name| bin.join(name))
        .into_iter()
        .find(|path| path.is_file())
        .unwrap()
}

#[track_caller]
fn rejects(bytes: &[u8], needle: &str) {
    let error = format!("{:#}", verify::verify_forwarder(bytes).unwrap_err());
    assert!(error.contains(needle), "{error:?} lacks {needle:?}");
}

// [spec:kbdgen:req:tsf.arch.builds+1/test]
#[test]
fn cargo_builds_each_architecture_statically() {
    let workspace = Path::new("/src/kbdgen");
    let command = cargo_command(workspace, &ARCHITECTURES[2]);
    assert_eq!(command.get_program(), "cargo");
    assert_eq!(command.get_current_dir(), Some(workspace));
    let args: Vec<String> = command
        .get_args()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    let target = workspace.join("target").to_string_lossy().into_owned();
    assert_eq!(
        args,
        [
            "rustc",
            "--package",
            "kbd-tsf",
            "--release",
            "--lib",
            "--target",
            "aarch64-pc-windows-msvc",
            "--target-dir",
            &target,
            "--",
            "-C",
            "target-feature=+crt-static",
        ]
    );
    assert!(
        command
            .get_envs()
            .all(|(key, _)| crate::build::windows::kbdl::build::is_scrubbed(
                &key.to_string_lossy()
            ))
    );
    assert_eq!(
        ARCHITECTURES.map(|a| (a.module, a.triple, a.machine)),
        [
            ("divvun_tip_x86", "i686-pc-windows-msvc", 0x014c),
            ("divvun_tip_x64", "x86_64-pc-windows-msvc", 0x8664),
            ("divvun_tip_arm64", "aarch64-pc-windows-msvc", 0xaa64),
        ]
    );
}

// [spec:kbdgen:req:tsf.arch.arm64x+1/test]
#[test]
fn forwarder_defs_and_link_command() {
    assert_eq!(
        forwarder_def("divvun_tip_x64"),
        "LIBRARY divvun_tip\nEXPORTS\n    DllCanUnloadNow=divvun_tip_x64.DllCanUnloadNow\n    DllGetClassObject=divvun_tip_x64.DllGetClassObject\n    DllRegisterServer=divvun_tip_x64.DllRegisterServer\n    DllUnregisterServer=divvun_tip_x64.DllUnregisterServer\n"
    );
    let dir = Path::new("/tmp/f");
    let command = forwarder_command(Path::new("/bin/rust-lld"), dir);
    assert_eq!(command.get_program(), "/bin/rust-lld");
    let args: Vec<String> = command
        .get_args()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    let path = |file: &str| dir.join(file).to_string_lossy().into_owned();
    assert_eq!(
        args,
        [
            "-flavor".to_owned(),
            "link".to_owned(),
            "-dll".to_owned(),
            "-noentry".to_owned(),
            "-machine:arm64x".to_owned(),
            format!("-defarm64native:{}", path("native.def")),
            format!("-def:{}", path("ec.def")),
            path("native.obj"),
            path("ec.obj"),
            format!("-out:{}", path("divvun_tip.dll")),
            "-brepro".to_owned(),
        ]
    );
}

// [spec:kbdgen:req:tsf.arch.arm64x+1/test]
#[test]
fn forwarder_is_arm64x_with_both_forward_tables() {
    let bytes = link_forwarder(&rust_lld()).unwrap();
    verify::verify_forwarder(&bytes).unwrap();
    assert_eq!(link_forwarder(&rust_lld()).unwrap(), bytes);

    let native = Image::parse(&bytes).unwrap();
    assert_eq!(native.machine, 0xaa64);
    let forwards = native.exports(&bytes).unwrap().forwards;
    assert_eq!(
        forwards[1],
        (
            "DllGetClassObject".to_owned(),
            "divvun_tip_arm64.DllGetClassObject".to_owned()
        )
    );
    let ec = verify::ec_view(&bytes).unwrap().unwrap();
    let ec_image = Image::parse(&ec).unwrap();
    assert_eq!(ec_image.machine, 0x8664);
    let forwards = ec_image.exports(&ec).unwrap().forwards;
    assert_eq!(forwards.len(), 4);
    assert_eq!(
        forwards[3],
        (
            "DllUnregisterServer".to_owned(),
            "divvun_tip_x64.DllUnregisterServer".to_owned()
        )
    );
    assert!(native.imports(&bytes).unwrap().is_empty());
}

// [spec:kbdgen:req:tsf.arch.arm64x+1/test]
#[test]
fn forwarder_verifier_rejects_broken_images() {
    let bytes = link_forwarder(&rust_lld()).unwrap();
    let image = Image::parse(&bytes).unwrap();
    let config = image.offset_of(image.directories[10].0).unwrap();

    let mut plain = bytes.clone();
    plain[config + 0xe4..config + 0xe6].copy_from_slice(&[0, 0]);
    rejects(&plain, "plain ARM64");

    let mut wrong = bytes.clone();
    let at = bytes
        .windows(14)
        .position(|window| window == b"divvun_tip_x64")
        .unwrap();
    wrong[at..at + 14].copy_from_slice(b"divvun_tip_x86");
    rejects(&wrong, "Arm64EC view");

    let mut x64 = bytes.clone();
    let pe = u32::from_le_bytes(bytes[0x3c..0x40].try_into().unwrap()) as usize;
    x64[pe + 4..pe + 6].copy_from_slice(&0x8664u16.to_le_bytes());
    rejects(&x64, "expected ARM64");

    let mut no_chpe = bytes.clone();
    let ec = verify::ec_view(&bytes).unwrap().unwrap();
    let ec_image = Image::parse(&ec).unwrap();
    let ec_config = ec_image.offset_of(ec_image.directories[10].0).unwrap();
    no_chpe[ec_config + 0xc8..ec_config + 0xd0].copy_from_slice(&[0; 8]);
    rejects(&no_chpe, "no CHPE metadata");

    assert!(verify::verify_forwarder(&bytes[..0x200]).is_err());
}

// [spec:kbdgen:req:tsf.arch.builds+1/test]
#[test]
fn tip_verifier_checks_exports_and_machine() {
    let dir = tempfile::tempdir().unwrap();
    let mut def = String::from("LIBRARY kbd_tsf\nEXPORTS\n");
    for name in EXPORTS {
        def.push_str(&format!("    {name}=_load_config_used\n"));
    }
    std::fs::write(dir.path().join("tip.def"), def).unwrap();
    std::fs::write(
        dir.path().join("cfg.obj"),
        coff::object(coff::MACHINE_ARM64, coff::NATIVE),
    )
    .unwrap();
    let out = dir.path().join("tip.dll");
    let status = Command::new(rust_lld())
        .args(["-flavor", "link", "-dll", "-noentry", "-machine:arm64"])
        .arg(format!("-def:{}", dir.path().join("tip.def").display()))
        .arg(dir.path().join("cfg.obj"))
        .arg(format!("-out:{}", out.display()))
        .status()
        .unwrap();
    assert!(status.success());
    let tip = std::fs::read(&out).unwrap();
    verify::verify_tip(&tip, 0xaa64).unwrap();

    let error = format!("{:#}", verify::verify_tip(&tip, 0x8664).unwrap_err());
    assert!(error.contains("expected 0x8664"), "{error}");
    let forwarder = link_forwarder(&rust_lld()).unwrap();
    let error = format!("{:#}", verify::verify_tip(&forwarder, 0xaa64).unwrap_err());
    assert!(error.contains("forwarders"), "{error}");
    let layout = include_bytes!("../kbdl/testdata/kbdvro-x64.dll");
    let error = format!("{:#}", verify::verify_tip(layout, 0x8664).unwrap_err());
    assert!(error.contains("KbdLayerDescriptor"), "{error}");
}

// [spec:kbdgen:req:tsf.security.signing+1/test]
#[test]
fn release_writes_exactly_the_files_to_sign() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("out");
    let names = [
        "divvun_tip_x86.dll",
        "divvun_tip_x64.dll",
        "divvun_tip_arm64.dll",
        "divvun_tip.dll",
    ];
    let files = |names: &[&str]| -> Vec<(String, Vec<u8>)> {
        names
            .iter()
            .map(|name| ((*name).to_owned(), name.as_bytes().to_vec()))
            .collect()
    };

    let error = write_release(&out, files(&names[..3]))
        .unwrap_err()
        .to_string();
    assert!(error.contains("divvun_tip.dll"), "{error}");
    assert!(!out.exists());

    let paths = write_release(&out, files(&names)).unwrap();
    let out = dunce::canonicalize(&out).unwrap();
    assert_eq!(paths, names.map(|name| out.join(name)));
    assert!(paths.iter().all(|path| path.is_absolute()));
    let mut written: Vec<String> = std::fs::read_dir(&out)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    written.sort();
    let mut expected = names.map(str::to_owned).to_vec();
    expected.sort();
    assert_eq!(written, expected);
    assert_eq!(std::fs::read(&paths[3]).unwrap(), b"divvun_tip.dll");
}
