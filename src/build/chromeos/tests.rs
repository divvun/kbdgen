//! The generated extension: its files, the golden manifest and messages,
//! the errors before anything is written, and what is reported.

use std::fs;
use std::path::{Path, PathBuf};

use kbd_engine::Model;
use kbd_model::Host;

use super::classify::{self, REACHABLE_SCAN_CODES};
use super::generate_chromeos::{ENGINE_WASM, SERVICE_WORKER_JS};
use super::*;
use crate::build::windows::kbdl::diag::Diagnostics;
use crate::bundle::fixture::{write_bundle, write_file};
use crate::bundle::read_kbdgen_bundle;

pub const WORKSPACE: &str = env!("CARGO_MANIFEST_DIR");
pub const TARGET: &str = "appId: abc\nbuild: \"7\"\nversion: 2.0.0\n";
const GOLDEN: &str = "src/build/chromeos/testdata/golden";

pub fn fixture_text(relative: &str) -> String {
    let path = Path::new(WORKSPACE).join(relative);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// A v3 layout without a `chromeOS` section, which has no output.
const FI: &str = "displayNames:\n  fi: Suomi\n  en: Finnish\n";

/// A v3 `chromeOS` layout whose `shift` layer has 47 tokens.
fn short_layer() -> String {
    let keys = "a b c d e f g h i j k l m n o p q r s t u v w x y z 1 2 3 4 5 6 7 8 9 0 A B C D E F G H I J K L";
    let short = keys.rsplit_once(' ').unwrap().0;
    format!(
        "displayNames:\n  smj: Julev\nchromeOS:\n  primary:\n    layers:\n      default: {keys}\n      shift: {short}\n"
    )
}

/// The golden bundle: a v3 layout with a `chromeOS` section, two v4
/// layouts with a `default` variant, and a v3 layout without one.
fn golden_layouts() -> Vec<(&'static str, String)> {
    vec![
        ("fi", FI.to_owned()),
        (
            "se-NO",
            fixture_text("crates/kbd-engine/tests/golden/v3/se-NO.yaml"),
        ),
        (
            "und-x-mods",
            fixture_text("crates/kbd-engine/tests/golden/layouts/und-x-mods.yaml"),
        ),
        (
            "vro",
            fixture_text("crates/kbd-engine/tests/golden/layouts/vro.yaml"),
        ),
    ]
}

pub struct Generated {
    _dir: tempfile::TempDir,
    pub bundle: PathBuf,
    pub out: PathBuf,
}

pub async fn generate(
    layouts: &[(&str, &str)],
    targets: &[(&str, &str)],
    resources: &[(&str, &[u8])],
) -> anyhow::Result<Generated> {
    let dir = tempfile::tempdir().unwrap();
    let bundle = write_bundle(dir.path(), "test", layouts, targets, &[]);
    for (relative, bytes) in resources {
        let path = bundle.join("resources").join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }
    let loaded = read_kbdgen_bundle(&bundle).unwrap();
    let out = dir.path().join("out");
    fs::create_dir_all(&out).unwrap();
    crate::build::BuildStep::build(&GenerateChromeOs, &loaded, &out).await?;
    Ok(Generated {
        _dir: dir,
        bundle,
        out,
    })
}

fn files(dir: &Path) -> Vec<String> {
    let mut files = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(next) = stack.pop() {
        for entry in fs::read_dir(&next).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else {
                let relative = path.strip_prefix(dir).unwrap();
                files.push(relative.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    files.sort();
    files
}

// [spec:kbdgen:req:ldml.chromeos.test.golden]
/// Compares `actual` with the golden file `relative`, first rewriting it
/// when `KBDGEN_BLESS` is set.
fn golden(relative: &str, actual: &str) {
    let path = Path::new(WORKSPACE).join(GOLDEN).join(relative);
    if std::env::var_os("KBDGEN_BLESS").is_some() {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, actual).unwrap();
    }
    let expected = fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "{}: {e}; run with KBDGEN_BLESS=1 to create it",
            path.display()
        )
    });
    assert_eq!(
        expected,
        actual,
        "{} differs; rerun with KBDGEN_BLESS=1 to update it",
        path.display()
    );
}

/// What each layout's keyboard reports (`ldml.chromeos.classify`).
fn diagnostics(out: &Path) -> String {
    let mut text = String::new();
    for name in files(&out.join("models")) {
        let bytes = fs::read(out.join("models").join(&name)).unwrap();
        let model = Model::from_bytes(&bytes).unwrap();
        let mut diag = Diagnostics::new(name.trim_end_matches(".dvkb"));
        classify::report(&classify::classify(model.keyboard()), &mut diag);
        for warning in diag.warnings() {
            text.push_str(&format!("{}: {warning}\n", diag.layout()));
        }
    }
    text
}

// [spec:kbdgen:def:chromeos.extension/test]
// [spec:kbdgen:req:chromeos.manifest+2/test]
// [spec:kbdgen:req:chromeos.locales/test]
// [spec:kbdgen:req:chromeos.engine/test]
// [spec:kbdgen:def:ldml.chromeos.input/test]
// [spec:kbdgen:req:ldml.chromeos.target/test]
// [spec:kbdgen:req:ldml.chromeos.test.golden/test]
// [spec:kbdgen:req:ldml.yaml.coexistence+2/test]
#[tokio::test]
async fn writes_the_extension_with_golden_manifest_and_messages() {
    let layouts = golden_layouts();
    let layouts: Vec<(&str, &str)> = layouts.iter().map(|(t, y)| (*t, y.as_str())).collect();
    let generated = generate(&layouts, &[("chromeos", TARGET)], &[])
        .await
        .unwrap();
    let out = &generated.out;

    let names = files(out);
    let locales: Vec<&String> = names
        .iter()
        .filter(|n| n.starts_with("_locales/"))
        .collect();
    assert_eq!(
        names
            .iter()
            .filter(|n| !n.starts_with("_locales/"))
            .collect::<Vec<_>>(),
        [
            "background.js",
            "engine.wasm",
            "manifest.json",
            "models/se-NO.dvkb",
            "models/und-x-mods.dvkb",
            "models/vro.dvkb",
        ]
    );
    assert_eq!(
        fs::read_to_string(out.join("background.js")).unwrap(),
        SERVICE_WORKER_JS
    );
    assert_eq!(fs::read(out.join("engine.wasm")).unwrap(), ENGINE_WASM);
    assert!(ENGINE_WASM.starts_with(b"\0asm"));

    let bundle = read_kbdgen_bundle(&generated.bundle).unwrap();
    let compiled = source::layouts(&bundle).unwrap();
    for layout in &compiled {
        let bytes = fs::read(out.join(format!("models/{}.dvkb", layout.tag))).unwrap();
        assert_eq!(bytes, layout.model, "{}", layout.tag);
        let model = Model::from_bytes(&bytes).unwrap();
        assert_eq!(
            model.keyboard().host,
            Some(Host::ChromeOs),
            "{}",
            layout.tag
        );
    }

    golden(
        "manifest.json",
        &fs::read_to_string(out.join("manifest.json")).unwrap(),
    );
    for name in &locales {
        golden(name, &fs::read_to_string(out.join(name.as_str())).unwrap());
    }
    let golden_locales = files(&Path::new(WORKSPACE).join(GOLDEN).join("_locales"));
    let written: Vec<String> = locales
        .iter()
        .map(|n| n.trim_start_matches("_locales/").to_owned())
        .collect();
    assert_eq!(written, golden_locales, "no golden locale is left over");
    golden("diagnostics.txt", &diagnostics(out));
}

// [spec:kbdgen:req:chromeos.icons+1/test]
#[tokio::test]
async fn copies_the_bundle_icon_and_references_it() {
    let vro = fixture_text("crates/kbd-engine/tests/golden/layouts/vro.yaml");
    let png = b"\x89PNG\r\n\x1a\nnot really";
    let generated = generate(
        &[("vro", &vro)],
        &[("chromeos", TARGET)],
        &[("chromeos/icon.png", png), ("chromeos/icon.vro.png", b"x")],
    )
    .await
    .unwrap();
    assert_eq!(fs::read(generated.out.join("icon.png")).unwrap(), png);
    let manifest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(generated.out.join("manifest.json")).unwrap())
            .unwrap();
    assert_eq!(
        manifest["icons"],
        serde_json::json!({"16": "icon.png", "48": "icon.png", "128": "icon.png"})
    );

    let again = generate(&[("vro", &vro)], &[("chromeos", TARGET)], &[])
        .await
        .unwrap();
    assert!(!again.out.join("icon.png").exists());
}

// [spec:kbdgen:req:chromeos.icons+1/test]
#[tokio::test]
async fn a_default_icon_that_is_not_png_is_an_error() {
    let vro = fixture_text("crates/kbd-engine/tests/golden/layouts/vro.yaml");
    let error = generate(
        &[("vro", &vro)],
        &[("chromeos", TARGET)],
        &[("chrome/icon.svg", b"<svg/>")],
    )
    .await
    .err()
    .expect("a non-PNG icon fails generation");
    assert!(
        error
            .to_string()
            .ends_with("chrome/icon.svg: a ChromeOS extension icon must be a PNG"),
        "{error}"
    );
}

// [spec:kbdgen:def:chromeos.extension/test]
#[tokio::test]
async fn regeneration_removes_stale_models_and_locales() {
    let vro = fixture_text("crates/kbd-engine/tests/golden/layouts/vro.yaml");
    let generated = generate(&[("vro", &vro)], &[("chromeos", TARGET)], &[])
        .await
        .unwrap();
    write_file(&generated.out, "models/gone.dvkb", "");
    write_file(&generated.out, "_locales/gone/messages.json", "{}");
    write_file(&generated.out, "icon.png", "");
    let bundle = read_kbdgen_bundle(&generated.bundle).unwrap();
    crate::build::BuildStep::build(&GenerateChromeOs, &bundle, &generated.out)
        .await
        .unwrap();
    let names = files(&generated.out);
    assert!(!names.iter().any(|n| n.contains("gone")), "{names:?}");
    assert!(!names.contains(&"icon.png".to_owned()));
}

/// Layouts and targets, each as (stem, text), and the error they give.
type ErrorCase<'a> = (&'a [(&'a str, &'a str)], &'a [(&'a str, &'a str)], &'a str);

// [spec:kbdgen:req:chromeos.manifest+2/test]
// [spec:kbdgen:def:ldml.chromeos.input/test]
// [spec:kbdgen:def:chromeos.extension/test]
#[tokio::test]
async fn errors_come_before_any_file_is_written() {
    let vro = fixture_text("crates/kbd-engine/tests/golden/layouts/vro.yaml");
    let cases: [ErrorCase; 5] = [
        (&[("vro", &vro)], &[], "targets/chromeos.yaml is missing"),
        (
            &[("fi", FI)],
            &[("chromeos", TARGET)],
            "no layout has a chromeOS keyboard",
        ),
        (
            &[("smj", &short_layer())],
            &[("chromeos", TARGET)],
            "smj: the v3 layout's migration is blocked by defects M05; `kbdgen ldml migrate --dry-run` reports them",
        ),
        (
            &[("vro", &vro)],
            &[("chromeos", "appId: abc\nbuild: \"1.02\"\nversion: 2.0.0\n")],
            "targets/chromeos.yaml: build \"1.02\" is not a Chrome extension version: one to four dot-separated integers from 0 to 65535, without leading zeros",
        ),
        (
            &[(
                "und",
                "format: 4\ndisplayNames: {und: Touch}\ntouch:\n  default:\n    sizes:\n      phone:\n        layers:\n          base: a b c\n",
            )],
            &[("chromeos", TARGET)],
            "no layout has a chromeOS keyboard",
        ),
    ];
    for (layouts, targets, message) in cases {
        let dir = tempfile::tempdir().unwrap();
        let bundle = write_bundle(dir.path(), "test", layouts, targets, &[]);
        let loaded = read_kbdgen_bundle(&bundle).unwrap();
        let out = dir.path().join("out");
        fs::create_dir_all(&out).unwrap();
        let error = crate::build::BuildStep::build(&GenerateChromeOs, &loaded, &out)
            .await
            .unwrap_err();
        assert_eq!(format!("{error:#}"), message);
        assert!(files(&out).is_empty(), "{message}");
    }

    let dir = tempfile::tempdir().unwrap();
    let bundle = write_bundle(
        dir.path(),
        "test",
        &[("vro", &vro)],
        &[("chromeos", TARGET)],
        &[],
    );
    fs::write(
        bundle.join("project.yaml"),
        "locales:\n  nb:\n    name: Test\n    description: Test\nauthor: Test\ncopyright: Test\nemail: test@example.com\norganisation: Test\n",
    )
    .unwrap();
    let loaded = read_kbdgen_bundle(&bundle).unwrap();
    let error = crate::build::BuildStep::build(&GenerateChromeOs, &loaded, dir.path())
        .await
        .unwrap_err();
    assert_eq!(error.to_string(), "project.yaml: locales has no en entry");
}

// [spec:kbdgen:sem:ldml.chromeos.keys/test]
#[test]
fn reachable_scan_codes_are_the_service_worker_table() {
    let start = SERVICE_WORKER_JS.find("const SCAN_CODES").unwrap();
    let end = start + SERVICE_WORKER_JS[start..].find("]);").unwrap();
    let codes: Vec<u8> = SERVICE_WORKER_JS[start..end]
        .lines()
        .filter_map(|line| {
            let hex = line.trim().strip_prefix('[')?.split("0x").nth(1)?;
            u8::from_str_radix(hex.trim_end_matches("],"), 16).ok()
        })
        .collect();
    assert_eq!(codes, REACHABLE_SCAN_CODES);
}

// [spec:kbdgen:req:ldml.chromeos.classify/test]
#[tokio::test]
async fn reports_what_chromeos_cannot_type() {
    let features = fixture_text("src/build/macos/testdata/features.yaml")
        .replace("hardware:\n  macOS:", "hardware:\n  chromeOS:");
    let custom = "format: 4\ndisplayNames: {und: Custom}\nhardware:\n  chromeOS:\n    form:\n      id: custom\n      rows: ['1E 5B']\n    layers:\n      none: a b\n";
    let generated = generate(
        &[("sme", &features), ("und", custom)],
        &[("chromeos", TARGET)],
        &[],
    )
    .await
    .unwrap();
    let report = diagnostics(&generated.out);
    let lines: Vec<&str> = report.lines().collect();
    assert_eq!(
        lines,
        [
            "sme: ChromeOS cannot use 2 modifier set(s) needing Left Alt, because Left Alt is ChromeOS's shortcut modifier and only Right Alt types (e.g. altL; altL caps)",
            "sme: ChromeOS cannot use 1 key(s) with long-press, multi-tap or flick gestures, because hardware keys have no gestures (e.g. u-0061)",
            "sme: ChromeOS cannot use 2 display(s) and key label(s), because the input method draws no keyboard (e.g. Å; ¨)",
            "und: ChromeOS cannot use 1 key position(s), because no ChromeOS key event carries their scan codes (e.g. scan code 5B)",
        ]
    );
}
