use std::fs;
use std::path::{Path, PathBuf};

use kbd_model::{Form, Hardware, Info, Keyboard, MAGIC};

use crate::data::{Description, Reason, Refusal, engines_xml, found, lookup, usable};

pub fn model_bytes() -> Vec<u8> {
    Keyboard::new("und", 45, Info::default())
        .to_bytes()
        .unwrap()
}

/// A description as kbdgen writes it (`linux.ibus.data`).
pub fn description(name: &str, longname: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<engine>\n  <name>{name}</name>\n  \
         <longname>{longname}</longname>\n  <description>{longname}</description>\n  \
         <language>se_FI</language>\n  <author>Test</author>\n  <layout>{name}</layout>\n  \
         <symbol>se</symbol>\n  <version>1.0</version>\n</engine>\n"
    )
}

/// Installs a keyboard as `<root>/<package>/<name>.{xml,dvkb}`.
pub fn install(root: &Path, package: &str, name: &str, xml: &str, model: Option<&[u8]>) {
    let dir = root.join(package);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join(format!("{name}.xml")), xml).unwrap();
    if let Some(model) = model {
        fs::write(dir.join(format!("{name}.dvkb")), model).unwrap();
    }
}

fn names(roots: &[PathBuf]) -> Vec<(String, PathBuf)> {
    usable(roots)
        .into_iter()
        .map(|found| (found.description.name().to_owned(), found.model))
        .collect()
}

// [spec:kbdgen:req:linux.ibus.lister/test]
#[test]
fn kbdgen_description_parses_and_lists_again() {
    let xml = description("divvun-se-fi", "Sámi &amp; &#x110;");
    let parsed = Description::parse(&xml).unwrap();
    assert_eq!(parsed.name(), "divvun-se-fi");
    let mut out = String::new();
    parsed.write(&mut out);
    assert_eq!(
        out,
        "  <engine>\n    <name>divvun-se-fi</name>\n    \
         <longname>Sámi &amp; Đ</longname>\n    <description>Sámi &amp; Đ</description>\n    \
         <language>se_FI</language>\n    <author>Test</author>\n    \
         <layout>divvun-se-fi</layout>\n    <symbol>se</symbol>\n    \
         <version>1.0</version>\n  </engine>\n"
    );
}

// [spec:kbdgen:req:linux.ibus.lister/test]
#[test]
fn other_shapes_are_unusable() {
    let good = description("l", "L");
    let broken = [
        good.replace("<symbol>se</symbol>", ""),
        good.replace("<engine>", "<engine id=\"x\">"),
        good.replace("<symbol>se</symbol>", "<symbol>se</symbol><!-- c -->"),
        good.replace("<symbol>se</symbol>", "<symbol><b>se</b></symbol>"),
        good.replace("<symbol>se</symbol>", "<symbol>se</sym>"),
        good.replace("<symbol>se</symbol>", "<symbol>s&e;</symbol>"),
        good.replace(
            "<symbol>se</symbol>",
            "<symbol>se</symbol><symbol>x</symbol>",
        ),
        good.replace("<name>l</name>", "<name></name>"),
        format!("{good}<engine/>"),
        good.replace("</engine>", ""),
    ];
    for text in &broken {
        assert_eq!(Description::parse(text), None, "{text}");
    }
    let extra = good.replace(
        "<symbol>se</symbol>",
        "<symbol>se</symbol><icon>i.png</icon>",
    );
    let parsed = Description::parse(&extra).unwrap();
    let mut out = String::new();
    parsed.write(&mut out);
    assert!(!out.contains("icon"), "{out}");
    let bare = good.replace("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n", "");
    assert!(Description::parse(&bare).is_some());
    let empty_author = good.replace("<author>Test</author>", "<author/>");
    assert!(Description::parse(&empty_author).is_some());
}

// [spec:kbdgen:req:linux.ibus.lister/test]
#[test]
fn user_copy_wins_and_order_is_bytewise() {
    let user = tempfile::tempdir().unwrap();
    let system = tempfile::tempdir().unwrap();
    let model = model_bytes();
    install(
        system.path(),
        "b.pkg",
        "z",
        &description("z", "Z"),
        Some(&model),
    );
    install(
        system.path(),
        "a.pkg",
        "y",
        &description("y", "Y"),
        Some(&model),
    );
    install(
        system.path(),
        "a.pkg",
        "X",
        &description("X", "X"),
        Some(&model),
    );
    install(
        system.path(),
        "a.pkg",
        "shared",
        &description("shared", "System"),
        Some(&model),
    );
    install(
        user.path(),
        "c.pkg",
        "shared",
        &description("shared", "User"),
        Some(&model),
    );
    let roots = [user.path().to_path_buf(), system.path().to_path_buf()];
    assert_eq!(
        names(&roots),
        [
            ("shared".to_owned(), user.path().join("c.pkg/shared.dvkb")),
            ("X".to_owned(), system.path().join("a.pkg/X.dvkb")),
            ("y".to_owned(), system.path().join("a.pkg/y.dvkb")),
            ("z".to_owned(), system.path().join("b.pkg/z.dvkb")),
        ]
    );
    let xml = engines_xml(&roots);
    assert!(xml.starts_with("<engines>\n  <engine>\n    <name>shared</name>\n"));
    assert!(xml.contains("<longname>User</longname>"));
    assert!(!xml.contains("System"));
    assert!(xml.ends_with("  </engine>\n</engines>\n"));
}

// [spec:kbdgen:req:linux.ibus.lister/test]
#[test]
fn bad_model_or_stem_leaves_engine_out() {
    let user = tempfile::tempdir().unwrap();
    let system = tempfile::tempdir().unwrap();
    let model = model_bytes();
    let mut future = model.clone();
    future[4] = 9;
    install(
        user.path(),
        "p",
        "shared",
        &description("shared", "User"),
        Some(b"junk"),
    );
    install(
        system.path(),
        "p",
        "shared",
        &description("shared", "System"),
        Some(&model),
    );
    install(
        system.path(),
        "p",
        "missing",
        &description("missing", "M"),
        None,
    );
    install(
        system.path(),
        "p",
        "future",
        &description("future", "F"),
        Some(&future),
    );
    install(
        system.path(),
        "p",
        "stem",
        &description("other", "S"),
        Some(&model),
    );
    fs::write(system.path().join("loose.xml"), description("loose", "L")).unwrap();
    assert!(future.starts_with(&MAGIC));
    let roots = [user.path().to_path_buf(), system.path().to_path_buf()];
    assert_eq!(
        names(&roots),
        [("shared".to_owned(), system.path().join("p/shared.dvkb"))]
    );
    assert_eq!(
        found(&roots).len(),
        4,
        "the junk model's description still parses"
    );
    assert_eq!(
        engines_xml(&[user.path().join("absent")]),
        "<engines>\n</engines>\n"
    );
}

/// A keyboard whose form names one scan code twice, which decoding
/// refuses.
fn invalid_model_bytes() -> Vec<u8> {
    let mut keyboard = Keyboard::new("und", 45, Info::default());
    keyboard.hardware = Some(Hardware {
        form: Form {
            id: "iso".to_owned(),
            rows: vec![vec![0x10, 0x10]],
        },
        min_device_width: None,
        layers: Vec::new(),
    });
    keyboard.to_bytes().unwrap()
}

// [spec:kbdgen:req:linux.ibus.inert/test]
#[test]
fn lookup_names_each_inert_reason() {
    let root = tempfile::tempdir().unwrap();
    let model = model_bytes();
    let mut future = model.clone();
    future[4] = 9;
    let invalid = invalid_model_bytes();
    install(
        root.path(),
        "p",
        "good",
        &description("good", "G"),
        Some(&model),
    );
    install(
        root.path(),
        "p",
        "missing",
        &description("missing", "M"),
        None,
    );
    install(
        root.path(),
        "p",
        "future",
        &description("future", "F"),
        Some(&future),
    );
    install(
        root.path(),
        "p",
        "junk",
        &description("junk", "J"),
        Some(b"DVKB"),
    );
    install(
        root.path(),
        "p",
        "invalid",
        &description("invalid", "I"),
        Some(&invalid),
    );
    let roots = [root.path().to_path_buf()];
    assert!(lookup(&roots, "good").is_ok());
    let refusal = |name: &str| lookup(&roots, name).err().unwrap();
    let model = |name: &str| root.path().join(format!("p/{name}.dvkb"));
    assert_eq!(
        refusal("missing"),
        Refusal {
            reason: Reason::ModelUnreadable,
            file: model("missing")
        }
    );
    assert_eq!(refusal("future").reason, Reason::ModelVersion);
    assert_eq!(refusal("junk").reason, Reason::ModelMalformed);
    assert_eq!(refusal("invalid").reason, Reason::ModelInvalid);
    assert_eq!(refusal("invalid").file, model("invalid"));
    assert_eq!(
        refusal("absent"),
        Refusal {
            reason: Reason::NoDescription,
            file: PathBuf::from("absent.xml")
        }
    );
    let codes = [
        Reason::NoDescription,
        Reason::ModelUnreadable,
        Reason::ModelVersion,
        Reason::ModelMalformed,
        Reason::ModelInvalid,
    ]
    .map(Reason::code);
    assert_eq!(
        codes,
        [
            "no-description",
            "model-unreadable",
            "model-version",
            "model-malformed",
            "model-invalid"
        ]
    );
}

// [spec:kbdgen:req:linux.ibus.lister/test]
// [spec:kbdgen:req:linux.ibus.inert/test]
#[test]
fn lookup_serves_what_the_lister_lists() {
    let user = tempfile::tempdir().unwrap();
    let system = tempfile::tempdir().unwrap();
    let model = model_bytes();
    install(
        user.path(),
        "p",
        "shared",
        &description("shared", "U"),
        Some(b"junk"),
    );
    install(
        system.path(),
        "p",
        "shared",
        &description("shared", "S"),
        Some(&model),
    );
    install(
        user.path(),
        "p",
        "only",
        &description("only", "O"),
        Some(b"junk"),
    );
    let roots = [user.path().to_path_buf(), system.path().to_path_buf()];
    assert!(lookup(&roots, "shared").is_ok());
    assert_eq!(
        lookup(&roots, "only").err().unwrap(),
        Refusal {
            reason: Reason::ModelMalformed,
            file: user.path().join("p/only.dvkb")
        }
    );
    let listed: Vec<String> = usable(&roots)
        .into_iter()
        .map(|found| found.description.name().to_owned())
        .collect();
    assert_eq!(listed, ["shared"]);
}
