//! The bundle's Linux names, the layouts selected and compiled for Linux,
//! and the errors the step raises before it writes anything.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use kbd_engine::Model;
use kbd_model::{Host, Keyboard};
use language_tags::LanguageTag;

use super::GenerateLinux;
use super::ids::Ids;
use super::source::{self, LinuxLayout};
use crate::build::BuildStep;
use crate::bundle::fixture::{write_bundle, write_file};
use crate::bundle::read_kbdgen_bundle;
use crate::bundle::target::Linux;
use crate::ldml::migrate::migrate_bundle_layout;

const WORKSPACE: &str = env!("CARGO_MANIFEST_DIR");
const TARGET: &str = "packageId: no.divvun\nlayoutPrefix: divvun\nversion: 1.0\n";

fn fixture_text(relative: &str) -> String {
    let path = Path::new(WORKSPACE).join(relative);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn tag(tag: &str) -> LanguageTag {
    tag.parse().unwrap()
}

fn target(package_id: &str, layout_prefix: &str) -> Linux {
    Linux {
        package_id: package_id.to_owned(),
        layout_prefix: layout_prefix.to_owned(),
        version: "1.0".to_owned(),
    }
}

/// A v3 layout with neither a `windows` nor a `chromeOS` section.
const FI: &str = "displayNames:\n  fi: Suomi\n  en: Finnish\n";

/// The Windows feature fixture, its variant renamed `linux`.
fn linux_features() -> String {
    fixture_text("src/build/windows/kbdl/testdata/features.yaml")
        .replace("hardware:\n  windows:", "hardware:\n  linux:")
}

/// A v4 layout with a `linux` variant, one with only a `macOS` variant, a
/// v3 layout with a `windows` section among others, one with only a
/// `chromeOS` section, and one with neither.
fn selection_layouts() -> Vec<(&'static str, String)> {
    vec![
        ("fi", FI.to_owned()),
        (
            "se-NO",
            fixture_text("crates/kbd-engine/tests/golden/v3/se-NO.yaml"),
        ),
        ("sme", linux_features()),
        (
            "sme-SE",
            fixture_text("src/build/macos/testdata/features.yaml"),
        ),
        (
            "vro",
            fixture_text("src/build/chromeos/testdata/vro-v3.yaml"),
        ),
    ]
}

struct Fixture {
    _dir: tempfile::TempDir,
    bundle: PathBuf,
    out: PathBuf,
}

fn fixture(layouts: &[(&str, &str)], targets: &[(&str, &str)]) -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let bundle = write_bundle(dir.path(), "test", layouts, targets, &[]);
    let out = dir.path().join("out");
    fs::create_dir_all(&out).unwrap();
    Fixture {
        _dir: dir,
        bundle,
        out,
    }
}

async fn generate(fixture: &Fixture) -> anyhow::Result<()> {
    let bundle = read_kbdgen_bundle(&fixture.bundle).unwrap();
    GenerateLinux.build(&bundle, &fixture.out).await
}

#[derive(Clone, Default)]
struct Captured(Arc<Mutex<Vec<u8>>>);

impl Write for Captured {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// What `run` logs at debug level and above, without time and colour.
fn logged<T>(run: impl FnOnce() -> T) -> (T, String) {
    let captured = Captured::default();
    let writer = captured.clone();
    let subscriber = tracing_subscriber::fmt()
        .with_writer(move || writer.clone())
        .with_max_level(tracing::Level::DEBUG)
        .with_ansi(false)
        .without_time()
        .finish();
    let value = tracing::subscriber::with_default(subscriber, run);
    let bytes = captured.0.lock().unwrap().clone();
    (value, String::from_utf8(bytes).unwrap())
}

/// The keyboard `host` has in the in-memory migration of the v3 layout
/// `tag`, as the `linux` host would have it.
fn migrated_keyboard(bundle: &Path, tag: &str, host: Host) -> Keyboard {
    let layout = migrate_bundle_layout(bundle, tag)
        .unwrap()
        .unwrap()
        .compiled()
        .unwrap()
        .unwrap();
    let mut keyboard = layout.keyboard_for(host).unwrap().clone();
    keyboard.host = Some(Host::Linux);
    keyboard
}

// [spec:kbdgen:def:linux.ids/test]
#[test]
fn ids_take_only_the_documented_shapes() {
    for package_id in ["no.divvun", "no.uit.giella-keyboards", "org.example2.kb"] {
        Ids::new(&target(package_id, "divvun")).unwrap();
    }
    for package_id in [
        "",
        "divvun",
        "No.divvun",
        "no..divvun",
        ".no",
        "no.",
        "no.div_vun",
        "no.divvün",
        "no divvun",
    ] {
        let error = Ids::new(&target(package_id, "divvun"))
            .unwrap_err()
            .to_string();
        assert_eq!(
            error,
            format!(
                "targets/linux.yaml: packageId {package_id:?} is not a reverse-DNS name: two or more dot-separated labels of lowercase ASCII letters, digits and '-'"
            )
        );
    }
    for prefix in ["divvun", "a", "giella2", "abcdefghijklmnop"] {
        Ids::new(&target("no.divvun", prefix)).unwrap();
    }
    for prefix in [
        "",
        "Divvun",
        "2giella",
        "giella-kb",
        "giella.kb",
        "divvün",
        "abcdefghijklmnopq",
    ] {
        let error = Ids::new(&target("no.divvun", prefix))
            .unwrap_err()
            .to_string();
        assert_eq!(
            error,
            format!(
                "targets/linux.yaml: layoutPrefix {prefix:?} is not a lowercase ASCII letter followed by at most 15 lowercase ASCII letters and digits"
            )
        );
    }
}

// [spec:kbdgen:def:linux.ids/test]
#[test]
fn names_derive_from_package_id_and_prefix() {
    let ids = Ids::new(&target("no.divvun.keyboards", "divvun")).unwrap();
    assert_eq!(ids.layout_id(&tag("se-FI")), "divvun-se-fi");
    assert_eq!(ids.layout_id(&tag("se-fi")), "divvun-se-fi");
    assert_eq!(ids.layout_id(&tag("sma")), "divvun-sma");
    assert_eq!(ids.layout_id(&tag("und-x-mods")), "divvun-und-x-mods");
    assert_eq!(ids.extension_uuid(), "osk@no.divvun.keyboards");
    assert_eq!(ids.kpackage_id(&tag("se-FI")), "no.divvun.keyboards.se_FI");
    assert_eq!(ids.kpackage_id(&tag("sma")), "no.divvun.keyboards.sma");
    assert_eq!(ids.package_dir(), "no.divvun.keyboards");
}

/// Layouts and targets, each as (stem, text), and the error they give.
type ErrorCase<'a> = (&'a [(&'a str, &'a str)], &'a [(&'a str, &'a str)], &'a str);

// [spec:kbdgen:def:linux.ids/test]
// [spec:kbdgen:def:ldml.linux.input/test]
#[tokio::test]
async fn errors_come_before_any_file_is_written() {
    let vro = fixture_text("crates/kbd-engine/tests/golden/layouts/vro.yaml");
    let mac = fixture_text("src/build/macos/testdata/features.yaml");
    let cases: [ErrorCase; 4] = [
        (&[("vro", &vro)], &[], "targets/linux.yaml is missing"),
        (
            &[("vro", &vro)],
            &[(
                "linux",
                "packageId: no.divvun\nlayoutPrefix: Divvun\nversion: 1.0\n",
            )],
            "targets/linux.yaml: layoutPrefix \"Divvun\" is not a lowercase ASCII letter followed by at most 15 lowercase ASCII letters and digits",
        ),
        (
            &[("vro", &vro)],
            &[(
                "linux",
                "packageId: divvun\nlayoutPrefix: divvun\nversion: 1.0\n",
            )],
            "targets/linux.yaml: packageId \"divvun\" is not a reverse-DNS name: two or more dot-separated labels of lowercase ASCII letters, digits and '-'",
        ),
        (
            &[("fi", FI), ("sme", &mac)],
            &[("linux", TARGET)],
            "no layout has a linux keyboard",
        ),
    ];
    for (layouts, targets, message) in cases {
        let fixture = fixture(layouts, targets);
        let error = generate(&fixture).await.unwrap_err();
        assert_eq!(format!("{error:#}"), message);
        assert_eq!(fs::read_dir(&fixture.out).unwrap().count(), 0, "{message}");
    }
}

// [spec:kbdgen:def:ldml.linux.input/test]
// [spec:kbdgen:def:ldml.linux.adapter/test]
#[test]
fn selects_v4_linux_and_v3_windows_or_chromeos() {
    let layouts = selection_layouts();
    let layouts: Vec<(&str, &str)> = layouts.iter().map(|(t, y)| (*t, y.as_str())).collect();
    let fixture = fixture(&layouts, &[("linux", TARGET)]);
    let bundle = read_kbdgen_bundle(&fixture.bundle).unwrap();
    let (selected, log) = logged(|| source::layouts(&bundle).unwrap());

    let summary: Vec<(&str, &str)> = selected
        .iter()
        .map(|l| (l.tag.as_str(), l.file.as_str()))
        .collect();
    assert_eq!(
        summary,
        [
            ("se-NO", "se-NO.yaml"),
            ("sme", "sme.yaml"),
            ("vro", "vro.yaml")
        ]
    );
    assert!(
        log.contains("Skipping fi: no windows or chromeOS section"),
        "{log}"
    );
    assert!(
        log.contains(
            "Skipping sme-SE: no linux keyboard; `hardware.linux: {inherits: windows}` would give it its Windows one"
        ),
        "{log}"
    );
    assert!(
        log.contains("se-NO: the v3 layout's linux keyboard inherits its windows variant"),
        "{log}"
    );
    assert!(
        log.contains("vro: the v3 layout's linux keyboard inherits its chromeOS variant"),
        "{log}"
    );

    for layout in &selected {
        let LinuxLayout {
            tag,
            keyboard,
            model,
            display_names,
            ..
        } = layout;
        assert_eq!(keyboard.host, Some(Host::Linux), "{tag}");
        assert!(keyboard.hardware.is_some(), "{tag}");
        assert!(keyboard.touch.is_empty(), "{tag}");
        assert_eq!(
            Model::from_bytes(model).unwrap().keyboard(),
            keyboard,
            "{tag}: the model is K"
        );
        let primary = tag.primary_language();
        assert!(display_names.contains_key(primary), "{tag}");
    }
    assert_eq!(
        selected[0].keyboard,
        migrated_keyboard(&bundle.path, "se-NO", Host::Windows),
        "a v3 layout with a windows section types as it"
    );
    assert_eq!(
        selected[2].keyboard,
        migrated_keyboard(&bundle.path, "vro", Host::ChromeOs),
        "a v3 layout with only a chromeOS section types as it"
    );
}

// [spec:kbdgen:def:ldml.linux.input/test]
#[test]
fn blocked_migrations_and_layerless_keyboards_are_fatal() {
    let keys = "a b c d e f g h i j k l m n o p q r s t u v w x y z 1 2 3 4 5 6 7 8 9 0 A B C D E F G H I J K L";
    let short = keys.rsplit_once(' ').unwrap().0;
    let blocked = format!(
        "displayNames:\n  smj: Julev\nwindows:\n  primary:\n    layers:\n      default: {keys}\n      shift: {short}\n"
    );
    let smj = fixture(&[("smj", &blocked)], &[("linux", TARGET)]);
    let bundle = read_kbdgen_bundle(&smj.bundle).unwrap();
    let error = source::layouts(&bundle).unwrap_err();
    assert_eq!(
        format!("{error:#}"),
        "smj: the v3 layout's migration is blocked by defects M05; `kbdgen ldml migrate --dry-run` reports them"
    );

    let und = fixture(
        &[(
            "und",
            "format: 4\ndisplayNames: {und: Gestures}\nldml: gestures.xml\n",
        )],
        &[("linux", TARGET)],
    );
    write_file(
        &und.bundle,
        "layouts/gestures.xml",
        &fixture_text("crates/kbd-engine/tests/golden/keyboards/gestures.xml"),
    );
    let bundle = read_kbdgen_bundle(&und.bundle).unwrap();
    let error = source::layouts(&bundle).unwrap_err();
    assert_eq!(
        format!("{error:#}"),
        "und: the linux keyboard has no hardware layers"
    );
}

// [spec:kbdgen:def:ldml.linux.input/test]
#[tokio::test]
async fn the_step_compiles_every_layout_and_writes_nothing() {
    let layouts = selection_layouts();
    let layouts: Vec<(&str, &str)> = layouts.iter().map(|(t, y)| (*t, y.as_str())).collect();
    let fixture = fixture(&layouts, &[("linux", TARGET)]);
    generate(&fixture).await.unwrap();
    assert_eq!(fs::read_dir(&fixture.out).unwrap().count(), 0);
}
