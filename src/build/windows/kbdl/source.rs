//! Writes a layout's `Cargo.toml` and `lib.rs`. All layout text reaches
//! `lib.rs` as hexadecimal UTF-16 units, so the file is ASCII whatever the
//! layout contains.

use std::fmt::Write as _;

use super::tables::{
    FIXED_VK_TO_WCH1, FIXED_VK_TO_WCH2, FIXED_VK_TO_WCH3, KeyName, Row, Tables, WCH_NONE,
};

/// `Cargo.toml` of a layout crate named `name`.
// [spec:kbdgen:def:kbdl.crate]
pub fn cargo_toml(name: &str) -> String {
    format!(
        r#"[package]
name = "{name}"
version = "0.0.0"
edition = "2024"
publish = false

[lib]
path = "lib.rs"
crate-type = ["cdylib"]

[features]
wow64 = []

[profile.release]
opt-level = "s"
panic = "abort"
lto = true
codegen-units = 1
debug = false
incremental = false

[workspace]
"#
    )
}

/// A unit array literal, every unit as `0x%04x`.
// [spec:kbdgen:syn:kbdl.escaping]
fn units(units: &[u16]) -> String {
    let mut out = String::from("[");
    for (i, unit) in units.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        let _ = write!(out, "0x{unit:04x}");
    }
    out.push(']');
    out
}

/// Units followed by the `0x0000` terminator.
// [spec:kbdgen:syn:kbdl.escaping]
fn terminated(name: &[u16]) -> Vec<u16> {
    name.iter().copied().chain(std::iter::once(0)).collect()
}

/// `lib.rs` of a layout crate.
// [spec:kbdgen:syn:kbdl.source]
pub fn lib_rs(tables: &Tables) -> String {
    let mut out = String::new();
    header(&mut out);
    pointer_slot(&mut out);
    structs(&mut out);
    layout_assertions(&mut out, tables);
    table_statics(&mut out, tables);
    export(&mut out);
    panic_handler(&mut out);
    debug_assert!(out.is_ascii());
    out
}

// [spec:kbdgen:syn:kbdl.source]
fn header(out: &mut String) {
    out.push_str(
        "#![no_std]
#![allow(non_camel_case_types, non_snake_case, non_upper_case_globals, dead_code)]

use core::mem::{offset_of, size_of};

",
    );
}

/// `P<T>`: a plain pointer, or under `wow64` a pointer zero-extended to the
/// 64-bit table layout the 64-bit kernel reads.
// [spec:kbdgen:req:kbdl.structs.pointers]
fn pointer_slot(out: &mut String) {
    out.push_str(
        r#"#[cfg(all(feature = "wow64", not(target_arch = "x86")))]
compile_error!("the wow64 feature requires an x86 target");

#[cfg(not(feature = "wow64"))]
#[repr(transparent)]
pub struct P<T>(*const T);

#[cfg(feature = "wow64")]
#[repr(C, align(8))]
pub struct P<T>(*const T, u32);

// Every table is immutable and free of interior mutability.
unsafe impl<T> Sync for P<T> {}

impl<T> P<T> {
    #[cfg(not(feature = "wow64"))]
    const fn new(p: *const T) -> Self {
        P(p)
    }

    #[cfg(feature = "wow64")]
    const fn new(p: *const T) -> Self {
        P(p, 0)
    }

    const NULL: Self = Self::new(core::ptr::null());
}

const fn p<T, U>(r: &'static U) -> P<T> {
    P::new(r as *const U as *const T)
}

"#,
    );
}

/// The `kbd.h` structures as `repr(C)` mirrors.
// [spec:kbdgen:def:kbdl.structs]
fn structs(out: &mut String) {
    out.push_str(
        "#[repr(C)]
pub struct VK_TO_BIT {
    pub Vk: u8,
    pub ModBits: u8,
}

#[repr(C)]
pub struct MODIFIERS<const N: usize> {
    pub pVkToBit: P<VK_TO_BIT>,
    pub wMaxModBits: u16,
    pub ModNumber: [u8; N],
}

#[repr(C)]
pub struct VK_TO_WCHARS<const N: usize> {
    pub VirtualKey: u8,
    pub Attributes: u8,
    pub wch: [u16; N],
}

#[repr(C)]
pub struct VK_TO_WCHAR_TABLE {
    pub pVkToWchars: P<()>,
    pub nModifications: u8,
    pub cbSize: u8,
}

#[repr(C)]
pub struct DEADKEY {
    pub dwBoth: u32,
    pub wchComposed: u16,
    pub uFlags: u16,
}

#[repr(C)]
pub struct LIGATURE<const N: usize> {
    pub VirtualKey: u8,
    pub ModificationNumber: u16,
    pub wch: [u16; N],
}

#[repr(C)]
pub struct VSC_LPWSTR {
    pub vsc: u8,
    pub pwsz: P<u16>,
}

#[repr(C)]
pub struct VSC_VK {
    pub Vsc: u8,
    pub Vk: u16,
}

#[repr(C)]
pub struct KBDTABLES {
    pub pCharModifiers: P<()>,
    pub pVkToWcharTable: P<VK_TO_WCHAR_TABLE>,
    pub pDeadKey: P<DEADKEY>,
    pub pKeyNames: P<VSC_LPWSTR>,
    pub pKeyNamesExt: P<VSC_LPWSTR>,
    pub pKeyNamesDead: P<P<u16>>,
    pub pusVSCtoVK: P<u16>,
    pub bMaxVSCtoVK: u8,
    pub pVSCtoVK_E0: P<VSC_VK>,
    pub pVSCtoVK_E1: P<VSC_VK>,
    pub fLocaleFlags: u32,
    pub nLgMax: u8,
    pub cbLgEntry: u8,
    pub pLigature: P<()>,
    pub dwType: u32,
    pub dwSubType: u32,
}

",
    );
}

/// Compile-time assertions of every size and offset Windows reads the
/// tables with; any mismatch fails compilation of the layout crate.
// [spec:kbdgen:thm:kbdl.structs.layout]
fn layout_assertions(out: &mut String, tables: &Tables) {
    out.push_str(
        r#"const WIDE: bool = cfg!(any(target_pointer_width = "64", feature = "wow64"));

const fn by_width(narrow: usize, wide: usize) -> usize {
    if WIDE { wide } else { narrow }
}

const _: () = {
    assert!(size_of::<P<u16>>() == by_width(4, 8));
    assert!(size_of::<KBDTABLES>() == by_width(60, 104));
    assert!(offset_of!(KBDTABLES, bMaxVSCtoVK) == by_width(28, 56));
    assert!(offset_of!(KBDTABLES, pVSCtoVK_E0) == by_width(32, 64));
    assert!(offset_of!(KBDTABLES, fLocaleFlags) == by_width(40, 80));
    assert!(offset_of!(KBDTABLES, nLgMax) == by_width(44, 84));
    assert!(offset_of!(KBDTABLES, pLigature) == by_width(48, 88));
    assert!(offset_of!(KBDTABLES, dwType) == by_width(52, 96));
    assert!(size_of::<VK_TO_WCHAR_TABLE>() == by_width(8, 16));
    assert!(size_of::<VSC_LPWSTR>() == by_width(8, 16));
"#,
    );
    let _ = writeln!(
        out,
        "    assert!(offset_of!(MODIFIERS<{}>, wMaxModBits) == by_width(4, 8));",
        tables.mod_numbers.len()
    );
    out.push_str(
        "    assert!(size_of::<DEADKEY>() == 8);
    assert!(size_of::<VSC_VK>() == 4);
",
    );
    for n in [1, 2, 3, tables.columns] {
        let _ = writeln!(
            out,
            "    assert!(size_of::<VK_TO_WCHARS<{n}>>() == {});",
            2 + 2 * n
        );
    }
    let lg_max = tables.lg_max();
    if lg_max > 0 {
        let _ = writeln!(
            out,
            "    assert!(size_of::<LIGATURE<{lg_max}>>() == {});",
            4 + 2 * lg_max
        );
    }
    out.push_str("};\n\n");
}

fn row(out: &mut String, row: &Row) {
    let _ = writeln!(
        out,
        "    r(0x{:02x}, {}, {}),",
        row.vk,
        row.attributes,
        units(&row.wch)
    );
}

fn fixed_table<const N: usize>(out: &mut String, name: &str, fixed: &[(u8, [u16; N])]) {
    let _ = writeln!(
        out,
        "static {name}: [VK_TO_WCHARS<{N}>; {}] = [",
        fixed.len() + 1
    );
    for (vk, wch) in fixed {
        row(
            out,
            &Row {
                vk: *vk,
                attributes: 0,
                wch: wch.to_vec(),
            },
        );
    }
    let _ = writeln!(out, "    r(0x00, 0, [0x0000; {N}]),\n];\n");
}

fn string_static(out: &mut String, ident: &str, name: &[u16]) {
    let name = terminated(name);
    let _ = writeln!(
        out,
        "static {ident}: [u16; {}] = {};",
        name.len(),
        units(&name)
    );
}

fn key_name_table(out: &mut String, table: &str, prefix: &str, names: &[KeyName]) {
    for name in names {
        string_static(out, &format!("{prefix}_{:02x}", name.scan_code), &name.name);
    }
    let _ = writeln!(
        out,
        "\nstatic {table}: [VSC_LPWSTR; {}] = [",
        names.len() + 1
    );
    for name in names {
        let _ = writeln!(
            out,
            "    n(0x{:02x}, &{prefix}_{:02x}),",
            name.scan_code, name.scan_code
        );
    }
    out.push_str("    VSC_LPWSTR { vsc: 0x00, pwsz: P::NULL },\n];\n\n");
}

/// The tables and the `KbdTables` descriptor, every one a private static.
// [spec:kbdgen:def:kbdl.tables]
fn table_statics(out: &mut String, tables: &Tables) {
    out.push_str(
        "const fn r<const N: usize>(vk: u8, attributes: u8, wch: [u16; N]) -> VK_TO_WCHARS<N> {
    VK_TO_WCHARS { VirtualKey: vk, Attributes: attributes, wch }
}

const fn l<const N: usize>(vk: u8, modification: u16, wch: [u16; N]) -> LIGATURE<N> {
    LIGATURE { VirtualKey: vk, ModificationNumber: modification, wch }
}

const fn d(both: u32, composed: u16, flags: u16) -> DEADKEY {
    DEADKEY { dwBoth: both, wchComposed: composed, uFlags: flags }
}

const fn n(vsc: u8, name: &'static [u16]) -> VSC_LPWSTR {
    VSC_LPWSTR { vsc, pwsz: P::new(name.as_ptr()) }
}

",
    );
    modifiers(out, tables);
    char_tables(out, tables);
    dead_keys(out, tables);
    ligatures(out, tables);
    key_name_table(out, "aKeyNames", "KN", &tables.key_names);
    key_name_table(out, "aKeyNamesExt", "KNE", &tables.key_names_ext);
    dead_key_names(out, tables);
    scancodes(out, tables);
    descriptor(out, tables);
}

// [spec:kbdgen:req:kbdl.modifiers]
fn modifiers(out: &mut String, tables: &Tables) {
    let _ = writeln!(
        out,
        "static aVkToBits: [VK_TO_BIT; {}] = [",
        tables.vk_to_bits.len() + 1
    );
    for (vk, bits) in &tables.vk_to_bits {
        let _ = writeln!(
            out,
            "    VK_TO_BIT {{ Vk: 0x{vk:02x}, ModBits: 0x{bits:02x} }},"
        );
    }
    out.push_str("    VK_TO_BIT { Vk: 0x00, ModBits: 0x00 },\n];\n\n");
    let numbers = tables
        .mod_numbers
        .iter()
        .map(|number| format!("0x{number:02x}"))
        .collect::<Vec<_>>()
        .join(", ");
    let _ = writeln!(
        out,
        "static CharModifiers: MODIFIERS<{}> = MODIFIERS {{\n    pVkToBit: p(&aVkToBits),\n    wMaxModBits: {},\n    ModNumber: [{numbers}],\n}};\n",
        tables.mod_numbers.len(),
        tables.max_mod_bits
    );
}

/// The character tables, with the layout's own table second and the
/// numpad table last so `VkKeyScan` prefers main-block digits.
// [spec:kbdgen:req:kbdl.vk-chars]
// [spec:kbdgen:def:kbdl.vk-chars.fixed]
fn char_tables(out: &mut String, tables: &Tables) {
    let n = tables.columns;
    fixed_table(out, "aVkToWch3", &FIXED_VK_TO_WCH3);
    let _ = writeln!(
        out,
        "static aVkToWch{n}: [VK_TO_WCHARS<{n}>; {}] = [",
        tables.rows.len() + 1
    );
    for r in &tables.rows {
        row(out, r);
    }
    let _ = writeln!(out, "    r(0x00, 0, [0x0000; {n}]),\n];\n");
    fixed_table(out, "aVkToWch2", &FIXED_VK_TO_WCH2);
    fixed_table(out, "aVkToWch1", &FIXED_VK_TO_WCH1);

    out.push_str("static aVkToWcharTable: [VK_TO_WCHAR_TABLE; 5] = [\n");
    for (table, width) in [
        ("aVkToWch3".to_owned(), 3),
        (format!("aVkToWch{n}"), n),
        ("aVkToWch2".to_owned(), 2),
        ("aVkToWch1".to_owned(), 1),
    ] {
        let _ = writeln!(
            out,
            "    VK_TO_WCHAR_TABLE {{ pVkToWchars: p(&{table}), nModifications: {width}, cbSize: {} }},",
            2 + 2 * width
        );
    }
    out.push_str(
        "    VK_TO_WCHAR_TABLE { pVkToWchars: P::NULL, nModifications: 0, cbSize: 0 },\n];\n\n",
    );
}

// [spec:kbdgen:req:kbdl.dead-keys.table]
fn dead_keys(out: &mut String, tables: &Tables) {
    if tables.dead_keys.is_empty() {
        return;
    }
    let _ = writeln!(
        out,
        "static aDeadKey: [DEADKEY; {}] = [",
        tables.dead_keys.len() + 1
    );
    for entry in &tables.dead_keys {
        let _ = writeln!(
            out,
            "    d(0x{:08x}, 0x{:04x}, {}),",
            entry.both(),
            entry.composed,
            entry.flags
        );
    }
    out.push_str("    d(0x00000000, 0x0000, 0),\n];\n\n");
}

/// `aLigature`, every entry padded with `WCH_NONE` to the longest.
// [spec:kbdgen:req:kbdl.ligatures]
fn ligatures(out: &mut String, tables: &Tables) {
    let lg_max = tables.lg_max();
    if lg_max == 0 {
        return;
    }
    let _ = writeln!(
        out,
        "static aLigature: [LIGATURE<{lg_max}>; {}] = [",
        tables.ligatures.len() + 1
    );
    for ligature in &tables.ligatures {
        let mut wch = ligature.units.clone();
        wch.resize(lg_max, WCH_NONE);
        let _ = writeln!(
            out,
            "    l(0x{:02x}, {}, {}),",
            ligature.vk,
            ligature.column,
            units(&wch)
        );
    }
    let _ = writeln!(out, "    l(0x00, 0, [0x0000; {lg_max}]),\n];\n");
}

// [spec:kbdgen:req:kbdl.dead-keys.names+1]
fn dead_key_names(out: &mut String, tables: &Tables) {
    if tables.dead_key_names.is_empty() {
        return;
    }
    for (i, name) in tables.dead_key_names.iter().enumerate() {
        string_static(out, &format!("DKN_{i}"), name);
    }
    let _ = writeln!(
        out,
        "\nstatic aKeyNamesDead: [P<u16>; {}] = [",
        tables.dead_key_names.len() + 1
    );
    for i in 0..tables.dead_key_names.len() {
        let _ = writeln!(out, "    P::new(DKN_{i}.as_ptr()),");
    }
    out.push_str("    P::NULL,\n];\n\n");
}

// [spec:kbdgen:def:kbdl.scancodes]
// [spec:kbdgen:def:kbdl.scancodes.extended]
fn scancodes(out: &mut String, tables: &Tables) {
    let _ = writeln!(out, "static ausVK: [u16; {}] = [", tables.aus_vk.len());
    for chunk in tables.aus_vk.chunks(16) {
        let line = chunk
            .iter()
            .map(|vk| format!("0x{vk:03x}"))
            .collect::<Vec<_>>()
            .join(", ");
        let _ = writeln!(out, "    {line},");
    }
    out.push_str("];\n\n");
    for (name, table) in [
        ("aE0VscToVk", &tables.e0_vsc_to_vk),
        ("aE1VscToVk", &tables.e1_vsc_to_vk),
    ] {
        let _ = writeln!(out, "static {name}: [VSC_VK; {}] = [", table.len() + 1);
        for (scan_code, vk) in table {
            let _ = writeln!(
                out,
                "    VSC_VK {{ Vsc: 0x{scan_code:02x}, Vk: 0x{vk:03x} }},"
            );
        }
        out.push_str("    VSC_VK { Vsc: 0x00, Vk: 0x000 },\n];\n\n");
    }
}

/// `KbdTables`, in `KBDTABLES` field order.
// [spec:kbdgen:def:kbdl.tables]
// [spec:kbdgen:req:kbdl.locale]
// [spec:kbdgen:req:kbdl.ligatures]
fn descriptor(out: &mut String, tables: &Tables) {
    let dead_keys = if tables.dead_keys.is_empty() {
        "P::NULL"
    } else {
        "p(&aDeadKey)"
    };
    let dead_names = if tables.dead_key_names.is_empty() {
        "P::NULL"
    } else {
        "p(&aKeyNamesDead)"
    };
    let lg_max = tables.lg_max();
    let (cb_lg_entry, ligature) = if lg_max == 0 {
        (0, "P::NULL")
    } else {
        (4 + 2 * lg_max, "p(&aLigature)")
    };
    let _ = write!(
        out,
        "static KbdTables: KBDTABLES = KBDTABLES {{
    pCharModifiers: p(&CharModifiers),
    pVkToWcharTable: p(&aVkToWcharTable),
    pDeadKey: {dead_keys},
    pKeyNames: p(&aKeyNames),
    pKeyNamesExt: p(&aKeyNamesExt),
    pKeyNamesDead: {dead_names},
    pusVSCtoVK: p(&ausVK),
    bMaxVSCtoVK: {},
    pVSCtoVK_E0: p(&aE0VscToVk),
    pVSCtoVK_E1: p(&aE1VscToVk),
    fLocaleFlags: 0x{:08x},
    nLgMax: {lg_max},
    cbLgEntry: {cb_lg_entry},
    pLigature: {ligature},
    dwType: 0,
    dwSubType: 0,
}};

",
        tables.aus_vk.len(),
        tables.locale_flags
    );
}

/// The sole export, returning the descriptor's address, zero-extended to
/// 64 bits under `wow64`.
// [spec:kbdgen:req:kbdl.export]
fn export(out: &mut String) {
    out.push_str(
        r#"#[cfg(not(feature = "wow64"))]
#[unsafe(no_mangle)]
pub extern "system" fn KbdLayerDescriptor() -> *const KBDTABLES {
    &KbdTables
}

#[cfg(feature = "wow64")]
#[unsafe(no_mangle)]
pub extern "system" fn KbdLayerDescriptor() -> u64 {
    &KbdTables as *const KBDTABLES as usize as u64
}

"#,
    );
}

// [spec:kbdgen:syn:kbdl.source]
fn panic_handler(out: &mut String) {
    out.push_str(
        "#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}
",
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build::windows::kbdl::{
        diag::Diagnostics,
        input::{ExtraModifierKey, KeyValue, Layer},
        tables::{
            self,
            tests::{base, branch, leaf, set},
        },
    };

    fn sample() -> Tables {
        let mut input = base();
        input.extra_modifiers = vec![ExtraModifierKey::RightCtrl];
        set(&mut input, Layer::Default, "C01", KeyValue::new("ŋ"));
        set(&mut input, Layer::Shift, "C01", KeyValue::new("\u{1F600}"));
        set(&mut input, Layer::Alt, "D05", KeyValue::new("t\u{301}"));
        set(&mut input, Layer::Extra(0), "D05", KeyValue::new("\"'\\"));
        set(&mut input, Layer::Default, "E12", KeyValue::dead("´"));
        input
            .dead_key_tree
            .insert("´".into(), branch(&[("a", leaf("á")), (" ", leaf("´"))]));
        input.dead_key_names.insert("´".into(), "AKUT".into());
        let mut diag = Diagnostics::new("kbdtest");
        tables::build(&input, &mut diag).unwrap()
    }

    // [spec:kbdgen:syn:kbdl.escaping/test]
    #[test]
    fn layout_text_appears_only_as_hex_units() {
        let source = lib_rs(&sample());
        assert!(source.is_ascii());
        assert!(
            source
                .contains("r(0x41, 1, [0x014b, 0xf002, 0xf000, 0xf000, 0xf000, 0xf000, 0xf000]),")
        );
        assert!(source.contains("l(0x41, 1, [0xd83d, 0xde00, 0xf000]),"));
        assert!(source.contains("l(0x54, 3, [0x0074, 0x0301, 0xf000]),"));
        assert!(source.contains("l(0x54, 5, [0x0022, 0x0027, 0x005c]),"));
        assert!(source.contains(
            "static DKN_0: [u16; 6] = [0x00b4, 0x0041, 0x004b, 0x0055, 0x0054, 0x0000];"
        ));
        assert!(source.contains("VSC_VK { Vsc: 0x1d, Vk: 0x1df },"));
        assert!(source.contains("0x0ff, 0x01b, 0x031,"));
        for line in source.lines() {
            assert!(
                !line.replace("&'static", "").contains('\''),
                "no char literals: {line}"
            );
            assert!(!line.contains("b\""), "no byte strings: {line}");
            let quoted = line.matches('"').count();
            if quoted > 0 {
                assert!(
                    line.contains("feature = \"wow64\"")
                        || line.contains("target_arch = \"x86\"")
                        || line.contains("target_pointer_width = \"64\"")
                        || line.contains("compile_error!(\"the wow64 feature")
                        || line.contains("extern \"system\""),
                    "unexpected string literal: {line}"
                );
            }
        }
    }

    // [spec:kbdgen:syn:kbdl.source/test]
    // [spec:kbdgen:def:kbdl.structs/test]
    // [spec:kbdgen:req:kbdl.structs.pointers/test]
    // [spec:kbdgen:req:kbdl.export/test]
    #[test]
    fn source_sections_in_order() {
        let source = lib_rs(&sample());
        let order = [
            "#![no_std]",
            "#![allow(non_camel_case_types, non_snake_case, non_upper_case_globals, dead_code)]",
            "compile_error!(\"the wow64 feature requires an x86 target\");",
            "#[repr(transparent)]\npub struct P<T>(*const T);",
            "#[repr(C, align(8))]\npub struct P<T>(*const T, u32);",
            "unsafe impl<T> Sync for P<T> {}",
            "pub struct VK_TO_BIT {",
            "pub struct KBDTABLES {",
            "const _: () = {",
            "static aVkToBits:",
            "static KbdTables: KBDTABLES",
            "pub extern \"system\" fn KbdLayerDescriptor() -> *const KBDTABLES {",
            "pub extern \"system\" fn KbdLayerDescriptor() -> u64 {",
            "#[panic_handler]",
        ];
        let mut last = 0;
        for needle in order {
            let at = source[last..]
                .find(needle)
                .unwrap_or_else(|| panic!("{needle} missing or out of order"));
            last += at + needle.len();
        }
        let unsafe_uses: Vec<&str> = source
            .lines()
            .filter(|line| line.contains("unsafe"))
            .collect();
        assert_eq!(
            unsafe_uses,
            vec![
                "unsafe impl<T> Sync for P<T> {}",
                "#[unsafe(no_mangle)]",
                "#[unsafe(no_mangle)]"
            ]
        );
        assert!(!source.contains("static mut"));
        assert!(!source.contains("#[repr(packed"));
        assert_eq!(source.matches("pub extern").count(), 2);
        assert!(!source.contains("pub static"));
        assert!(!source.contains("use std"));
    }

    // [spec:kbdgen:thm:kbdl.structs.layout/test]
    #[test]
    fn layout_assertions_cover_every_used_width() {
        let tables = sample();
        let source = lib_rs(&tables);
        for assertion in [
            "assert!(size_of::<KBDTABLES>() == by_width(60, 104));",
            "assert!(offset_of!(KBDTABLES, dwType) == by_width(52, 96));",
            "assert!(offset_of!(MODIFIERS<10>, wMaxModBits) == by_width(4, 8));",
            "assert!(size_of::<VK_TO_WCHARS<1>>() == 4);",
            "assert!(size_of::<VK_TO_WCHARS<3>>() == 8);",
            "assert!(size_of::<VK_TO_WCHARS<7>>() == 16);",
            "assert!(size_of::<LIGATURE<3>>() == 10);",
            "assert!(size_of::<DEADKEY>() == 8);",
            "assert!(size_of::<VSC_VK>() == 4);",
        ] {
            assert!(source.contains(assertion), "{assertion}");
        }
    }

    // [spec:kbdgen:def:kbdl.tables/test]
    // [spec:kbdgen:req:kbdl.vk-chars/test]
    // [spec:kbdgen:req:kbdl.ligatures/test]
    // [spec:kbdgen:req:kbdl.dead-keys.names+1/test]
    #[test]
    fn descriptor_fields_and_null_tables() {
        let source = lib_rs(&sample());
        assert!(source.contains("    pDeadKey: p(&aDeadKey),\n"));
        assert!(source.contains("    pKeyNamesDead: p(&aKeyNamesDead),\n"));
        assert!(source.contains("    bMaxVSCtoVK: 127,\n"));
        assert!(
            source.contains("    nLgMax: 3,\n    cbLgEntry: 10,\n    pLigature: p(&aLigature),\n")
        );
        assert!(source.contains("    dwType: 0,\n    dwSubType: 0,\n"));
        assert!(source.contains(
            "VK_TO_WCHAR_TABLE { pVkToWchars: p(&aVkToWch3), nModifications: 3, cbSize: 8 },\n    VK_TO_WCHAR_TABLE { pVkToWchars: p(&aVkToWch7), nModifications: 7, cbSize: 16 },\n    VK_TO_WCHAR_TABLE { pVkToWchars: p(&aVkToWch2), nModifications: 2, cbSize: 6 },\n    VK_TO_WCHAR_TABLE { pVkToWchars: p(&aVkToWch1), nModifications: 1, cbSize: 4 },\n    VK_TO_WCHAR_TABLE { pVkToWchars: P::NULL, nModifications: 0, cbSize: 0 },"
        ));
        assert!(source.contains("    r(0x00, 0, [0x0000; 7]),\n];"));
        assert!(source.contains("    d(0x00000000, 0x0000, 0),\n];"));

        let mut diag = Diagnostics::new("kbdtest");
        let empty = tables::build(&base(), &mut diag).unwrap();
        let source = lib_rs(&empty);
        assert!(source.contains("    pDeadKey: P::NULL,\n"));
        assert!(source.contains("    pKeyNamesDead: P::NULL,\n"));
        assert!(source.contains("    nLgMax: 0,\n    cbLgEntry: 0,\n    pLigature: P::NULL,\n"));
        assert!(!source.contains("aDeadKey"));
        assert!(!source.contains("aLigature"));
        assert!(!source.contains("aKeyNamesDead"));
    }

    // [spec:kbdgen:def:kbdl.vk-chars.fixed/test]
    #[test]
    fn fixed_character_tables() {
        let source = lib_rs(&sample());
        assert!(source.contains(
            "static aVkToWch3: [VK_TO_WCHARS<3>; 5] = [\n    r(0x08, 0, [0x0008, 0x0008, 0x007f]),\n    r(0x1b, 0, [0x001b, 0x001b, 0x001b]),\n    r(0x0d, 0, [0x000d, 0x000d, 0x000a]),\n    r(0x03, 0, [0x0003, 0x0003, 0x0003]),\n    r(0x00, 0, [0x0000; 3]),\n];"
        ));
        assert!(source.contains(
            "static aVkToWch2: [VK_TO_WCHARS<2>; 6] = [\n    r(0x09, 0, [0x0009, 0x0009]),\n    r(0x6b, 0, [0x002b, 0x002b]),\n    r(0x6f, 0, [0x002f, 0x002f]),\n    r(0x6a, 0, [0x002a, 0x002a]),\n    r(0x6d, 0, [0x002d, 0x002d]),\n    r(0x00, 0, [0x0000; 2]),\n];"
        ));
        let numpad: String = (0..10)
            .map(|i| format!("    r(0x{:02x}, 0, [0x{:04x}]),\n", 0x60 + i, 0x30 + i))
            .collect();
        assert!(source.contains(&format!(
            "static aVkToWch1: [VK_TO_WCHARS<1>; 11] = [\n{numpad}    r(0x00, 0, [0x0000; 1]),\n];"
        )));
    }

    // [spec:kbdgen:def:kbdl.scancodes.extended/test]
    // [spec:kbdgen:def:kbdl.scancodes/test]
    #[test]
    fn scan_code_tables() {
        let mut diag = Diagnostics::new("kbdtest");
        let source = lib_rs(&tables::build(&base(), &mut diag).unwrap());
        assert!(source.contains("static ausVK: [u16; 127] = [\n    0x0ff, 0x01b, 0x031,"));
        assert!(source.contains("    0x0ff, 0x0e9, 0x0ff, 0x0c1, 0x0ff, 0x0ff, 0x087, 0x0ff, 0x0ff, 0x0ff, 0x0ff, 0x0eb, 0x009, 0x0ff, 0x0c2,\n];"));
        assert!(source.contains(
            "static aE0VscToVk: [VSC_VK; 39] = [\n    VSC_VK { Vsc: 0x10, Vk: 0x1b1 },\n    VSC_VK { Vsc: 0x19, Vk: 0x1b0 },\n    VSC_VK { Vsc: 0x1d, Vk: 0x1a3 },"
        ));
        assert!(source.contains(
            "    VSC_VK { Vsc: 0x1c, Vk: 0x10d },\n    VSC_VK { Vsc: 0x46, Vk: 0x103 },\n    VSC_VK { Vsc: 0x00, Vk: 0x000 },\n];"
        ));
        assert!(source.contains(
            "static aE1VscToVk: [VSC_VK; 2] = [\n    VSC_VK { Vsc: 0x1d, Vk: 0x013 },\n    VSC_VK { Vsc: 0x00, Vk: 0x000 },\n];"
        ));
    }

    // [spec:kbdgen:def:kbdl.crate/test]
    #[test]
    fn cargo_manifest_is_exact() {
        assert_eq!(
            cargo_toml("kbdse-FI"),
            "[package]\nname = \"kbdse-FI\"\nversion = \"0.0.0\"\nedition = \"2024\"\npublish = false\n\n[lib]\npath = \"lib.rs\"\ncrate-type = [\"cdylib\"]\n\n[features]\nwow64 = []\n\n[profile.release]\nopt-level = \"s\"\npanic = \"abort\"\nlto = true\ncodegen-units = 1\ndebug = false\nincremental = false\n\n[workspace]\n"
        );
    }
}
