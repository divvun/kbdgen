//! The XKB tier on fixture layouts.

mod registry;
mod symbols;

use std::path::Path;

use crate::build::linux::ids::Ids;
use crate::build::linux::source::{self, LinuxLayout};
use crate::bundle::fixture::write_bundle;
use crate::bundle::read_kbdgen_bundle;
use crate::bundle::target::Linux;

pub const WORKSPACE: &str = env!("CARGO_MANIFEST_DIR");

pub fn fixture_text(relative: &str) -> String {
    let path = Path::new(WORKSPACE).join(relative);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

pub fn ids() -> Ids {
    Ids::new(&Linux {
        package_id: "no.divvun".to_owned(),
        layout_prefix: "divvun".to_owned(),
        version: "1.0".to_owned(),
    })
    .unwrap()
}

/// The Linux layouts of a bundle holding `layouts`, each (stem, text).
pub fn linux_layouts(layouts: &[(&str, &str)]) -> Vec<LinuxLayout> {
    let dir = tempfile::tempdir().unwrap();
    let target = "packageId: no.divvun\nlayoutPrefix: divvun\nversion: 1.0\n";
    let path = write_bundle(dir.path(), "test", layouts, &[("linux", target)], &[]);
    let bundle = read_kbdgen_bundle(&path).unwrap();
    source::layouts(&bundle).unwrap_or_else(|e| panic!("{e:#}"))
}

/// A v4 layout whose `linux` variant has the one layer `none`, under
/// `display` for the primary language subtag `primary`.
pub fn v4_linux(primary: &str, display: &str, body: &str, rows: &str) -> String {
    v4_layers(primary, display, body, &[("none", rows)])
}

/// A v4 layout whose `linux` variant has `layers`, each (modifiers, rows),
/// and the layers they imply (`ldml.yaml.implied-layers`).
pub fn v4_layers(primary: &str, display: &str, body: &str, layers: &[(&str, &str)]) -> String {
    let mut text = format!(
        "format: 4\ndisplayNames: {{{primary}: {display:?}}}\n{body}hardware:\n  linux:\n    layers:\n"
    );
    for (modifiers, rows) in layers {
        text.push_str(&format!("      {modifiers}: |\n"));
        for row in rows.lines() {
            text.push_str(&format!("        {row}\n"));
        }
    }
    text
}

/// The ISO rows of a Nordic layout.
pub const ROWS: &str = "§ 1 2 3 4 5 6 7 8 9 0 + ´\nq w e r t y u i o p å ¨\na s d f g h j k l ö æ '\n< z x c v b n m , . -";
