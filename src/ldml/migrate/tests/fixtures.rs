//! The v3 Võro and Northern Sámi (Norway) fixtures of the golden vectors:
//! their defects, their loading, and their Windows layout DLLs against the
//! v3 build.

use language_tags::LanguageTag;

use super::*;
use crate::build::windows::kbdl::migrated::compare;
use crate::bundle::{fixture::write_bundle, read_kbdgen_bundle};
use crate::ldml::yaml::load;

const VRO: &str = include_str!("../../../../crates/kbd-engine/tests/golden/v3/vro.yaml");
const SE_NO: &str = include_str!("../../../../crates/kbd-engine/tests/golden/v3/se-NO.yaml");

const WINDOWS: &str = "appName: Test\nversion: 1.0.0\nurl: http://example.com\nuuid: 00000000-0000-0000-0000-000000000000\nbuild: 2\n";

fn distinct(m: &Migration) -> Vec<Code> {
    let mut c = codes(m);
    c.dedup();
    c.sort();
    c.dedup();
    c
}

// [spec:kbdgen:req:ldml.migrate.equivalence+1/test]
// [spec:kbdgen:req:ldml.migrate.output+1/test]
#[test]
fn fixtures_migrate_load_and_match_v3_dlls() {
    let cases = [
        ("vro", VRO, vec![Code::M10, Code::M11]),
        (
            "se-NO",
            SE_NO,
            vec![Code::M09, Code::M13, Code::M14, Code::M16],
        ),
    ];
    for (tag, text, expected) in cases {
        let dir = tempfile::tempdir().unwrap();
        let path = write_bundle(
            dir.path(),
            "t",
            &[(tag, text)],
            &[("windows", WINDOWS)],
            &[],
        );
        let migration = migrate_bundle_layout(&path, tag).unwrap().unwrap();
        assert_eq!(
            distinct(&migration),
            expected,
            "{tag}: {:?}",
            migration.defects
        );
        let out = dir.path().join(format!("{tag}.yaml"));
        std::fs::write(&out, yaml(&migration)).unwrap();
        load(&out, tag).unwrap_or_else(|e| panic!("{tag}: {e}"));

        let bundle = read_kbdgen_bundle(&path).unwrap();
        let tag: LanguageTag = tag.parse().unwrap();
        let comparison = compare(&bundle, &tag).unwrap();
        assert!(
            comparison.unexplained.is_empty(),
            "{tag}: {:#?}",
            comparison.unexplained
        );
    }
}

// [spec:kbdgen:req:ldml.migrate.caps-diff+1/test]
#[test]
fn sme_altgr_caps_differences_are_listed() {
    let m = migrate_as("se-NO", SE_NO);
    let windows = m
        .caps_diffs
        .iter()
        .find(|d| d.platform == "windows")
        .unwrap();
    assert_eq!(windows.state.name(), "alt+caps");
    assert!(
        windows.positions.contains(&"D01"),
        "{:?}",
        windows.positions
    );
    let explained = with_code(&m, Code::M09);
    assert!(
        explained
            .iter()
            .any(|d| d.message.contains("D01 \"q\" → \"Q\"")),
        "{explained:?}"
    );
}
