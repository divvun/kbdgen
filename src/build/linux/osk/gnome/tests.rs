//! GNOME Shell's layouts of the golden layouts, their parts on small
//! layouts, and the extension that holds them.

use std::borrow::Cow;
use std::collections::BTreeSet;
use std::path::Path;

use serde_json::Value;

use super::*;
use crate::build::BuildStep;
use crate::build::linux::GenerateLinux;
use crate::build::linux::osk::present;
use crate::build::linux::osk::tests::{
    features, linux_layout, mods, se_no, single, sma, us_layer, vro,
};
use crate::build::linux::xkb::adapt;
use crate::bundle::fixture::write_bundle;
use crate::bundle::read_kbdgen_bundle;
use crate::bundle::target::Linux;

const WORKSPACE: &str = env!("CARGO_MANIFEST_DIR");

fn ids() -> Ids {
    Ids::new(&Linux {
        package_id: "no.divvun".to_owned(),
        layout_prefix: "divvun".to_owned(),
        version: "1.0".to_owned(),
    })
    .unwrap()
}

fn en() -> LocaleProjectDescription {
    LocaleProjectDescription {
        name: "Divvun keyboards".to_owned(),
        description: "Keyboards for the Sámi languages".to_owned(),
    }
}

/// The dead keys `layout`'s GNOME Shell layout sends as keyvals.
fn keyvals_of(layout: &LinuxLayout) -> BTreeMap<MarkerIndex, Keysym> {
    keyvals(&adapt(&ids(), layout).unwrap())
}

fn layout_of(layout: &LinuxLayout) -> Layout {
    let id = ids().layout_id(&layout.tag);
    super::layout(&id, layout, &present(layout).unwrap(), &keyvals_of(layout)).unwrap()
}

fn parsed(layout: &LinuxLayout) -> Value {
    serde_json::from_str(&layout_of(layout).json).unwrap()
}

fn keys(document: &Value, level: usize, row: usize) -> &[Value] {
    document["levels"][level]["rows"][row].as_array().unwrap()
}

/// The key whose first string or keyval is `face`, on the default level.
fn default_key<'a>(document: &'a Value, face: &str) -> &'a Value {
    document["levels"][0]["rows"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|row| row.as_array().unwrap())
        .find(|key| key["strings"][0] == face || key["keyval"] == face)
        .unwrap_or_else(|| panic!("no key {face} in {document}"))
}

/// A row's width in keys: its keys' widths and left offsets.
fn span(keys: &[Value]) -> f64 {
    keys.iter()
        .map(|key| key["width"].as_f64().unwrap_or(1.0) + key["leftOffset"].as_f64().unwrap_or(0.0))
        .sum()
}

/// Compares `actual` with the golden file `name`, or rewrites it when
/// `KBDGEN_BLESS` is set.
fn golden(name: &str, actual: &str) {
    let path = Path::new(WORKSPACE)
        .join("src/build/linux/osk/gnome/golden")
        .join(name);
    if std::env::var_os("KBDGEN_BLESS").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, actual).unwrap();
    }
    let expected =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    assert_eq!(
        actual, expected,
        "{name} differs from its golden file; rerun with KBDGEN_BLESS=1 to update it"
    );
}

fn golden_layouts() -> Vec<(&'static str, LinuxLayout)> {
    vec![
        ("vro", linux_layout("vro", &vro())),
        ("single", linux_layout("sma", &single())),
        ("mods", linux_layout("und-x-mods", &mods())),
        ("features", linux_layout("sme", &features())),
        ("se-NO", linux_layout("se-NO", &se_no())),
    ]
}

// [spec:kbdgen:sem:ldml.linux.touch.gnome/test]
#[test]
fn golden_layouts_write_their_golden_files() {
    for (name, layout) in golden_layouts() {
        let written = layout_of(&layout);
        assert_eq!(written.id, ids().layout_id(&layout.tag));
        let value: Value = serde_json::from_str(&written.json).unwrap();
        assert_eq!(
            serde_json::to_string(&value).unwrap(),
            written.json,
            "{name}: no insignificant whitespace, keys in order"
        );
        golden(
            &format!("{name}.json"),
            &(serde_json::to_string_pretty(&value).unwrap() + "\n"),
        );
    }
}

/// Checks that `document`, the GNOME Shell layout of `layout`, holds only
/// the fields of GNOME Shell's format, and that every `levelSwitch` names
/// one of its levels.
pub(in crate::build::linux) fn assert_format(name: &str, layout: &LinuxLayout, document: &Value) {
    let fields = [
        "action",
        "iconName",
        "keyval",
        "label",
        "leftOffset",
        "level",
        "strings",
        "width",
    ];
    let object = document.as_object().unwrap();
    assert_eq!(
        object.keys().collect::<Vec<_>>(),
        ["levels", "locale", "name"]
    );
    assert_eq!(document["locale"], layout.tag.as_str());
    assert_eq!(document["name"], display_name(layout).unwrap());
    let levels = document["levels"].as_array().unwrap();
    let names: Vec<&str> = levels
        .iter()
        .map(|level| level["level"].as_str().unwrap())
        .collect();
    let modes: Vec<&str> = levels
        .iter()
        .map(|level| level["mode"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["default", "shift", "opt", "opt+shift"], "{name}");
    assert_eq!(modes, ["default", "latched", "locked", "locked"], "{name}");
    for level in levels {
        assert_eq!(level.as_object().unwrap().len(), 3, "{name}");
        for key in level["rows"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|row| row.as_array().unwrap())
        {
            let key = key.as_object().unwrap();
            assert!(
                key.keys().all(|field| fields.contains(&field.as_str())),
                "{name}: {key:?}"
            );
            for field in ["width", "leftOffset"] {
                if let Some(value) = key.get(field) {
                    let halves = value.as_f64().unwrap() * 2.0;
                    assert!(halves >= 1.0 && halves.fract() == 0.0, "{name}: {key:?}");
                }
            }
            if key
                .get("action")
                .is_some_and(|action| action == "levelSwitch")
            {
                assert!(
                    names.contains(&key["level"].as_str().unwrap()),
                    "{name}: {key:?}"
                );
            }
            assert!(
                key.contains_key("strings")
                    || key.contains_key("keyval")
                    || key.contains_key("action"),
                "{name}: {key:?} does nothing"
            );
        }
    }
}

// [spec:kbdgen:sem:ldml.linux.touch.gnome/test]
#[test]
fn keys_hold_only_the_formats_fields() {
    for (name, layout) in golden_layouts() {
        assert_format(name, &layout, &parsed(&layout));
    }
}

// [spec:kbdgen:sem:ldml.linux.touch.gnome/test]
#[test]
fn rows_centre_and_space_fills_the_bottom() {
    for (name, layout) in golden_layouts() {
        let document = parsed(&layout);
        for level in document["levels"].as_array().unwrap() {
            let rows: Vec<&[Value]> = level["rows"]
                .as_array()
                .unwrap()
                .iter()
                .map(|row| row.as_array().unwrap().as_slice())
                .collect();
            let (bottom, rows) = rows.split_last().unwrap();
            let widest = span(bottom);
            assert!(widest >= 9.0, "{name}: {widest}");
            let space = &bottom[2];
            assert_eq!(space["strings"], serde_json::json!([" "]));
            assert_eq!(space["width"].as_f64().unwrap(), widest - 6.0, "{name}");
            for row in rows.iter().filter(|row| !row.is_empty()) {
                let right = widest - span(row);
                let left = row[0]["leftOffset"].as_f64().unwrap_or(0.0);
                assert!(right >= 0.0, "{name}: {row:?}");
                assert!(
                    right <= left + 0.5,
                    "{name}: centred, rounded down: {row:?}"
                );
            }
        }
    }
}

// [spec:kbdgen:sem:ldml.linux.touch.gnome/test]
#[test]
fn added_and_bottom_row_keys_are_gnomes() {
    let document = parsed(&linux_layout("vro", &vro()));
    let shift = serde_json::json!({"action": "levelSwitch", "iconName": "osk-shift-symbolic", "level": "shift", "width": 1.5});
    let unshift = serde_json::json!({"action": "levelSwitch", "iconName": "osk-shift-symbolic", "level": "default", "width": 1.5});
    let delete =
        serde_json::json!({"action": "delete", "iconName": "osk-delete-symbolic", "width": 1.5});
    assert_eq!(keys(&document, 0, 0).last().unwrap(), &delete);
    assert_eq!(keys(&document, 0, 3)[0]["action"], "levelSwitch");
    assert_eq!(keys(&document, 0, 3)[0]["level"], "shift");
    let mut first = keys(&document, 0, 3)[0].clone();
    first.as_object_mut().unwrap().remove("leftOffset");
    assert_eq!(first, shift);
    let mut first = keys(&document, 1, 3)[0].clone();
    first.as_object_mut().unwrap().remove("leftOffset");
    assert_eq!(first, unshift);
    for (level, switch) in [(2, ("=/<", "opt+shift")), (3, ("?123", "opt"))] {
        let key = &keys(&document, level, 2)[0];
        assert_eq!(key["action"], "levelSwitch");
        assert_eq!(
            (
                key["label"].as_str().unwrap(),
                key["level"].as_str().unwrap()
            ),
            switch
        );
        assert_eq!(key["width"], 1.5);
    }
    for (level, (label, to)) in [
        (0, ("?123", "opt")),
        (1, ("?123", "opt")),
        (2, ("ABC", "default")),
        (3, ("ABC", "default")),
    ] {
        let rows = document["levels"][level]["rows"].as_array().unwrap();
        let bottom = rows.last().unwrap().as_array().unwrap().as_slice();
        assert_eq!(
            bottom,
            [
                serde_json::json!({"action": "levelSwitch", "label": label, "level": to, "width": 1.5}),
                serde_json::json!({"action": "emoji", "iconName": "osk-emoji-picker-symbolic"}),
                serde_json::json!({"label": "vaih", "strings": [" "], "width": bottom[2]["width"]}),
                serde_json::json!({"action": "languageMenu", "iconName": "osk-layout-symbolic"}),
                serde_json::json!({"action": "hide", "iconName": "osk-hide-symbolic"}),
                serde_json::json!({"iconName": "osk-enter-symbolic", "keyval": "0xff0d", "width": 1.5}),
            ]
        );
    }
}

// [spec:kbdgen:sem:ldml.linux.touch.gnome/test]
#[test]
fn keys_take_strings_labels_widths_and_offsets() {
    let document = parsed(&linux_layout("sma", &single()));
    assert_eq!(default_key(&document, "q")["label"], "Q̄");
    assert_eq!(
        default_key(&document, "w"),
        &serde_json::json!({"strings": ["w"], "width": 1.5})
    );
    assert_eq!(
        default_key(&document, "e"),
        &serde_json::json!({"strings": ["e", "é", "ë"]})
    );
    assert_eq!(
        default_key(&document, "ŋg"),
        &serde_json::json!({"strings": ["ŋg"]})
    );
    assert_eq!(
        default_key(&document, "0xfe51"),
        &serde_json::json!({"keyval": "0xfe51", "label": "´"})
    );
    assert_eq!(
        default_key(&document, "0xfe57"),
        &serde_json::json!({"keyval": "0xfe57", "label": "¨"}),
        "a dead key without a standalone is sent by its dead keysym"
    );
    let row = keys(&document, 0, 2);
    assert_eq!(row[0]["strings"][0], "s");
    assert_eq!(
        row[1],
        serde_json::json!({"leftOffset": 1, "strings": ["d"]}),
        "the gap between s and d"
    );
    let natural = 0.5 + 1.0 + 1.0 + 1.0;
    let widest = span(keys(&document, 0, 4));
    let slack = ((widest - natural) / 2.0 * 2.0).floor() / 2.0;
    assert_eq!(
        row[0]["leftOffset"].as_f64().unwrap(),
        0.5 + slack,
        "the leading gap, then half the slack rounded down to a half key"
    );
    assert!(
        !document.to_string().contains("\\\\m"),
        "a key mixing text and markers is left out"
    );
    for (thousandths, written) in [
        (1000, 2),
        (1200, 2),
        (1250, 3),
        (200, 0),
        (1500, 3),
        (2000, 4),
    ] {
        assert_eq!(halves(thousandths), written, "{thousandths}");
    }
}

/// A layout with the dead keys ´, ¨, ˇ and ★ on the default level at
/// `AC01`–`AC04`, and on lower key codes at other levels: ´ at AltGr on
/// `AE01`, ¨ at AltGr+Shift on `AE02`, ˇ at Shift on `AE03`. No dead
/// keysym stands for ★.
fn dead_levels() -> LinuxLayout {
    let head = "deadKeys:\n  ´: {standalone: ´, compose: {a: á}}\n  ¨: {standalone: ¨, compose: {a: ä}}\n  ˇ: {standalone: ˇ, compose: {c: č}}\n  ★: {standalone: ★, compose: {a: å}}\n";
    linux_layout(
        "sma",
        &sma(
            head,
            &[
                us_layer("none", ["0 1 2 3", "", r"\d{´} \d{¨} \d{ˇ} \d{★}", ""]),
                us_layer("shift", [r") ! @ \d{ˇ}", "", "A S D F", ""]),
                us_layer("altR", [r"x \d{´} y z", "", "", ""]),
                us_layer("altR shift", [r"X Y \d{¨} Z", "", "", ""]),
            ],
        ),
    )
}

// [spec:kbdgen:sem:ldml.linux.touch.gnome/test]
#[test]
fn dead_keys_send_keysyms_only_up_to_level3() {
    let layout = dead_levels();
    let adapted = adapt(&ids(), &layout).unwrap();
    let keysym = |name: &str| Keysym::from_name(name).unwrap();
    let assigned: BTreeSet<Keysym> = adapted.dead.keysyms.values().copied().collect();
    assert_eq!(
        assigned,
        BTreeSet::from([
            keysym("dead_acute"),
            keysym("dead_diaeresis"),
            keysym("dead_caron")
        ]),
        "★ has no dead keysym"
    );
    let sent: BTreeSet<Keysym> = keyvals(&adapted).values().copied().collect();
    assert_eq!(
        sent,
        BTreeSet::from([keysym("dead_acute"), keysym("dead_caron")])
    );
    let document = parsed(&layout);
    let row = keys(&document, 0, 2);
    let faces: Vec<&Value> = row
        .iter()
        .map(|key| key.get("keyval").unwrap_or(&key["strings"]))
        .collect();
    assert_eq!(
        faces,
        [
            &serde_json::json!("0xfe51"),
            &serde_json::json!(["¨"]),
            &serde_json::json!("0xfe5a"),
            &serde_json::json!(["★"]),
        ],
        "´ at Level3 and ˇ at Level2 of lower key codes are sent; ¨ at Level4 and ★ type their standalone"
    );
    let labels: Vec<&Value> = row.iter().map(|key| &key["label"]).collect();
    assert_eq!(labels, ["´", "¨", "ˇ", "★"]);
}

// [spec:kbdgen:def:linux.osk.gnome/test]
#[test]
fn metadata_names_the_extension_for_gnome_50_and_51() {
    let extension = extension(&ids(), "1.2.3", &en(), Vec::new()).unwrap();
    assert_eq!(extension.uuid, "osk@no.divvun");
    assert_eq!(
        extension.metadata,
        "{\n  \"uuid\": \"osk@no.divvun\",\n  \"name\": \"Divvun keyboards\",\n  \"description\": \"Keyboards for the Sámi languages\",\n  \"shell-version\": [\n    \"50\",\n    \"51\"\n  ],\n  \"session-modes\": [\n    \"user\",\n    \"unlock-dialog\"\n  ],\n  \"version-name\": \"1.2.3\"\n}\n"
    );
    let files: Vec<&str> = extension.files().iter().map(|(name, _)| *name).collect();
    assert_eq!(
        files,
        ["metadata.json", "extension.js", "osk-layouts.gresource"]
    );
}

// [spec:kbdgen:def:linux.osk.gnome/test]
#[test]
fn script_registers_the_bundle_and_reloads_sources() {
    let spike = std::fs::read_to_string(
        Path::new(WORKSPACE)
            .join("tests/linux-vm/gnome-spike/ext-reload/osk@no.divvun.spike/extension.js"),
    )
    .unwrap();
    let code = |text: &str| -> Vec<String> {
        text.lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with("//"))
            .map(str::to_owned)
            .collect()
    };
    assert_eq!(code(EXTENSION_JS), code(&spike), "the verified extension");
    assert!(EXTENSION_JS.contains("Gio.Resource.load(`${this.path}/osk-layouts.gresource`)"));
    assert_eq!(
        EXTENSION_JS
            .matches("getInputSourceManager().reload();")
            .count(),
        2
    );
}

// [spec:kbdgen:def:linux.osk.gnome/test]
// [spec:kbdgen:sem:linux.osk.gnome.selection/test]
#[test]
fn bundle_reads_back_each_layout_by_id() {
    let layouts = [linux_layout("vro", &vro()), linux_layout("sma", &single())];
    let written: Vec<Layout> = layouts.iter().map(layout_of).collect();
    let extension = extension(&ids(), "1.0", &en(), written.clone()).unwrap();
    let file = gvdb::read::File::from_bytes(Cow::Borrowed(&extension.bundle)).unwrap();
    let table = file.hash_table().unwrap();
    let mut names: Vec<String> = table.keys().map(|key| key.unwrap().to_string()).collect();
    names.sort();
    assert_eq!(
        names,
        [
            "/",
            "/org/",
            "/org/gnome/",
            "/org/gnome/shell/",
            "/org/gnome/shell/osk-layouts/",
            "/org/gnome/shell/osk-layouts/divvun-sma.json",
            "/org/gnome/shell/osk-layouts/divvun-vro.json",
        ]
    );
    for layout in &written {
        let (size, flags, content): (u32, u32, Vec<u8>) =
            table.get(&layout.resource_path()).unwrap();
        assert_eq!(flags, 0, "uncompressed");
        assert_eq!(size as usize, layout.json.len());
        assert_eq!(&content[..layout.json.len()], layout.json.as_bytes());
        assert_eq!(content[layout.json.len()..], [0], "zero-terminated");
    }
}

// [spec:kbdgen:def:linux.osk.gnome/test]
#[tokio::test]
async fn step_fails_without_an_en_locale() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_bundle(
        dir.path(),
        "test",
        &[("vro", &vro())],
        &[(
            "linux",
            "packageId: no.divvun\nlayoutPrefix: divvun\nversion: 1.0\n",
        )],
        &[],
    );
    std::fs::write(
        path.join("project.yaml"),
        "locales:\n  nb:\n    name: Test\n    description: Test\nauthor: Test\ncopyright: Test\nemail: test@example.com\norganisation: Test\n",
    )
    .unwrap();
    let bundle = read_kbdgen_bundle(&path).unwrap();
    let error = GenerateLinux.build(&bundle, dir.path()).await.unwrap_err();
    assert_eq!(error.to_string(), "project.yaml: locales has no en entry");
}
