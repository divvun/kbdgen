//! `kbdgen ldml` as a process: exit status and what it leaves behind.

use std::process::Command;

// [spec:kbdgen:def:ldml.cli.commands/test]
#[test]
fn errors_exit_non_zero_and_write_nothing() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("layouts")).unwrap();
    std::fs::write(
        dir.path().join("layouts/sme.yaml"),
        "displayNames: {sme: Davvisámegiella}\n",
    )
    .unwrap();
    let out = dir.path().join("out");
    for command in ["export", "compile"] {
        let output = Command::new(env!("CARGO_BIN_EXE_kbdgen"))
            .args(["ldml", command, "-b"])
            .arg(dir.path())
            .arg("-o")
            .arg(&out)
            .output()
            .unwrap();
        assert!(!output.status.success(), "{command}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("kbdgen ldml migrate"),
            "{command}: {stderr}"
        );
        assert!(!out.exists(), "{command} wrote output");
    }
}

const VRO: &str = include_str!("../src/ldml/yaml/tests/vro.yaml");

fn kbdgen(args: &[&std::ffi::OsStr]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_kbdgen"))
        .args(args)
        .output()
        .unwrap()
}

fn names(dir: &std::path::Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

// [spec:kbdgen:req:ldml.cli.export/test]
// [spec:kbdgen:req:ldml.cli.compile/test]
// [spec:kbdgen:req:ldml.cli.import/test]
// [spec:kbdgen:thm:ldml.yaml.roundtrip/test]
#[test]
fn v4_bundle_exports_compiles_and_imports_back() {
    let dir = tempfile::tempdir().unwrap();
    let bundle = dir.path().join("vro.kbdgen");
    std::fs::create_dir_all(bundle.join("layouts")).unwrap();
    std::fs::write(bundle.join("layouts/vro.yaml"), VRO).unwrap();
    let xml = dir.path().join("xml");
    let dvkb = dir.path().join("dvkb");
    for (command, out) in [("export", &xml), ("compile", &dvkb)] {
        let output = kbdgen(&[
            "ldml".as_ref(),
            command.as_ref(),
            "-b".as_ref(),
            bundle.as_os_str(),
            "-o".as_ref(),
            out.as_os_str(),
        ]);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let hosts = [
        "android", "chromeOS", "iOS", "linux", "macOS", "web", "windows",
    ];
    assert_eq!(names(&xml), hosts.map(|h| format!("vro.{h}.xml")));
    assert_eq!(names(&dvkb), hosts.map(|h| format!("vro.{h}.dvkb")));
    let read = |d: &std::path::Path, n: &str| std::fs::read(d.join(n)).unwrap();
    assert_eq!(read(&dvkb, "vro.linux.dvkb"), read(&dvkb, "vro.web.dvkb"));
    assert_ne!(read(&dvkb, "vro.linux.dvkb"), read(&dvkb, "vro.macOS.dvkb"));

    let imported = dir.path().join("imported.kbdgen");
    let mut args: Vec<&std::ffi::OsStr> = vec![
        "ldml".as_ref(),
        "import".as_ref(),
        "-b".as_ref(),
        imported.as_os_str(),
    ];
    let files: Vec<std::path::PathBuf> = names(&xml).iter().map(|n| xml.join(n)).collect();
    args.extend(files.iter().map(|f| f.as_os_str()));
    let output = kbdgen(&args);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.ends_with("vro.yaml\n"), "{stdout}");
    let again = dir.path().join("again");
    let output = kbdgen(&[
        "ldml".as_ref(),
        "compile".as_ref(),
        "-b".as_ref(),
        imported.as_os_str(),
        "-o".as_ref(),
        again.as_os_str(),
    ]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    for name in names(&dvkb) {
        assert_eq!(read(&dvkb, &name), read(&again, &name), "{name}");
    }
}

// [spec:kbdgen:req:ldml.yaml.coexistence/test]
#[test]
fn target_generators_fail_on_v4_layouts() {
    let dir = tempfile::tempdir().unwrap();
    let bundle = dir.path().join("vro.kbdgen");
    for sub in ["layouts", "targets", "resources"] {
        std::fs::create_dir_all(bundle.join(sub)).unwrap();
    }
    std::fs::write(
        bundle.join("project.yaml"),
        "locales:\n  en:\n    name: Test\n    description: Test\nauthor: Test\ncopyright: Test\nemail: test@example.com\norganisation: Test\n",
    )
    .unwrap();
    std::fs::write(bundle.join("layouts/vro.yaml"), VRO).unwrap();
    let out = dir.path().join("out");
    for (target, sub, expected) in [
        (
            "macos",
            Some("generate"),
            "the macos target cannot build v4 layouts yet",
        ),
        (
            "ios",
            Some("build"),
            "the ios target cannot build v4 layouts yet",
        ),
        (
            "android",
            Some("generate"),
            "the android target cannot build v4 layouts yet",
        ),
        (
            "chromeos",
            None,
            "the chromeos target cannot build v4 layouts yet",
        ),
        (
            "windows",
            None,
            "the windows target builds v4 layouts through the kbdl adapter",
        ),
    ] {
        let mut args: Vec<&std::ffi::OsStr> = vec![
            "target".as_ref(),
            "-b".as_ref(),
            bundle.as_os_str(),
            "-o".as_ref(),
            out.as_os_str(),
            target.as_ref(),
        ];
        args.extend(sub.iter().map(std::ffi::OsStr::new));
        let output = kbdgen(&args);
        assert!(!output.status.success(), "{target}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("layout vro is a v4 layout"),
            "{target}: {stderr}"
        );
        assert!(stderr.contains(expected), "{target}: {stderr}");
        assert!(!out.exists(), "{target} wrote output");
    }
}
