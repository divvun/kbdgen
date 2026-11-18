//! Stevia's layouts of the golden layouts, and their parts on small
//! layouts.

use std::path::Path;

use serde_json::{Value, json};

use super::*;
use crate::build::linux::ids::Ids;
use crate::build::linux::osk::present;
use crate::build::linux::osk::tests::{
    features, linux_layout, mods, se_no, single, sma, us_layer, vro,
};
use crate::bundle::target::Linux;

const WORKSPACE: &str = env!("CARGO_MANIFEST_DIR");

fn layout_of(layout: &LinuxLayout) -> Layout {
    let ids = Ids::new(&Linux {
        package_id: "no.divvun".to_owned(),
        layout_prefix: "divvun".to_owned(),
        version: "1.0".to_owned(),
    })
    .unwrap();
    let id = ids.layout_id(&layout.tag);
    super::layout(&id, layout, &present(layout).unwrap()).unwrap()
}

fn parsed(layout: &LinuxLayout) -> Value {
    serde_json::from_str(&layout_of(layout).json).unwrap()
}

fn rows(document: &Value, level: usize) -> &[Value] {
    document["levels"][level]["rows"].as_array().unwrap()
}

/// Compares `actual` with the golden file `name`, or rewrites it when
/// `KBDGEN_BLESS` is set.
fn golden(name: &str, actual: &str) {
    let path = Path::new(WORKSPACE)
        .join("src/build/linux/osk/stevia/golden")
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

// [spec:kbdgen:def:linux.osk.stevia/test]
// [spec:kbdgen:sem:ldml.linux.touch.stevia/test]
#[test]
fn golden_layouts_write_their_golden_files() {
    for (name, layout) in golden_layouts() {
        let written = layout_of(&layout);
        assert_eq!(
            written.id,
            format!("divvun-{}", layout.tag.as_str().to_lowercase())
        );
        assert_eq!(written.path(), format!("layouts/{}.json", written.id));
        golden(&format!("{name}.json"), &written.json);
    }
}

// [spec:kbdgen:sem:ldml.linux.touch.stevia/test]
#[test]
fn keys_hold_only_the_formats_fields() {
    for (name, layout) in golden_layouts() {
        let document = parsed(&layout);
        let object = document.as_object().unwrap();
        assert_eq!(
            object.keys().collect::<Vec<_>>(),
            ["levels", "locale", "name"]
        );
        assert_eq!(document["locale"], layout.tag.as_str());
        assert_eq!(document["name"], display_name(&layout).unwrap());
        let levels = document["levels"].as_array().unwrap();
        let names: Vec<&str> = levels
            .iter()
            .map(|level| level["level"].as_str().unwrap())
            .collect();
        let modes: Vec<&str> = levels
            .iter()
            .map(|level| level["mode"].as_str().unwrap())
            .collect();
        assert_eq!(names, ["", "shift", "opt", "opt+shift"], "{name}");
        assert_eq!(modes, ["default", "latched", "locked", "locked"], "{name}");
        for level in levels {
            for key in level["rows"]
                .as_array()
                .unwrap()
                .iter()
                .flat_map(|row| row.as_array().unwrap())
            {
                match key {
                    Value::Array(symbols) => {
                        assert!(!symbols.is_empty() && symbols.iter().all(Value::is_string))
                    }
                    Value::Object(key) => {
                        assert!(key["symbol"].is_string(), "{name}: {key:?}");
                        assert!(
                            key.keys()
                                .all(|field| ["symbol", "symbols", "label", "width"]
                                    .contains(&field.as_str())),
                            "{name}: {key:?}"
                        );
                        if let Some(width) = key.get("width") {
                            let width = width.as_f64().unwrap();
                            assert!((1.0..=10.0).contains(&width) && width != 1.0, "{name}");
                        }
                    }
                    other => panic!("{name}: {other}"),
                }
            }
        }
    }
}

// [spec:kbdgen:sem:ldml.linux.touch.stevia/test]
#[test]
fn rows_drop_gaps_and_added_keys() {
    let document = parsed(&linux_layout("vro", &vro()));
    let default = rows(&document, 0);
    assert_eq!(default.len(), 5);
    assert_eq!(
        default[0][0],
        json!({"symbol": "ˇ", "label": "ˇ"}),
        "a dead key commits its standalone"
    );
    assert_eq!(default[0][12], json!({"symbol": "´", "label": "´"}));
    assert_eq!(default[0].as_array().unwrap().len(), 13, "no backspace");
    assert_eq!(default[2][0], json!(["a", "á", "ä", "æ", "å"]));
    assert_eq!(default[3][0], json!(["<"]), "no shift key");
    assert_eq!(default[4], json!([[","], [" "], ["."]]));
    let shift = rows(&document, 1);
    assert_eq!(
        shift[0][12],
        json!({"symbol": "\u{301}", "label": "\u{25CC}\u{301}"})
    );
    assert_eq!(
        rows(&document, 2)[3],
        json!([["_"], ["/"], [" "], [","], ["."]])
    );
    assert_eq!(
        rows(&document, 3)[3],
        json!([["<"], [">"], [" "], [","], ["."]])
    );
    assert_eq!(
        rows(&document, 2)[2],
        json!([["*"], ["\""], ["'"], [":"], [";"], ["!"], ["?"]]),
        "no symbol-level switch"
    );
}

// [spec:kbdgen:sem:ldml.linux.touch.stevia/test]
#[test]
fn keys_take_labels_widths_and_candidates() {
    let document = parsed(&linux_layout("sma", &single()));
    let default = rows(&document, 0);
    assert_eq!(
        default[0],
        json!([{"symbol": "´", "label": "´"}, ["ŋg"]]),
        "a dead key without a standalone is left out"
    );
    assert_eq!(
        default[1],
        json!([
            {"symbol": "q", "label": "Q̄"},
            {"symbol": "w", "width": 1.5},
            ["e", "é", "ë"],
            {"symbol": "t", "label": "T!"},
            ["h", "w"],
            ["f"],
        ])
    );
    assert_eq!(default[2], json!([["s"], ["d"]]), "gaps are left out");
    assert_eq!(
        default.len(),
        4,
        "the row of only gaps and shift is left out"
    );
    let shift = rows(&document, 1);
    assert_eq!(shift[0], json!([["ß"], ["˝"]]));
    assert_eq!(shift[1], json!([["Q"], ["W"], ["E"]]));
    assert_eq!(shift.len(), 3);
    assert!(
        !document.to_string().contains("\\\\m"),
        "a key mixing text and markers is left out"
    );
}

// [spec:kbdgen:sem:ldml.linux.touch.stevia/test]
#[test]
fn widths_stay_in_stevias_range() {
    let layout = linux_layout(
        "sma",
        &sma(
            "keys:\n  narrow: {output: n, width: 0.5}\n  huge: {output: h, width: 12}\n  wide: {output: w, width: 2}\n",
            &[us_layer(
                "none",
                [r"\k{narrow} \k{huge} \k{wide} x", "", "", ""],
            )],
        ),
    );
    let document = parsed(&layout);
    assert_eq!(
        rows(&document, 0)[0],
        json!([["n"], {"symbol": "h", "width": 10.0}, {"symbol": "w", "width": 2.0}, ["x"]])
    );
    let names: Vec<&Value> = document["levels"]
        .as_array()
        .unwrap()
        .iter()
        .map(|level| &level["level"])
        .collect();
    assert_eq!(names, ["", "opt", "opt+shift"], "no shift level");
}
