//! Exported fixture keyboards through Keyman's `kmc` LDML compiler, the
//! validator CLDR itself uses (`ldml.test.kmc`).
//!
//! kbdgen's own content is out of `kmc`'s scope, so `kmc` sees each
//! keyboard as an LDML-only consumer does (`ldml.xml.ldml-view`): without
//! elements in kbdgen's namespace, `special` elements left empty, or the
//! namespace declaration. An LDML error fails the test; hints and warnings
//! do not.
//!
//! The test needs Node.js: it runs `npx -y @keymanapp/kmc@18`, or the
//! executable named by `KBDGEN_KMC`. It is ignored by default; CI runs it
//! with `cargo nextest run --run-ignored all`.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::str::FromStr;

use xmlem::Document;

const KBDGEN_NS: &str = "https://divvun.no/ns/kbdgen-ldml/1";

fn golden() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("crates/kbd-engine/tests/golden")
}

/// The LDML-only view of a keyboard document.
fn ldml_view(xml: &str) -> String {
    let mut doc = Document::from_str(xml).unwrap();
    let root = doc.root();
    let prefixes: Vec<String> = root
        .attributes(&doc)
        .iter()
        .filter(|(_, v)| v.as_str() == KBDGEN_NS)
        .map(|(k, _)| k.prefixed_name().to_string())
        .collect();
    for attribute in &prefixes {
        root.remove_attribute(&mut doc, attribute);
    }
    let local: Vec<&str> = prefixes
        .iter()
        .filter_map(|p| p.strip_prefix("xmlns:"))
        .collect();
    let mut stack = vec![root];
    while let Some(el) = stack.pop() {
        for child in el.children(&doc) {
            let foreign = child.prefix(&doc).is_some_and(|p| local.contains(&p));
            let emptied = child.name(&doc) == "special" && {
                let kept = child.children(&doc);
                kept.iter()
                    .all(|c| c.prefix(&doc).is_some_and(|p| local.contains(&p)))
            };
            if foreign || emptied {
                el.remove_child(&mut doc, child.as_node());
            } else {
                stack.push(child);
            }
        }
    }
    kbd_ldml::write(&doc)
}

fn kmc() -> Command {
    match std::env::var_os("KBDGEN_KMC") {
        Some(path) => Command::new(path),
        None => {
            let mut command = Command::new("npx");
            command.args(["-y", "@keymanapp/kmc@18"]);
            command
        }
    }
}

/// The fixture bundle's exports, one per layout and host, and its LDML
/// keyboards, all as LDML-only views in `dir`.
fn fixture_views(dir: &Path) -> Vec<PathBuf> {
    let exported = dir.join("exported");
    let output = Command::new(env!("CARGO_BIN_EXE_kbdgen"))
        .args(["ldml", "export", "-b"])
        .arg(golden())
        .arg("-o")
        .arg(&exported)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let mut sources: Vec<PathBuf> = Vec::new();
    for source in [exported, golden().join("keyboards")] {
        sources.extend(
            std::fs::read_dir(source)
                .unwrap()
                .map(|e| e.unwrap().path()),
        );
    }
    sources.sort();
    let views = dir.join("views");
    std::fs::create_dir_all(&views).unwrap();
    sources
        .iter()
        .map(|source| {
            let view = views.join(source.file_name().unwrap());
            std::fs::write(&view, ldml_view(&std::fs::read_to_string(source).unwrap())).unwrap();
            view
        })
        .collect()
}

// [spec:kbdgen:req:ldml.test.kmc]
// [spec:kbdgen:req:ldml.test.kmc/test]
#[test]
#[ignore = "needs Node.js and @keymanapp/kmc; run with `cargo nextest run --run-ignored all`"]
fn exported_fixtures_pass_kmc() {
    let dir = tempfile::tempdir().unwrap();
    let views = fixture_views(dir.path());
    assert!(views.len() >= 18, "{views:?}");
    let mut failed = Vec::new();
    for view in &views {
        let kmx = dir
            .path()
            .join("kmx")
            .join(view.file_name().unwrap())
            .with_extension("kmx");
        let output = kmc()
            .arg("build")
            .arg(view)
            .arg("-o")
            .arg(&kmx)
            .output()
            .unwrap();
        let log = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        if !output.status.success() || log.contains(" - error ") {
            failed.push(format!("{}\n{log}", view.display()));
        }
    }
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}

#[test]
fn ldml_view_drops_only_kbdgen_content() {
    let xml = format!(
        r#"<keyboard3 locale="sme" conformsTo="45" xmlns:k="{KBDGEN_NS}"><info name="T"/><keys><key id="a" output="a"/><special><k:role keyId="a" role="shift"/></special></keys><layers formId="iso"><special><k:layer modifiers="cmd"/><other/></special></layers></keyboard3>"#
    );
    let view = ldml_view(&xml);
    assert!(!view.contains("k:") && !view.contains(KBDGEN_NS), "{view}");
    assert!(view.contains(r#"<key id="a" output="a" />"#), "{view}");
    assert_eq!(view.matches("<special>").count(), 1, "{view}");
    assert!(view.contains("<other"), "{view}");
}
