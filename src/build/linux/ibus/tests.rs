use std::fs;
use std::path::Path;

use kbd_engine::Model;

use super::Tree;
use crate::build::linux::ids::Ids;
use crate::build::linux::source;
use crate::bundle::fixture::write_bundle;
use crate::bundle::read_kbdgen_bundle;

const WORKSPACE: &str = env!("CARGO_MANIFEST_DIR");
const TARGET: &str = "packageId: no.divvun\nlayoutPrefix: divvun\nversion: 1.0\n";

fn fixture_text(relative: &str) -> String {
    let path = Path::new(WORKSPACE).join(relative);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// The engine data of a bundle holding `layouts`, with `author` in its
/// `project.yaml`.
fn tree(layouts: &[(&str, &str)], author: &str) -> Tree {
    let dir = tempfile::tempdir().unwrap();
    let bundle = write_bundle(dir.path(), "test", layouts, &[("linux", TARGET)], &[]);
    let project = bundle.join("project.yaml");
    let text = fs::read_to_string(&project).unwrap();
    fs::write(
        &project,
        text.replace("author: Test", &format!("author: {author}")),
    )
    .unwrap();
    let bundle = read_kbdgen_bundle(&bundle).unwrap();
    let ids = Ids::new(bundle.targets.linux.as_ref().unwrap()).unwrap();
    let layouts = source::layouts(&bundle).unwrap();
    Tree::new(&ids, &bundle.project.author, "1.0", &layouts).unwrap()
}

fn features(variant: &str) -> String {
    fixture_text("src/build/windows/kbdl/testdata/features.yaml")
        .replace("hardware:\n  windows:", &format!("hardware:\n  {variant}:"))
}

// [spec:kbdgen:syn:linux.ibus.data/test]
#[test]
fn description_lists_layout_language_and_symbol() {
    let sme_fi = features("linux").replace(
        "displayNames: {sme: Davvisámegiella}",
        "displayNames: {se: Sámi <Suopma> & Ruošša}",
    );
    let tree = tree(
        &[("se-FI", &sme_fi), ("sme", &features("linux"))],
        "Divvun & Giellatekno",
    );
    let names: Vec<&str> = tree.files.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(
        names,
        [
            "divvun-se-fi.xml",
            "divvun-se-fi.dvkb",
            "divvun-sme.xml",
            "divvun-sme.dvkb"
        ]
    );
    let xml = |i: usize| String::from_utf8(tree.files[i].1.clone()).unwrap();
    assert_eq!(
        xml(0),
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <engine>\n  \
         <name>divvun-se-fi</name>\n  \
         <longname>Sámi &lt;Suopma&gt; &amp; Ruošša</longname>\n  \
         <description>Sámi &lt;Suopma&gt; &amp; Ruošša</description>\n  \
         <language>se_FI</language>\n  \
         <author>Divvun &amp; Giellatekno</author>\n  \
         <layout>divvun-se-fi</layout>\n  \
         <symbol>se</symbol>\n  \
         <version>1.0</version>\n\
         </engine>\n"
    );
    assert!(xml(2).contains("  <language>sme</language>\n  <author>"));
    assert!(xml(2).contains("  <longname>Davvisámegiella</longname>\n"));
}

// [spec:kbdgen:syn:linux.ibus.data/test]
#[test]
fn model_is_the_linux_keyboard_encoded() {
    let vro = fixture_text("crates/kbd-engine/tests/golden/layouts/vro.yaml");
    let dir = tempfile::tempdir().unwrap();
    let bundle = write_bundle(
        dir.path(),
        "test",
        &[("vro", &vro)],
        &[("linux", TARGET)],
        &[],
    );
    let bundle = read_kbdgen_bundle(&bundle).unwrap();
    let ids = Ids::new(bundle.targets.linux.as_ref().unwrap()).unwrap();
    let layouts = source::layouts(&bundle).unwrap();
    let tree = Tree::new(&ids, "Test", "2.1", &layouts).unwrap();
    let (name, model) = &tree.files[1];
    assert_eq!(name, "divvun-vro.dvkb");
    let decoded = Model::from_bytes(model).unwrap();
    assert_eq!(decoded.keyboard(), &layouts[0].keyboard);
    assert_eq!(tree.descriptions[0].version, "2.1");
    assert_eq!(tree.descriptions[0].language, "vro");
}
