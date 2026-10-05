//! `kbdgen ldml` as a process: exit status and what it leaves behind.

use std::process::Command;

// [spec:kbdgen:def:ldml.cli.commands+1/test]
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

const VRO: &str = include_str!("../crates/kbd-engine/tests/golden/layouts/vro.yaml");

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
// [spec:kbdgen:req:ldml.cli.compile+1/test]
// [spec:kbdgen:req:ldml.cli.import/test]
// [spec:kbdgen:thm:ldml.yaml.roundtrip+1/test]
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
// [spec:kbdgen:def:ldml.kbdl.adapter/test]
#[test]
fn only_the_windows_target_builds_v4_layouts() {
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
    let output = kbdgen(&[
        "target".as_ref(),
        "-b".as_ref(),
        bundle.as_os_str(),
        "-o".as_ref(),
        out.as_os_str(),
        "windows".as_ref(),
    ]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.contains("is a v4 layout"), "{stderr}");
    assert!(
        out.join("build/kbdvro/lib.rs").is_file(),
        "the windows target generates the v4 layout's crate: {stderr}"
    );
}

/// Standard output without the log lines `tracing` writes there.
fn report(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|l| !l.contains(" INFO"))
        .map(|l| format!("{l}\n"))
        .collect()
}

/// A bundle holding the Võro layout and `tests/vro.yaml` with `steps`.
fn vro_test_bundle(dir: &std::path::Path, steps: &str) -> std::path::PathBuf {
    let bundle = dir.join("vro.kbdgen");
    std::fs::create_dir_all(bundle.join("layouts")).unwrap();
    std::fs::create_dir_all(bundle.join("tests")).unwrap();
    std::fs::write(bundle.join("layouts/vro.yaml"), VRO).unwrap();
    std::fs::write(
        bundle.join("tests/vro.yaml"),
        format!(
            "layout: layouts/vro.yaml\nhost: macOS\ntests:\n  - name: acute\n    steps:\n{steps}"
        ),
    )
    .unwrap();
    bundle
}

// [spec:kbdgen:req:ldml.cli.test/test]
// [spec:kbdgen:req:ldml.test.bundle+1/test]
#[test]
fn ldml_test_passes_on_matching_vectors() {
    let dir = tempfile::tempdir().unwrap();
    let bundle = vro_test_bundle(
        dir.path(),
        "      - press: {key: E12}\n      - press: {key: C01}\n      - expect: {text: á}\n",
    );
    let output = kbdgen(&[
        "ldml".as_ref(),
        "test".as_ref(),
        "-b".as_ref(),
        bundle.as_os_str(),
    ]);
    let stdout = report(&output);
    assert!(
        output.status.success(),
        "{stdout}{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(stdout, "files: 1, tests: 1, checks: 1, failing steps: 0\n");
}

// [spec:kbdgen:req:ldml.cli.test/test]
#[test]
fn ldml_test_fails_with_a_diff() {
    let dir = tempfile::tempdir().unwrap();
    let bundle = vro_test_bundle(
        dir.path(),
        "      - press: {key: E12}\n      - expect: {text: ´, preedit: ´}\n      - press: {key: C07}\n      - expect: {text: j}\n",
    );
    let output = kbdgen(&[
        "ldml".as_ref(),
        "test".as_ref(),
        "-b".as_ref(),
        bundle.as_os_str(),
    ]);
    assert!(!output.status.success());
    let stdout = report(&output);
    assert_eq!(
        stdout,
        "FAIL tests/vro.yaml: acute: step 2: expect\n  \
         expected text     \"´\"\n  \
         expected preedit  \"´\"\n  \
         actual   text     \"\"\n  \
         actual   preedit  \"´\"\n  \
         actual   layer    none\n  \
         actual   action   edit: delete 0, insert \"\", preedit \"´\", layer none\n\
         FAIL tests/vro.yaml: acute: step 4: expect\n  \
         expected text     \"j\"\n  \
         actual   text     \"´j\"\n  \
         actual   preedit  \"\"\n  \
         actual   layer    none\n  \
         actual   action   edit: delete 0, insert \"´j\", preedit \"\", layer none\n\
         files: 1, tests: 1, checks: 2, failing steps: 2\n"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("2 conformance steps failed"), "{stderr}");
}

// [spec:kbdgen:req:ldml.cli.test/test]
// [spec:kbdgen:req:ldml.test.golden/test]
#[test]
fn ldml_test_runs_golden_and_cldr_vectors() {
    let golden =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("crates/kbd-engine/tests/golden");
    let output = kbdgen(&[
        "ldml".as_ref(),
        "test".as_ref(),
        "-b".as_ref(),
        golden.as_os_str(),
        "--cldr".as_ref(),
    ]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "{stdout}{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.contains("note: cldr/48/keyboards/test/pcm-test.xml: repertoire"),
        "{stdout}"
    );
    assert!(stdout.ends_with(", failing steps: 0\n"), "{stdout}");
    let broken = tempfile::tempdir().unwrap();
    let bundle = vro_test_bundle(broken.path(), "      - shout\n");
    let output = kbdgen(&[
        "ldml".as_ref(),
        "test".as_ref(),
        "-b".as_ref(),
        bundle.as_os_str(),
    ]);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("tests/vro.yaml: tests[1].steps[1]: shout is not a step"),
        "{stderr}"
    );
}

const SE_V3: &str = "displayNames:\n  se: Davvisámegiella\nwindows:\n  primary:\n    layers:\n      default: a b c d e f g h i j k l m n o p q r s t u v w x y z 1 2 3 4 5 6 7 8 9 0 + , . - ' ¨ < ´ § ½ å æ\n      shift: A B C D E F G H I J K L M N O P Q R S T U V W X Y Z ! \" @ ¤ % & / ( ) = ? ; | _ * ^ > ` ° ¶ Å Æ\n";

// [spec:kbdgen:def:ldml.cli.commands+1/test]
// [spec:kbdgen:def:ldml.migrate.report+1/test]
#[test]
fn migrate_writes_unblocked_layouts_and_reports() {
    let dir = tempfile::tempdir().unwrap();
    let bundle = dir.path().join("se.kbdgen");
    std::fs::create_dir_all(bundle.join("layouts")).unwrap();
    std::fs::write(bundle.join("layouts/se.yaml"), SE_V3).unwrap();
    let migrate = |extra: &[&std::ffi::OsStr]| {
        let mut args: Vec<&std::ffi::OsStr> = vec![
            "ldml".as_ref(),
            "migrate".as_ref(),
            "-b".as_ref(),
            bundle.as_os_str(),
        ];
        args.extend_from_slice(extra);
        kbdgen(&args)
    };
    let output = migrate(&["--dry-run".as_ref()]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(report(&output).contains("se.yaml: migrated (dry run)"));
    assert_eq!(
        std::fs::read_to_string(bundle.join("layouts/se.yaml")).unwrap(),
        SE_V3
    );

    std::fs::write(
        bundle.join("layouts/fi.yaml"),
        "displayNames:\n  fi: suomi\nmodes: {}\n",
    )
    .unwrap();
    let path = dir.path().join("report.yaml");
    let output = migrate(&["--report".as_ref(), path.as_os_str()]);
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("migration blocked for") && stderr.contains("fi.yaml"),
        "{stderr}"
    );
    let written = std::fs::read_to_string(bundle.join("layouts/se.yaml")).unwrap();
    assert!(written.starts_with("format: 4\n"), "{written}");
    let blocked = std::fs::read_to_string(bundle.join("layouts/fi.yaml")).unwrap();
    assert!(blocked.contains("modes"));
    let yaml: serde_yaml::Value =
        serde_yaml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(yaml["summary"]["counts"]["M07"].as_u64(), Some(1));
    assert!(report(&output).starts_with("summary: 2 layout(s)"));
}
