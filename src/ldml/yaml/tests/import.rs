//! Import, the round-trip theorem, and v3/v4 coexistence.

use crate::bundle::{Error as BundleError, fixture, read_kbdgen_bundle};
use crate::ldml::import::{ImportedFile, group};
use crate::ldml::yaml::import::{Imported, import};

use super::*;

/// Writes each document to `<tag>.<host>.xml` and imports them as one
/// group, returning the import and the files as read.
fn import_documents(tag: &str, docs: &[(Host, SourceDocument)]) -> (Imported, Vec<ImportedFile>) {
    let dir = tempfile::tempdir().unwrap();
    let paths: Vec<PathBuf> = docs
        .iter()
        .map(|(host, source)| {
            let path = dir.path().join(format!("{tag}.{}.xml", host.name()));
            std::fs::write(&path, kbd_ldml::write(&source.document)).unwrap();
            path
        })
        .collect();
    let groups = group(&paths).unwrap();
    assert_eq!(groups.len(), 1);
    let dest = dir.path().join(format!("layouts/{tag}.yaml"));
    let imported =
        import(&groups[0].tag, &dest, &groups[0].files).unwrap_or_else(|e| panic!("{e}"));
    (imported, groups.into_iter().next().unwrap().files)
}

fn import_file(path: &Path) -> (String, Imported, ImportedFile) {
    let groups = group(&[path.to_path_buf()]).unwrap();
    let g = groups.into_iter().next().unwrap();
    let dest = std::env::temp_dir().join(format!("{}.yaml", g.tag));
    let imported = import(&g.tag, &dest, &g.files).unwrap_or_else(|e| panic!("{e}"));
    (g.tag, imported, g.files.into_iter().next().unwrap())
}

// [spec:kbdgen:thm:ldml.yaml.roundtrip+2/test]
// [spec:kbdgen:sem:ldml.yaml.import+1/test]
#[test]
fn vro_export_import_export_is_identical() {
    let original = documents("vro", VRO);
    let (imported, _) = import_documents("vro", &original);
    assert!(imported.warnings.is_empty(), "{:?}", imported.warnings);
    let again = documents("vro", &imported.yaml);
    let hosts = |d: &[(Host, SourceDocument)]| d.iter().map(|(h, _)| *h).collect::<Vec<_>>();
    assert_eq!(hosts(&again), hosts(&original));
    for ((host, a), (_, b)) in original.iter().zip(&again) {
        assert_eq!(
            kbd_ldml::write(&a.document),
            kbd_ldml::write(&b.document),
            "{}\n{}",
            host.name(),
            imported.yaml
        );
        assert_eq!(
            kbd_ldml::resolve(a).unwrap().keyboard,
            kbd_ldml::resolve(b).unwrap().keyboard
        );
    }
    let (twice, _) = import_documents("vro", &again);
    assert_eq!(twice.yaml, imported.yaml, "import is deterministic");
}

// [spec:kbdgen:sem:ldml.yaml.import+1/test]
// [spec:kbdgen:sem:ldml.yaml.implied-layers+1/test]
#[test]
fn import_rederives_sugar_from_metadata() {
    let (imported, _) = import_documents("vro", &documents("vro", VRO));
    let yaml = &imported.yaml;
    for fragment in [
        "deadKeys:\n  ´:\n    compose:\n      a: á\n",
        "longPress:\n",
        "      none: |\n        \\d{ˇ} 1 2 3 4 5 6 7 8 9 0 + \\d{´}\n",
        "        \\u{0} š é ŕ t\\u{0301} ý",
        "            \\s{shift:1.25} \\s{gap:0.25} z x c v b n m đ \\s{gap:0.25} \\s{backspace:1.25}\n",
        "            flicks:\n              s: |\n                1 2 3 4 5 6 7 8 9 0 ` ´ \\u{0}\n",
        "targets:\n  windows:\n    locale: vro-Latn\n",
    ] {
        assert!(yaml.contains(fragment), "{fragment}\n{yaml}");
    }
    for absent in [
        "impliedLayers",
        "alt caps:",
        "caps shift",
        "transforms:",
        "keys:",
        "displays:",
    ] {
        assert!(!yaml.contains(absent), "{absent}\n{yaml}");
    }
}

// [spec:kbdgen:thm:ldml.yaml.roundtrip+2/test]
// [spec:kbdgen:req:ldml.test.roundtrip/test]
#[test]
fn cldr_keyboards_survive_import_where_expressible() {
    let expected: [(&str, &[&str]); 9] = [
        ("bn.xml", &["displays"]),
        ("fr-t-k0-test.xml", &["touch"]),
        ("fr.xml", &[]),
        ("ja-Hira-t-k0-flicks.xml", &["displays", "touch"]),
        ("ja-Latn.xml", &[]),
        ("mt-t-k0-47key.xml", &[]),
        ("mt.xml", &[]),
        ("pcm.xml", &["displays"]),
        ("pt-t-k0-abnt2.xml", &[]),
    ];
    let paths = cldr_keyboards();
    assert_eq!(paths.len(), expected.len());
    for (path, (name, inexpressible)) in paths.iter().zip(expected) {
        assert!(path.ends_with(name));
        let (tag, imported, file) = import_file(path);
        let differing: Vec<&str> = imported
            .warnings
            .iter()
            .filter_map(|w| w.strip_suffix(" differs; v4 cannot write it"))
            .filter_map(|w| w.rsplit(": ").next())
            .collect();
        assert_eq!(differing, inexpressible, "{name}: {:?}", imported.warnings);
        let lowered = documents(&tag, &imported.yaml);
        let (_, web) = lowered.iter().find(|(h, _)| *h == Host::Web).unwrap();
        let mut again = kbd_ldml::resolve(web).unwrap().keyboard;
        again.host = None;
        let original = &file.resolved.keyboard;
        if inexpressible.is_empty() {
            assert_eq!(&again, original, "{name}\n{}", imported.yaml);
        } else {
            assert_eq!(again.keys, original.keys, "{name}");
            assert_eq!(again.simple, original.simple, "{name}");
        }
    }
}

// [spec:kbdgen:sem:ldml.yaml.import+1/test]
#[test]
fn normalization_follows_settings() {
    let paths = cldr_keyboards();
    let fr = paths.iter().find(|p| p.ends_with("fr.xml")).unwrap();
    let (_, imported, _) = import_file(fr);
    assert!(
        imported.yaml.contains("normalization: enabled\n"),
        "{}",
        imported.yaml
    );
    let (vro, _) = import_documents("vro", &documents("vro", VRO));
    assert!(!vro.yaml.contains("normalization"));
}

// [spec:kbdgen:sem:ldml.yaml.import+1/test]
#[test]
fn unreproducible_generated_groups_stay_verbatim() {
    let docs = documents("vro", VRO);
    let (_, mac) = docs.iter().find(|(h, _)| *h == Host::MacOs).unwrap();
    let edited = kbd_ldml::write(&mac.document).replace(
        r#"<transform from="\m{dk_00B4}a" to="á" />"#,
        r#"<transform from="\m{dk_00B4}a" to="à" />"#,
    );
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("vro.macOS.xml");
    std::fs::write(&path, edited).unwrap();
    let (_, imported, file) = import_file(&path);
    assert!(
        imported
            .warnings
            .iter()
            .any(|w| w.contains("written without sugar") && w.contains("simple differs")),
        "{:?}",
        imported.warnings
    );
    assert!(!imported.yaml.contains("deadKeys"), "{}", imported.yaml);
    assert!(
        imported.yaml.contains(r"from: \m{dk_00B4}a"),
        "{}",
        imported.yaml
    );
    let lowered = documents("vro", &imported.yaml);
    let (_, again) = lowered.iter().find(|(h, _)| *h == Host::MacOs).unwrap();
    let again = kbd_ldml::resolve(again).unwrap().keyboard;
    assert_eq!(again.simple, file.resolved.keyboard.simple);
}

// [spec:kbdgen:sem:ldml.yaml.import+1/test]
#[test]
fn foreign_touch_sets_get_size_names() {
    let paths = cldr_keyboards();
    let ja = paths
        .iter()
        .find(|p| p.ends_with("ja-Hira-t-k0-flicks.xml"))
        .unwrap();
    let (_, imported, _) = import_file(ja);
    let yaml = &imported.yaml;
    assert!(
        yaml.contains(
            "touch:\n  default:\n    sizes:\n      phone:\n        bottomRow: authored\n"
        ),
        "{yaml}"
    );
    assert!(yaml.contains("normalization: enabled"));
}

// [spec:kbdgen:req:ldml.yaml.coexistence+1/test]
#[test]
fn target_generators_refuse_v4_layouts() {
    let root = tempfile::tempdir().unwrap();
    let path = fixture::write_bundle(
        root.path(),
        "mixed",
        &[
            ("se", "displayNames: {se: Davvisámegiella}\n"),
            ("sme", "format: 4\ndisplayNames: {sme: Davvisámegiella}\n"),
        ],
        &[],
        &[],
    );
    let bundle = read_kbdgen_bundle(&path).unwrap();
    assert_eq!(bundle.layouts.len(), 1);
    assert_eq!(bundle.v4_layouts.len(), 1);
    for target in ["ios", "android", "chromeos"] {
        let err = bundle.reject_v4_layouts(target).unwrap_err().to_string();
        assert_eq!(
            err,
            format!(
                "layout sme is a v4 layout (`format: 4`); the {target} target cannot build v4 layouts yet"
            )
        );
    }
    bundle.reject_v4_layouts("windows").unwrap();
    bundle.reject_v4_layouts("macos").unwrap();
    let v3 = fixture::write_bundle(
        root.path(),
        "v3",
        &[("se", "displayNames: {se: x}\n")],
        &[],
        &[],
    );
    read_kbdgen_bundle(&v3)
        .unwrap()
        .reject_v4_layouts("ios")
        .unwrap();
    let bad = fixture::write_bundle(root.path(), "bad", &[("se", "format: 5\n")], &[], &[]);
    assert!(matches!(
        read_kbdgen_bundle(&bad),
        Err(BundleError::LayoutFormat { .. })
    ));
}

// [spec:kbdgen:req:ldml.yaml.coexistence+1/test]
#[test]
fn ldml_commands_take_only_v4_layouts() {
    let dir = bundle(&[("se", "displayNames: {se: x}\n"), ("vro", VRO)]);
    let files = crate::ldml::layouts::layout_files(dir.path()).unwrap();
    let err = crate::ldml::layouts::host_documents(&files, &[]).unwrap_err();
    assert!(
        err.to_string().contains("layout se is a v3 layout"),
        "{err}"
    );
    let docs = crate::ldml::layouts::host_documents(&files, &["vro".to_string()]).unwrap();
    assert_eq!(docs.len(), 7);
}
