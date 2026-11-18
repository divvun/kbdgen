//! The plasma-keyboard package of the golden layouts, and its parts on
//! small layouts.

use std::path::Path;

use super::*;
use crate::build::linux::osk::present;
use crate::build::linux::osk::tests::{
    features, linux_layout, linux_layouts, mods, se_no, single, sma, us_layer, vro,
};
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

fn package_of(layout: &LinuxLayout) -> Package {
    package(&ids(), "1.2.3", layout, &present(layout).unwrap()).unwrap()
}

/// Compares `actual` with the golden file `name`, or rewrites it when
/// `KBDGEN_BLESS` is set.
fn golden(name: &str, actual: &str) {
    let path = Path::new(WORKSPACE)
        .join("src/build/linux/osk/plasma/golden")
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

/// The total weight of each `KeyboardRow` of `qml`, in order.
fn row_weights(qml: &str) -> Vec<f64> {
    let mut rows = Vec::new();
    let mut current: Option<f64> = None;
    for line in qml.lines().map(str::trim) {
        if line == "KeyboardRow {" {
            current = Some(0.0);
        } else if line == "}" {
            if let Some(weight) = current.take() {
                rows.push(weight);
            }
        } else if let Some(weight) = current.as_mut() {
            let explicit = line.split("weight: ").nth(1).map(|rest| {
                rest.split([';', ' '])
                    .next()
                    .unwrap()
                    .parse::<f64>()
                    .unwrap()
            });
            *weight += explicit.unwrap_or(100.0);
        }
    }
    rows
}

// [spec:kbdgen:def:linux.osk.plasma/test]
// [spec:kbdgen:sem:ldml.linux.touch.plasma/test]
#[test]
fn golden_layouts_write_their_golden_packages() {
    let layouts = [
        ("vro", "vro", vro()),
        ("single", "sma", single()),
        ("mods", "und-x-mods", mods()),
        ("features", "sme", features()),
        ("se-NO", "se-NO", se_no()),
    ];
    for (name, stem, yaml) in layouts {
        let layout = linux_layout(stem, &yaml);
        let package = package_of(&layout);
        golden(&format!("{name}.metadata.json"), &package.metadata);
        golden(&format!("{name}.desktop.qml"), &package.layout);
    }
}

// [spec:kbdgen:def:linux.osk.plasma/test]
#[test]
fn metadata_names_every_display_name_and_locale() {
    let layouts = linux_layouts(&[("se-NO", &se_no()), ("sma", &single())]);
    let package = package_of(&layouts[0]);
    assert_eq!(package.id, "no.divvun.se_NO");
    let metadata: serde_json::Value = serde_json::from_str(&package.metadata).unwrap();
    let plugin = &metadata["KPlugin"];
    assert_eq!(plugin["Id"], "no.divvun.se_NO");
    assert_eq!(plugin["Name"], display_name(&layouts[0]).unwrap());
    assert_eq!(plugin["Version"], "1.2.3");
    for (tag, name) in &layouts[0].display_names {
        if tag != "se" {
            assert_eq!(plugin[format!("Name[{}]", tag.replace('-', "_"))], **name);
        }
    }
    assert!(plugin.get("Name[se]").is_none());
    let keyboard = &metadata["X-Plasma-Keyboard"];
    assert_eq!(keyboard["Locales"], serde_json::json!(["se_NO"]));
    assert_eq!(
        keyboard["KeyboardLayouts"],
        serde_json::json!([{
            "File": "desktop.qml",
            "FormFactors": ["phone", "desktop", "mediacenter"],
            "Id": "desktop",
            "Name": display_name(&layouts[0]).unwrap(),
        }])
    );
    let sma: serde_json::Value = serde_json::from_str(&package_of(&layouts[1]).metadata).unwrap();
    assert_eq!(sma["KPlugin"]["Name"], "Åarjelsaemien");
    assert_eq!(sma["KPlugin"]["Name[en]"], "Southern Sami");
    assert!(package.metadata.ends_with("}\n"));
}

// [spec:kbdgen:sem:ldml.linux.touch.plasma/test]
#[test]
fn every_row_weighs_the_same() {
    let layouts = linux_layouts(&[
        ("vro", &vro()),
        ("sma", &single()),
        ("und-x-mods", &mods()),
        ("sme", &features()),
    ]);
    for layout in &layouts {
        let qml = package_of(layout).layout;
        let weights = row_weights(&qml);
        assert!(!weights.is_empty(), "{}", layout.tag);
        assert!(
            weights.iter().all(|w| (w - weights[0]).abs() < 1e-9),
            "{}: {weights:?}",
            layout.tag
        );
        assert!(weights[0] >= 1100.0, "{}: {weights:?}", layout.tag);
    }
}

// [spec:kbdgen:sem:ldml.linux.touch.plasma/test]
#[test]
fn shift_level_loads_through_a_loader() {
    let qml = package_of(&linux_layout("vro", &vro())).layout;
    assert!(qml.starts_with(
        "import QtQuick\n\n\
         import org.kde.plasma.keyboard.virtualkeyboard\n\
         import org.kde.plasma.keyboard.virtualkeyboard.components\n\
         import org.kde.plasma.keyboard.layouts.defaultplugin as SharedLayouts\n\n\
         KeyboardLayoutDefinition {\n\
         \x20   symbols: SharedLayouts.Symbols {}\n\
         \x20   numbers: SharedLayouts.Numbers {}\n\
         \x20   digits: SharedLayouts.Digits {}\n\
         \x20   dialpad: SharedLayouts.Dialpad {}\n\n\
         \x20   KeyboardLayoutLoader {\n\
         \x20       readonly property real maxWidthToHeightRatio: -1\n\
         \x20       sourceComponent: inputEngine && inputEngine.uppercase ? shiftLevel : defaultLevel\n"
    ));
    let (default, shift) = qml.split_once("id: shiftLevel").unwrap();
    assert!(!default.contains("noModifier"), "{default}");
    assert!(
        shift.contains("Key { text: \"Q\"; noModifier: true }"),
        "{shift}"
    );
    assert!(
        default.contains("Key { text: \"ˇ\"; displayText: \"ˇ\" }"),
        "dead keys type their standalone: {default}"
    );
    assert!(
        default
            .contains("Key { text: \"o\"; alternativeKeys: [\"ó\", \"ö\", \"õ\", \"ø\", \"õ̭\"] }"),
        "{default}"
    );
    assert!(default.contains("SpaceKey { weight: 750; displayText: \"vaih\" }"));
    assert_eq!(qml.matches("ShiftKey { weight: 150 }").count(), 2);
    assert_eq!(qml.matches("BackspaceKey { weight: 150 }").count(), 2);
}

// [spec:kbdgen:sem:ldml.linux.touch.plasma/test]
#[test]
fn without_shift_level_the_default_is_main() {
    let layout = linux_layout("sma", &sma("", &[us_layer("none", ["a b", "", "", ""])]));
    let qml = package_of(&layout).layout;
    assert!(!qml.contains("KeyboardLayoutLoader"), "{qml}");
    assert!(!qml.contains("ShiftKey"), "{qml}");
    assert!(
        qml.contains("    dialpad: SharedLayouts.Dialpad {}\n\n    KeyboardLayout {\n"),
        "{qml}"
    );
    assert!(qml.ends_with("    }\n}\n"), "{qml}");
}

// [spec:kbdgen:sem:ldml.linux.touch.plasma/test]
#[test]
fn keys_gaps_and_left_out_dead_keys() {
    let qml = package_of(&linux_layout("sma", &single())).layout;
    let default = qml.split_once("id: shiftLevel").unwrap().0;
    for written in [
        "Key { text: \"´\"; displayText: \"´\" }",
        "Key { text: \"ŋg\" }",
        "Key { text: \"q\"; displayText: \"Q̄\" }",
        "Key { text: \"w\"; weight: 150 }",
        "Key { text: \"e\"; alternativeKeys: [\"é\", \"ë\"] }",
        "Key { text: \"t\"; displayText: \"T!\" }",
        "FillerKey { weight: 50 }",
        "FillerKey {}",
        "SpaceKey { weight: 750; displayText: \"gaskoe\" }",
    ] {
        assert!(default.contains(written), "{written} in {default}");
    }
    assert!(
        !qml.contains("¨"),
        "a dead key without a standalone is left out"
    );
    assert!(
        !qml.contains("\\m"),
        "a key mixing text and markers is left out"
    );
}

// [spec:kbdgen:sem:ldml.linux.touch.plasma/test]
#[test]
fn strings_are_qml_string_literals() {
    let cases = [
        ("a", "\"a\""),
        ("\"", "\"\\\"\""),
        ("\\", "\"\\\\\""),
        ("\n\t", "\"\\n\\t\""),
        ("\u{7}\u{85}", "\"\\u0007\\u0085\""),
        ("\u{2028}\u{2029}\u{FEFF}", "\"\\u2028\\u2029\\uFEFF\""),
        ("õ̭\u{200D}𝑎", "\"õ̭\u{200D}𝑎\""),
    ];
    for (text, literal) in cases {
        assert_eq!(string(text), literal, "{text:?}");
    }
    assert_eq!(weight(1000), "100");
    assert_eq!(weight(1250), "125");
    assert_eq!(weight(625), "62.5");
}
