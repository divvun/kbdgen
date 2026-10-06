//! Writes a layout's version and string resources as a binary `.res` file,
//! byte for byte what `rc.exe` produces for the equivalent script.

use std::collections::BTreeMap;

use super::{diag::Diagnostics, input::Metadata};

const RT_STRING: u16 = 6;
const RT_RCDATA: u16 = 10;
const RT_VERSION: u16 = 16;
const LANG_EN_US: u16 = 0x0409;
const VERSION_FLAGS: u16 = 0x0030;
const STRING_FLAGS: u16 = 0x1030;
/// `MOVEABLE | PURE`, what `rc.exe` writes for `RCDATA`.
const RCDATA_FLAGS: u16 = 0x0030;

/// Name and language of the `RT_RCDATA` resource holding the engine model
/// (`tsf.data.resource`).
pub const MODEL_RESOURCE_ID: u16 = 1;
pub const MODEL_RESOURCE_LANGUAGE: u16 = 0;

/// String id of the layout description, which the registry's
/// `Layout Display Name` resolves.
pub const STRING_DESCRIPTION: u16 = 1000;
pub const STRING_LANGUAGE_NAME: u16 = 1100;
pub const STRING_LOCALE_NAME: u16 = 1200;

/// The version used when the input's version cannot be read.
const FALLBACK_VERSION: [u16; 4] = [1, 0, 0, 0];

/// A decimal number of 0 to 65535, digits only.
fn decimal_u16(text: &str) -> Option<u16> {
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

/// The file version `a.b.c.d`: `a.b.c` from up to three components of the
/// version, missing ones 0, and `d` from the build.
// [spec:kbdgen:req:kbdl.resources]
pub fn file_version(metadata: &Metadata, diag: &mut Diagnostics) -> [u16; 4] {
    let Some(version) = metadata.version.as_deref() else {
        diag.warn("no version is given; using 1.0.0.0");
        return FALLBACK_VERSION;
    };
    let components: Vec<&str> = version.split('.').collect();
    let mut result = [0u16; 4];
    let parsed = components.len() <= 3
        && components
            .iter()
            .enumerate()
            .all(|(i, component)| match decimal_u16(component) {
                Some(value) => {
                    result[i] = value;
                    true
                }
                None => false,
            });
    if !parsed {
        diag.warn(format!(
            "version {version:?} is not up to three decimal components of 0-65535; using 1.0.0.0"
        ));
        return FALLBACK_VERSION;
    }
    if let Some(build) = metadata.build.as_deref() {
        let Some(build) = decimal_u16(build) else {
            diag.warn(format!(
                "build {build:?} is not a decimal number of 0-65535; using version 1.0.0.0"
            ));
            return FALLBACK_VERSION;
        };
        result[3] = build;
    }
    result
}

fn utf16le(text: &str, out: &mut Vec<u8>) {
    for unit in text.encode_utf16() {
        out.extend_from_slice(&unit.to_le_bytes());
    }
}

fn pad4(out: &mut Vec<u8>) {
    while !out.len().is_multiple_of(4) {
        out.push(0);
    }
}

/// One resource entry: the 32-byte header, then the data padded to 4.
// [spec:kbdgen:syn:kbdl.resources.format+1]
fn entry(out: &mut Vec<u8>, kind: u16, name: u16, language: u16, flags: u16, data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(&32u32.to_le_bytes());
    out.extend_from_slice(&0xffffu16.to_le_bytes());
    out.extend_from_slice(&kind.to_le_bytes());
    out.extend_from_slice(&0xffffu16.to_le_bytes());
    out.extend_from_slice(&name.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&flags.to_le_bytes());
    out.extend_from_slice(&language.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(data);
    pad4(out);
}

/// The value of a version node.
enum Value<'a> {
    None,
    Binary(&'a [u8]),
    Text(&'a str),
}

/// One version node with its key, value and already encoded children.
// [spec:kbdgen:syn:kbdl.resources.version]
fn version_node(key: &str, value: Value, children: &[u8]) -> Vec<u8> {
    let mut node = vec![0, 0, 0, 0, 0, 0];
    utf16le(key, &mut node);
    node.extend_from_slice(&[0, 0]);
    pad4(&mut node);
    let (value_length, kind) = match value {
        Value::None => (0, 1u16),
        Value::Binary(bytes) => {
            node.extend_from_slice(bytes);
            (bytes.len(), 0)
        }
        Value::Text(text) => {
            utf16le(text, &mut node);
            node.extend_from_slice(&[0, 0]);
            (text.encode_utf16().count() + 1, 1)
        }
    };
    if !children.is_empty() {
        pad4(&mut node);
        node.extend_from_slice(children);
    }
    let length = node.len() as u16;
    node[0..2].copy_from_slice(&length.to_le_bytes());
    node[2..4].copy_from_slice(&(value_length as u16).to_le_bytes());
    node[4..6].copy_from_slice(&kind.to_le_bytes());
    node
}

/// `VS_VERSION_INFO` with its fixed file info and strings.
// [spec:kbdgen:def:kbdl.resources.fields]
fn version_info(metadata: &Metadata, version: [u16; 4]) -> Vec<u8> {
    let [a, b, c, d] = version.map(u32::from);
    let ms = (a << 16) | b;
    let ls = (c << 16) | d;
    let fixed: [u32; 13] = [
        0xfeef04bd, 0x00010000, ms, ls, ms, ls, 0x3f, 0, 0x00040004, 2, 2, 0, 0,
    ];
    let fixed: Vec<u8> = fixed.iter().flat_map(|value| value.to_le_bytes()).collect();

    let version_text = format!("{a}.{b}.{c}.{d}");
    let description = format!("{} Keyboard Layout", metadata.description);
    let filename = format!("{}.dll", metadata.name);
    let strings = [
        ("CompanyName", metadata.company.as_str()),
        ("FileDescription", description.as_str()),
        ("FileVersion", version_text.as_str()),
        ("InternalName", metadata.name.as_str()),
        ("LegalCopyright", metadata.copyright.as_str()),
        ("OriginalFilename", filename.as_str()),
        ("ProductName", metadata.description.as_str()),
        ("ProductVersion", version_text.as_str()),
    ];
    let mut string_nodes = Vec::new();
    for (key, value) in strings {
        pad4(&mut string_nodes);
        string_nodes.extend(version_node(key, Value::Text(value), &[]));
    }
    let table = version_node("000004B0", Value::None, &string_nodes);
    let mut children = version_node("StringFileInfo", Value::None, &table);
    pad4(&mut children);
    let translation = version_node("Translation", Value::Binary(&[0x00, 0x00, 0xb0, 0x04]), &[]);
    children.extend(version_node("VarFileInfo", Value::None, &translation));
    version_node("VS_VERSION_INFO", Value::Binary(&fixed), &children)
}

/// One string-table block: 16 slots of unit count and unterminated units.
// [spec:kbdgen:syn:kbdl.resources.format+1]
fn string_block(slots: &[(u16, &str)]) -> Vec<u8> {
    let mut out = Vec::new();
    for index in 0..16 {
        match slots.iter().find(|(slot, _)| *slot == index) {
            Some((_, text)) => {
                let count = text.encode_utf16().count() as u16;
                out.extend_from_slice(&count.to_le_bytes());
                utf16le(text, &mut out);
            }
            None => out.extend_from_slice(&0u16.to_le_bytes()),
        }
    }
    out
}

/// The complete `.res` file of a layout. `model`, the encoded engine model
/// of the layout, goes in an `RT_RCDATA` entry before the string tables.
// [spec:kbdgen:req:kbdl.resources]
// [spec:kbdgen:syn:kbdl.resources.format+1]
// [spec:kbdgen:req:ldml.kbdl.model-resource+1]
// [spec:kbdgen:req:tsf.data.resource]
pub fn res_file(metadata: &Metadata, version: [u16; 4], model: Option<&[u8]>) -> Vec<u8> {
    let mut out = Vec::new();
    entry(&mut out, 0, 0, 0, 0, &[]);
    entry(
        &mut out,
        RT_VERSION,
        1,
        LANG_EN_US,
        VERSION_FLAGS,
        &version_info(metadata, version),
    );
    if let Some(model) = model {
        entry(
            &mut out,
            RT_RCDATA,
            MODEL_RESOURCE_ID,
            MODEL_RESOURCE_LANGUAGE,
            RCDATA_FLAGS,
            model,
        );
    }

    let language = (metadata.lcid & 0xffff) as u16;
    let strings = [
        (STRING_DESCRIPTION, language, metadata.description.as_str()),
        (
            STRING_LANGUAGE_NAME,
            language,
            metadata.language_name.as_str(),
        ),
        (
            STRING_LOCALE_NAME,
            LANG_EN_US,
            metadata.locale_name.as_str(),
        ),
    ];
    let mut blocks: BTreeMap<(u16, u16), Vec<(u16, &str)>> = BTreeMap::new();
    for (id, language, text) in strings {
        blocks
            .entry(((id >> 4) + 1, language))
            .or_default()
            .push((id & 15, text));
    }
    for ((block, language), slots) in blocks {
        entry(
            &mut out,
            RT_STRING,
            block,
            language,
            STRING_FLAGS,
            &string_block(&slots),
        );
    }
    out
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    fn vro() -> Metadata {
        Metadata {
            name: "kbdvro".into(),
            description: "Võro".into(),
            language_name: "Võro".into(),
            locale_name: "vro-Latn".into(),
            lcid: 0x2000,
            company: "UiT Norgga árktalaš universitehta".into(),
            copyright: "(c) 2017 Divvun/Giellatekno/UiT".into(),
            version: Some("1.0.3".into()),
            build: Some("40".into()),
        }
    }

    /// Parses a `.res` file into (type, name, language, flags, data).
    pub(crate) fn entries(bytes: &[u8]) -> Vec<(u16, u16, u16, u16, Vec<u8>)> {
        let u16_at = |at: usize| u16::from_le_bytes([bytes[at], bytes[at + 1]]);
        let u32_at = |at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        let mut at = 0;
        let mut result = Vec::new();
        while at < bytes.len() {
            let size = u32_at(at) as usize;
            assert_eq!(u32_at(at + 4), 32);
            let data = bytes[at + 32..at + 32 + size].to_vec();
            result.push((
                u16_at(at + 10),
                u16_at(at + 14),
                u16_at(at + 22),
                u16_at(at + 20),
                data,
            ));
            at += 32 + size.div_ceil(4) * 4;
        }
        result
    }

    // The fixture is rc.exe 10.0.26100 output for the equivalent script:
    // VERSIONINFO with these fields and strings, then STRINGTABLEs 1000 and
    // 1100 under LANGUAGE 0, 8 and 1200 under LANGUAGE 9, 1.
    // [spec:kbdgen:syn:kbdl.resources.format+1/test]
    // [spec:kbdgen:syn:kbdl.resources.version/test]
    // [spec:kbdgen:def:kbdl.resources.fields/test]
    // [spec:kbdgen:req:kbdl.resources/test]
    #[test]
    fn res_file_matches_rc_exe_byte_for_byte() {
        let mut diag = Diagnostics::new("kbdvro");
        let version = file_version(&vro(), &mut diag);
        assert_eq!(version, [1, 0, 3, 40]);
        let bytes = res_file(&vro(), version, None);
        assert_eq!(bytes, include_bytes!("testdata/kbdvro.res").as_slice());
    }

    // [spec:kbdgen:req:kbdl.resources/test]
    // [spec:kbdgen:syn:kbdl.resources.format+1/test]
    #[test]
    fn string_tables_by_block_and_language() {
        let mut metadata = vro();
        metadata.lcid = 0x0c3b;
        metadata.description = "Davvisámegiella (Suopma)".into();
        let entries = entries(&res_file(&metadata, [1, 0, 6, 1], None));
        let header: Vec<(u16, u16, u16, u16)> = entries
            .iter()
            .map(|(kind, name, language, flags, _)| (*kind, *name, *language, *flags))
            .collect();
        assert_eq!(
            header,
            vec![
                (0, 0, 0, 0),
                (16, 1, 0x0409, 0x0030),
                (6, 63, 0x0c3b, 0x1030),
                (6, 69, 0x0c3b, 0x1030),
                (6, 76, 0x0409, 0x1030),
            ]
        );
        assert!(entries[0].4.is_empty());
        let block = &entries[2].4;
        let mut slots = Vec::new();
        let mut at = 0;
        for _ in 0..16 {
            let count = usize::from(u16::from_le_bytes([block[at], block[at + 1]]));
            let units: Vec<u16> = block[at + 2..at + 2 + 2 * count]
                .chunks(2)
                .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
                .collect();
            slots.push(String::from_utf16(&units).unwrap());
            at += 2 + 2 * count;
        }
        assert_eq!(at, block.len());
        assert_eq!(slots[8], "Davvisámegiella (Suopma)");
        assert!(
            slots
                .iter()
                .enumerate()
                .all(|(i, s)| i == 8 || s.is_empty())
        );
    }

    // [spec:kbdgen:req:ldml.kbdl.model-resource+1/test]
    // [spec:kbdgen:syn:kbdl.resources.format+1/test]
    // [spec:kbdgen:req:tsf.data.resource/test]
    #[test]
    fn model_resource_precedes_the_string_tables() {
        let model = b"DVKB\x01\x00\x00\x00\x07";
        let with = res_file(&vro(), [1, 0, 3, 40], Some(model));
        let without = res_file(&vro(), [1, 0, 3, 40], None);
        let entries = entries(&with);
        let header: Vec<(u16, u16, u16, u16)> = entries
            .iter()
            .map(|(kind, name, language, flags, _)| (*kind, *name, *language, *flags))
            .collect();
        assert_eq!(header[2], (RT_RCDATA, 1, 0, 0x0030));
        assert_eq!(entries[2].4, model);
        assert_eq!(with.len(), without.len() + 32 + 12, "data padded to 4");
        let version_end = 32 + 32 + entries[1].4.len().div_ceil(4) * 4;
        assert_eq!(with[..version_end], without[..version_end]);
        assert_eq!(with[version_end + 44..], without[version_end..]);
    }

    // [spec:kbdgen:req:kbdl.resources/test]
    #[test]
    fn file_version_parsing_and_fallbacks() {
        let with = |version: Option<&str>, build: Option<&str>| {
            let mut metadata = vro();
            metadata.version = version.map(Into::into);
            metadata.build = build.map(Into::into);
            let mut diag = Diagnostics::new("kbdvro");
            let version = file_version(&metadata, &mut diag);
            (version, diag.warnings().len())
        };
        assert_eq!(with(Some("1.0.6"), Some("1")), ([1, 0, 6, 1], 0));
        assert_eq!(with(Some("2"), Some("7")), ([2, 0, 0, 7], 0));
        assert_eq!(with(Some("0.1"), None), ([0, 1, 0, 0], 0));
        assert_eq!(with(None, Some("3")), ([1, 0, 0, 0], 1));
        assert_eq!(with(Some("1.2.3.4"), Some("5")), ([1, 0, 0, 0], 1));
        assert_eq!(with(Some("1.x"), Some("5")), ([1, 0, 0, 0], 1));
        assert_eq!(with(Some("65536"), Some("5")), ([1, 0, 0, 0], 1));
        assert_eq!(with(Some("+1"), Some("5")), ([1, 0, 0, 0], 1));
        assert_eq!(with(Some(""), Some("5")), ([1, 0, 0, 0], 1));
        assert_eq!(with(Some("1.0.6"), Some("b2")), ([1, 0, 0, 0], 1));
    }
}
