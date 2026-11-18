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
// [spec:kbdgen:thm:ldml.yaml.roundtrip+2/test]
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

// [spec:kbdgen:req:ldml.yaml.coexistence+2/test]
// [spec:kbdgen:def:ldml.kbdl.adapter/test]
// [spec:kbdgen:req:ldml.macos.target/test]
// [spec:kbdgen:req:ldml.chromeos.target/test]
#[test]
fn windows_macos_and_chromeos_targets_build_v4_layouts() {
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
            "ios",
            Some("build"),
            "the ios target cannot build v4 layouts yet",
        ),
        (
            "android",
            Some("generate"),
            "the android target cannot build v4 layouts yet",
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
    std::fs::create_dir_all(bundle.join("resources/macos")).unwrap();
    std::fs::write(
        bundle.join("targets/macos.yaml"),
        "codeSignId: X\npackageId: com.example\nbundleName: Võro\nversion: 1.0.0\nbuild: \"1\"\n",
    )
    .unwrap();
    let mac = dir.path().join("mac");
    let output = kbdgen(&[
        "target".as_ref(),
        "-b".as_ref(),
        bundle.as_os_str(),
        "-o".as_ref(),
        mac.as_os_str(),
        "macos".as_ref(),
        "generate".as_ref(),
    ]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stderr}");
    let contents = mac.join("com.example.keyboardlayout.vro.bundle/Contents");
    let keylayout = std::fs::read_to_string(contents.join("Resources/vro.keylayout")).unwrap();
    assert!(
        keylayout.contains("<keyboard group=\"126\" id=\"-19693\" name=\"vro\">"),
        "{keylayout}"
    );
    assert!(keylayout.contains("<terminators>"), "{keylayout}");
    let plist = std::fs::read_to_string(contents.join("Info.plist")).unwrap();
    assert!(plist.contains("<key>KLInfo_vro</key>"), "{plist}");
    let strings =
        std::fs::read_to_string(contents.join("Resources/vro.lproj/InfoPlist.strings")).unwrap();
    assert_eq!(strings, "\"vro\" = \"Võro\";");

    std::fs::write(
        bundle.join("targets/chromeos.yaml"),
        "appId: abc\nbuild: \"1\"\nversion: 1.0.0\n",
    )
    .unwrap();
    let chromeos = dir.path().join("chromeos");
    let output = kbdgen(&[
        "target".as_ref(),
        "-b".as_ref(),
        bundle.as_os_str(),
        "-o".as_ref(),
        chromeos.as_os_str(),
        "chromeos".as_ref(),
    ]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stderr}");
    for file in [
        "manifest.json",
        "background.js",
        "engine.wasm",
        "models/vro.dvkb",
    ] {
        assert!(chromeos.join(file).is_file(), "{file}: {stderr}");
    }
    let manifest = std::fs::read_to_string(chromeos.join("manifest.json")).unwrap();
    assert!(manifest.contains("\"id\": \"vro\""), "{manifest}");
}

// [spec:kbdgen:req:ldml.yaml.strict+1/test]
#[test]
fn every_command_logs_format_4_load_warnings_once() {
    let dir = tempfile::tempdir().unwrap();
    let bundle = dir.path().join("vro.kbdgen");
    for sub in ["layouts", "targets", "resources/macos", "tests"] {
        std::fs::create_dir_all(bundle.join(sub)).unwrap();
    }
    let write = |path: &str, text: &str| std::fs::write(bundle.join(path), text).unwrap();
    write(
        "project.yaml",
        "locales:\n  en:\n    name: Test\n    description: Test\nauthor: Test\ncopyright: Test\nemail: test@example.com\norganisation: Test\n",
    );
    write(
        "layouts/vro.yaml",
        &VRO.replacen("deadKeys:\n", "deadKeys:\n  ¨: {compose: {a: ä}}\n", 1),
    );
    write(
        "targets/macos.yaml",
        "codeSignId: X\npackageId: com.example\nbundleName: Võro\nversion: 1.0.0\nbuild: \"1\"\n",
    );
    write(
        "targets/chromeos.yaml",
        "appId: abc\nbuild: \"1\"\nversion: 1.0.0\n",
    );
    let vectors = "layout: layouts/vro.yaml\nhost: macOS\ntests:\n  - name: acute\n    steps:\n      - press: {key: E12}\n      - press: {key: C01}\n      - expect: {text: á}\n";
    write("tests/first.yaml", vectors);
    write("tests/second.yaml", vectors);

    let out = |name: &str| dir.path().join(name).into_os_string();
    let ldml = |command: &str, output: Option<std::ffi::OsString>| {
        let mut args = vec![
            "ldml".into(),
            command.into(),
            "-b".into(),
            bundle.clone().into(),
        ];
        if let Some(output) = output {
            args.extend(["-o".into(), output]);
        }
        args
    };
    let target = |target: &str, sub: Option<&str>| {
        let mut args: Vec<std::ffi::OsString> = vec![
            "target".into(),
            "-b".into(),
            bundle.clone().into(),
            "-o".into(),
            out(target),
            target.into(),
        ];
        args.extend(sub.map(Into::into));
        args
    };
    let warning =
        "layouts/vro.yaml: deadKeys.¨: no \\d{¨} token uses this dead key, so no host gets it";
    // The Windows target loads the layout in both of its steps and `ldml
    // test` once per vector file, so each would log the warning twice
    // without deduplication.
    for (args, own) in [
        (ldml("compile", Some(out("compile"))), None),
        (ldml("export", Some(out("export"))), None),
        (ldml("test", None), None),
        (
            target("windows", None),
            Some(" WARN kbdgen::build::windows: kbdvro: "),
        ),
        (
            target("macos", Some("generate")),
            Some(" WARN kbdgen::build::macos: vro: the .keylayout cannot express "),
        ),
        (
            target("chromeos", None),
            Some(" WARN kbdgen::build::chromeos: vro: ChromeOS cannot use "),
        ),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_kbdgen"))
            .args(&args)
            .env("NO_COLOR", "1")
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        let lines: Vec<&str> = stderr.lines().filter(|l| l.contains(warning)).collect();
        assert_eq!(lines.len(), 1, "{args:?}: {stderr}");
        assert!(
            lines[0].contains(" WARN kbdgen::ldml::yaml: "),
            "{args:?}: {stderr}"
        );
        if let Some(own) = own {
            assert!(stderr.contains(own), "{args:?}: {stderr}");
        }
        assert!(!stderr.contains("kbdl::diag"), "{args:?}: {stderr}");
    }
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

// [spec:kbdgen:req:tsf.security.signing+2/test]
#[test]
fn logs_go_to_stderr_not_stdout() {
    let dir = tempfile::tempdir().unwrap();
    let bundle = vro_test_bundle(
        dir.path(),
        "      - press: {key: E12}\n      - press: {key: C01}\n      - expect: {text: á}\n",
    );
    let output = Command::new(env!("CARGO_BIN_EXE_kbdgen"))
        .args([
            "ldml".as_ref(),
            "test".as_ref(),
            "-b".as_ref(),
            bundle.as_os_str(),
        ])
        .env("RUST_LOG", "info")
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("kbdgen "), "{stderr}");
    assert_eq!(
        report(&output),
        "files: 1, tests: 1, checks: 1, failing steps: 0\n"
    );
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
