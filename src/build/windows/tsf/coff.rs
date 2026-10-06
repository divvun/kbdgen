//! Writes the two COFF objects that make `rust-lld` link an Arm64X image:
//! a native ARM64 and an Arm64EC `_load_config_used`. They carry the same
//! data, symbols and relocations as LLD's own test inputs
//! `lld/test/COFF/Inputs/loadconfig-arm64.s` and `loadconfig-arm64ec.s`, so
//! no C compiler or assembler is needed.

pub const MACHINE_ARM64: u16 = 0xaa64;
pub const MACHINE_ARM64EC: u16 = 0xa641;

/// `IMAGE_SCN_CNT_INITIALIZED_DATA | IMAGE_SCN_ALIGN_8BYTES | IMAGE_SCN_MEM_READ`.
const READ_ONLY: u32 = 0x4040_0040;
/// [`READ_ONLY`] with `IMAGE_SCN_MEM_WRITE`.
const READ_WRITE: u32 = 0xc040_0040;

const IMAGE_REL_ARM64_ADDR32: u16 = 0x0001;
const IMAGE_REL_ARM64_ADDR32NB: u16 = 0x0002;
const IMAGE_REL_ARM64_ADDR64: u16 = 0x000e;

const IMAGE_SYM_CLASS_EXTERNAL: u8 = 2;
const IMAGE_SYM_CLASS_STATIC: u8 = 3;

/// The size of `IMAGE_LOAD_CONFIG_DIRECTORY64` that both objects declare.
const LOAD_CONFIG_SIZE: u32 = 0x140;

/// One item of a section's contents, in the terms of the assembler sources.
#[derive(Debug, Clone, Copy)]
pub enum Item {
    /// A label at the current offset; `true` makes it external.
    Label(&'static str, bool),
    Word(u32),
    /// `.fill n, 1, 0`.
    Zeros(usize),
    /// `.xword symbol`: the symbol's 64-bit address.
    Address(&'static str),
    /// `.rva symbol`: the symbol's 32-bit image-relative address.
    Rva(&'static str),
    /// `.word symbol`: the symbol's 32-bit absolute value, for the counts
    /// and sizes the linker defines.
    Value(&'static str),
}

use Item::{Address, Label, Rva, Value, Word, Zeros};

/// A section: its name, characteristics and contents.
pub type SectionItems = (&'static str, u32, &'static [Item]);

/// `loadconfig-arm64.s`: the native load configuration, whose guard
/// fields the linker fills in.
pub const NATIVE: &[SectionItems] = &[(
    ".rdata",
    READ_ONLY,
    &[
        Label("_load_config_used", true),
        Word(LOAD_CONFIG_SIZE),
        Zeros(0x7c),
        Address("__guard_fids_table"),
        Address("__guard_fids_count"),
        Address("__guard_flags"),
        Zeros(8),
        Address("__guard_iat_table"),
        Address("__guard_iat_count"),
        Address("__guard_longjmp_table"),
        Address("__guard_longjmp_count"),
        Zeros(0x80),
    ],
)];

/// `loadconfig-arm64ec.s`: the Arm64EC load configuration, which points
/// at `__chpe_metadata`, and the slots in which the loader stores the
/// emulator's entry points. Every other symbol is one the linker defines
/// for an Arm64EC or Arm64X image.
pub const EC: &[SectionItems] = &[
    (
        ".data",
        READ_WRITE,
        &[
            Label("__chpe_metadata", true),
            Word(2),
            Rva("__hybrid_code_map"),
            Value("__hybrid_code_map_count"),
            Rva("__x64_code_ranges_to_entry_points"),
            Rva("__arm64x_redirection_metadata"),
            Rva("__os_arm64x_dispatch_call_no_redirect"),
            Rva("__os_arm64x_dispatch_ret"),
            Rva("__os_arm64x_check_call"),
            Rva("__os_arm64x_check_icall"),
            Rva("__os_arm64x_check_icall_cfg"),
            Rva("__arm64x_native_entrypoint"),
            Rva("__hybrid_auxiliary_iat"),
            Value("__x64_code_ranges_to_entry_points_count"),
            Value("__arm64x_redirection_metadata_count"),
            Rva("__os_arm64x_get_x64_information"),
            Rva("__os_arm64x_set_x64_information"),
            Rva("__arm64x_extra_rfe_table"),
            Value("__arm64x_extra_rfe_table_size"),
            Rva("__os_arm64x_dispatch_fptr"),
            Rva("__hybrid_auxiliary_iat_copy"),
            Rva("__hybrid_auxiliary_delayload_iat"),
            Rva("__hybrid_auxiliary_delayload_iat_copy"),
            Value("__hybrid_image_info_bitfield"),
            Rva("__os_arm64x_helper3"),
            Rva("__os_arm64x_helper4"),
            Rva("__os_arm64x_helper5"),
            Rva("__os_arm64x_helper6"),
            Rva("__os_arm64x_helper7"),
            Rva("__os_arm64x_helper8"),
            Label("__security_cookie", false),
            Zeros(8),
        ],
    ),
    (
        ".00cfg",
        READ_ONLY,
        &[
            Label("_load_config_used", true),
            Word(LOAD_CONFIG_SIZE),
            Zeros(0x54),
            Address("__security_cookie"),
            Zeros(0x10),
            Address("__guard_check_icall_fptr"),
            Address("__guard_dispatch_icall_fptr"),
            Address("__guard_fids_table"),
            Address("__guard_fids_count"),
            Address("__guard_flags"),
            Zeros(8),
            Address("__guard_iat_table"),
            Address("__guard_iat_count"),
            Address("__guard_longjmp_table"),
            Address("__guard_longjmp_count"),
            Zeros(8),
            Address("__chpe_metadata"),
            Zeros(0x78),
            Label("__guard_check_icall_fptr", false),
            Zeros(8),
            Label("__guard_dispatch_icall_fptr", false),
            Zeros(8),
            Label("__os_arm64x_dispatch_call_no_redirect", false),
            Zeros(8),
            Label("__os_arm64x_dispatch_ret", true),
            Zeros(8),
            Label("__os_arm64x_check_call", false),
            Zeros(8),
            Label("__os_arm64x_dispatch_icall", true),
            Label("__os_arm64x_check_icall", false),
            Zeros(8),
            Label("__os_arm64x_get_x64_information", false),
            Zeros(8),
            Label("__os_arm64x_set_x64_information", false),
            Zeros(8),
            Label("__os_arm64x_check_icall_cfg", false),
            Zeros(8),
            Label("__os_arm64x_dispatch_fptr", false),
            Zeros(8),
            Label("__os_arm64x_helper3", false),
            Zeros(8),
            Label("__os_arm64x_helper4", false),
            Zeros(8),
            Label("__os_arm64x_helper5", false),
            Zeros(8),
            Label("__os_arm64x_helper6", false),
            Zeros(8),
            Label("__os_arm64x_helper7", false),
            Zeros(8),
            Label("__os_arm64x_helper8", false),
            Zeros(8),
        ],
    ),
];

/// A symbol of the object: its name, 1-based section (0 when undefined),
/// value and storage class.
struct Symbol {
    name: &'static str,
    section: i16,
    value: u32,
    class: u8,
}

/// A section laid out: header fields, raw data and relocations as
/// (offset, symbol name, type).
struct Laid {
    name: &'static str,
    characteristics: u32,
    data: Vec<u8>,
    relocations: Vec<(u32, &'static str, u16)>,
}

/// Lays out each section's items, collecting the defined symbols.
fn lay_out(sections: &[SectionItems]) -> (Vec<Laid>, Vec<Symbol>) {
    let mut symbols = Vec::new();
    let mut laid = Vec::new();
    for (index, (name, characteristics, items)) in sections.iter().enumerate() {
        let mut data = Vec::new();
        let mut relocations = Vec::new();
        for item in *items {
            let offset = data.len() as u32;
            let (size, relocation) = match *item {
                Label(label, external) => {
                    symbols.push(Symbol {
                        name: label,
                        section: index as i16 + 1,
                        value: offset,
                        class: if external {
                            IMAGE_SYM_CLASS_EXTERNAL
                        } else {
                            IMAGE_SYM_CLASS_STATIC
                        },
                    });
                    (0, None)
                }
                Word(word) => {
                    data.extend_from_slice(&word.to_le_bytes());
                    (0, None)
                }
                Zeros(size) => (size, None),
                Address(symbol) => (8, Some((symbol, IMAGE_REL_ARM64_ADDR64))),
                Rva(symbol) => (4, Some((symbol, IMAGE_REL_ARM64_ADDR32NB))),
                Value(symbol) => (4, Some((symbol, IMAGE_REL_ARM64_ADDR32))),
            };
            data.resize(data.len() + size, 0);
            if let Some((symbol, kind)) = relocation {
                relocations.push((offset, symbol, kind));
            }
        }
        laid.push(Laid {
            name,
            characteristics: *characteristics,
            data,
            relocations,
        });
    }
    (laid, symbols)
}

/// Writes a COFF object of `machine` with `sections`, [`NATIVE`] or [`EC`]. Every relocation
/// target that no label defines becomes an undefined external symbol.
// [spec:kbdgen:req:tsf.arch.arm64x+1]
pub fn object(machine: u16, sections: &[SectionItems]) -> Vec<u8> {
    let (laid, mut symbols) = lay_out(sections);
    for section in &laid {
        for (_, name, _) in &section.relocations {
            if !symbols.iter().any(|symbol| symbol.name == *name) {
                symbols.push(Symbol {
                    name,
                    section: 0,
                    value: 0,
                    class: IMAGE_SYM_CLASS_EXTERNAL,
                });
            }
        }
    }
    let index_of = |name: &str| {
        symbols
            .iter()
            .position(|symbol| symbol.name == name)
            .unwrap_or_default() as u32
    };

    let headers = 20 + 40 * laid.len();
    let mut body = Vec::new();
    let mut section_headers = Vec::new();
    for section in &laid {
        let raw = headers + body.len();
        body.extend_from_slice(&section.data);
        let relocations = headers + body.len();
        for (offset, name, kind) in &section.relocations {
            body.extend_from_slice(&offset.to_le_bytes());
            body.extend_from_slice(&index_of(name).to_le_bytes());
            body.extend_from_slice(&kind.to_le_bytes());
        }
        let mut header = [0u8; 40];
        header[..section.name.len()].copy_from_slice(section.name.as_bytes());
        header[16..20].copy_from_slice(&(section.data.len() as u32).to_le_bytes());
        header[20..24].copy_from_slice(&(raw as u32).to_le_bytes());
        header[24..28].copy_from_slice(&(relocations as u32).to_le_bytes());
        header[32..34].copy_from_slice(&(section.relocations.len() as u16).to_le_bytes());
        header[36..40].copy_from_slice(&section.characteristics.to_le_bytes());
        section_headers.extend_from_slice(&header);
    }

    let symbol_table = headers + body.len();
    let mut strings = Vec::new();
    for symbol in &symbols {
        let mut record = [0u8; 18];
        if symbol.name.len() <= 8 {
            record[..symbol.name.len()].copy_from_slice(symbol.name.as_bytes());
        } else {
            let offset = 4 + strings.len() as u32;
            record[4..8].copy_from_slice(&offset.to_le_bytes());
            strings.extend_from_slice(symbol.name.as_bytes());
            strings.push(0);
        }
        record[8..12].copy_from_slice(&symbol.value.to_le_bytes());
        record[12..14].copy_from_slice(&symbol.section.to_le_bytes());
        record[16] = symbol.class;
        body.extend_from_slice(&record);
    }
    body.extend_from_slice(&(4 + strings.len() as u32).to_le_bytes());
    body.extend_from_slice(&strings);

    let mut bytes = Vec::with_capacity(headers + body.len());
    bytes.extend_from_slice(&machine.to_le_bytes());
    bytes.extend_from_slice(&(laid.len() as u16).to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&(symbol_table as u32).to_le_bytes());
    bytes.extend_from_slice(&(symbols.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&[0; 4]);
    bytes.extend_from_slice(&section_headers);
    bytes.extend_from_slice(&body);
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build::windows::kbdl::image::Reader;

    /// Each section's (name, size, relocation count, characteristics).
    fn sections(bytes: &[u8]) -> Vec<(String, u32, u16, u32)> {
        (0..usize::from(Reader(bytes).u16(2).unwrap()))
            .map(|i| {
                let at = 20 + 40 * i;
                let name = &bytes[at..at + 8];
                let end = name.iter().position(|byte| *byte == 0).unwrap_or(8);
                (
                    String::from_utf8(name[..end].to_vec()).unwrap(),
                    Reader(bytes).u32(at + 16).unwrap(),
                    Reader(bytes).u16(at + 32).unwrap(),
                    Reader(bytes).u32(at + 36).unwrap(),
                )
            })
            .collect()
    }

    /// The name of symbol `index`, resolving string table offsets.
    fn symbol_name(bytes: &[u8], index: usize) -> String {
        let table = Reader(bytes).u32(8).unwrap() as usize;
        let strings = table + 18 * Reader(bytes).u32(12).unwrap() as usize;
        let record = &bytes[table + 18 * index..table + 18 * index + 18];
        let name = if record[..4] == [0; 4] {
            let at = strings + Reader(record).u32(4).unwrap() as usize;
            let end = bytes[at..].iter().position(|byte| *byte == 0).unwrap();
            &bytes[at..at + end]
        } else {
            let end = record[..8].iter().position(|byte| *byte == 0).unwrap_or(8);
            &record[..end]
        };
        String::from_utf8(name.to_vec()).unwrap()
    }

    // [spec:kbdgen:req:tsf.arch.arm64x+1/test]
    #[test]
    fn objects_match_lld_load_config_inputs() {
        let native = object(MACHINE_ARM64, NATIVE);
        assert_eq!(Reader(&native).u16(0).unwrap(), 0xaa64);
        assert_eq!(
            sections(&native),
            [(".rdata".to_owned(), 0x140, 7, 0x4040_0040)]
        );
        let ec = object(MACHINE_ARM64EC, EC);
        assert_eq!(Reader(&ec).u16(0).unwrap(), 0xa641);
        assert_eq!(
            sections(&ec),
            [
                (".data".to_owned(), 0x7c, 28, 0xc040_0040),
                (".00cfg".to_owned(), 0x1c8, 11, 0x4040_0040),
            ]
        );

        let count = Reader(&ec).u32(12).unwrap() as usize;
        let names: Vec<String> = (0..count).map(|i| symbol_name(&ec, i)).collect();
        assert_eq!(names[0], "__chpe_metadata");
        assert!(names.contains(&"_load_config_used".to_owned()));
        assert!(names.contains(&"__hybrid_code_map".to_owned()));

        let cfg = 20 + 2 * 40;
        let data_size = 0x7c + 28 * 10;
        let config = &ec[cfg + data_size..];
        assert_eq!(Reader(config).u32(0).unwrap(), 0x140);
        let relocation_at = |section: usize, i: usize| {
            let at = Reader(&ec).u32(20 + 40 * section + 24).unwrap() as usize + 10 * i;
            (
                Reader(&ec).u32(at).unwrap(),
                symbol_name(&ec, Reader(&ec).u32(at + 4).unwrap() as usize),
                Reader(&ec).u16(at + 8).unwrap(),
            )
        };
        assert_eq!(
            relocation_at(1, 10),
            (0xc8, "__chpe_metadata".to_owned(), 0x000e)
        );
        assert_eq!(
            relocation_at(0, 1),
            (8, "__hybrid_code_map_count".to_owned(), 0x0001)
        );
        assert_eq!(
            relocation_at(0, 0),
            (4, "__hybrid_code_map".to_owned(), 0x0002)
        );
    }
}
