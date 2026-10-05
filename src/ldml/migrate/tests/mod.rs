//! Migration tests: small v3 layouts for each mapping and defect, the
//! report, the bundle command, and the Võro and Northern Sámi fixtures.

use std::path::Path;

use super::defect::{Code, Defect};
use super::*;

/// 48 desktop tokens: the ISO rows of a Nordic layout.
const KEYS: &str = "§ 1 2 3 4 5 6 7 8 9 0 + ´ q w e r t y u i o p å ¨ a s d f g h j k l ö ä ' < z x c v b n m , . -";

/// `KEYS` with the letters in uppercase and the digit row shifted.
const SHIFTED: &str = "° ! \" # ¤ % & / ( ) = ? ` Q W E R T Y U I O P Å ^ A S D F G H J K L Ö Ä * > Z X C V B N M ; : _";

fn migrate_as(tag: &str, yaml: &str) -> Migration {
    let path = format!("layouts/{tag}.yaml");
    migrate_text(Path::new(&path), tag, yaml).unwrap_or_else(|e| panic!("{e}"))
}

fn migrate(yaml: &str) -> Migration {
    migrate_as(
        "se",
        &format!("displayNames:\n  se: Davvisámegiella\n{yaml}"),
    )
}

fn codes(m: &Migration) -> Vec<Code> {
    m.defects.iter().map(|d| d.code).collect()
}

/// The codes of `m` other than M09, which every layout without authored
/// caps layers has on some platform.
fn codes_besides_caps(m: &Migration) -> Vec<Code> {
    codes(m).into_iter().filter(|c| *c != Code::M09).collect()
}

fn with_code(m: &Migration, code: Code) -> Vec<&Defect> {
    m.defects.iter().filter(|d| d.code == code).collect()
}

fn yaml(m: &Migration) -> &str {
    m.yaml
        .as_deref()
        .unwrap_or_else(|| panic!("no v4 text: {:?}", m.defects))
}

fn desktop(section: &str, layers: &[(&str, &str)], dead: &str) -> String {
    let mut out = format!("{section}:\n  primary:\n    layers:\n");
    for (name, keys) in layers {
        out.push_str(&format!("      {name}: '{}'\n", keys.replace('\'', "''")));
    }
    out.push_str(dead);
    out
}

mod defects;
mod fixtures;
mod output;
mod platforms;
