//! Reads the PE headers of a built layout DLL and checks the image shape
//! `LoadKeyboardLayout` accepts.

use anyhow::{Context, Result, bail};

const IMAGE_FILE_DLL: u16 = 0x2000;
const IMAGE_SUBSYSTEM_NATIVE: u16 = 1;
const IMAGE_SCN_MEM_EXECUTE: u32 = 0x2000_0000;
const IMAGE_SCN_MEM_READ: u32 = 0x4000_0000;
const IMAGE_SCN_MEM_WRITE: u32 = 0x8000_0000;
const PE32_MAGIC: u16 = 0x10b;
const PE32_PLUS_MAGIC: u16 = 0x20b;
const DIRECTORY_EXPORT: usize = 0;
const DIRECTORY_IMPORT: usize = 1;
const DIRECTORY_RESOURCE: usize = 2;
/// The high bit of a resource directory entry's fields: a string name, or
/// a subdirectory rather than data.
const RESOURCE_HIGH_BIT: u32 = 0x8000_0000;

/// The name of the layout DLL's only export.
pub const EXPORT_NAME: &str = "KbdLayerDescriptor";

/// A section header's name, placement and flags.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    pub name: String,
    pub virtual_address: u32,
    pub virtual_size: u32,
    pub raw_offset: u32,
    pub raw_size: u32,
    pub characteristics: u32,
}

/// The parts of a PE image the layout checks look at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    pub machine: u16,
    pub characteristics: u16,
    pub entry_point: u32,
    pub subsystem: u16,
    pub sections: Vec<Section>,
    /// (RVA, size) of each data directory present.
    pub directories: Vec<(u32, u32)>,
}

/// Bounds-checked little-endian reads; every out-of-range read is an error.
struct Reader<'a>(&'a [u8]);

impl Reader<'_> {
    fn bytes(&self, at: usize, len: usize) -> Result<&[u8]> {
        match at.checked_add(len).and_then(|end| self.0.get(at..end)) {
            Some(bytes) => Ok(bytes),
            None => bail!("truncated image: {len} bytes at offset 0x{at:x} are past the end"),
        }
    }

    fn u16(&self, at: usize) -> Result<u16> {
        let bytes = self.bytes(at, 2)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    fn u32(&self, at: usize) -> Result<u32> {
        let bytes = self.bytes(at, 4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }
}

impl Image {
    /// Parses the DOS, COFF and optional headers and the section table.
    pub fn parse(bytes: &[u8]) -> Result<Image> {
        let reader = Reader(bytes);
        if reader.bytes(0, 2)? != b"MZ" {
            bail!("not a PE image: no MZ signature");
        }
        let pe = reader.u32(0x3c)? as usize;
        if reader.bytes(pe, 4)? != b"PE\0\0" {
            bail!("not a PE image: no PE signature at 0x{pe:x}");
        }
        let coff = pe + 4;
        let machine = reader.u16(coff)?;
        let section_count = usize::from(reader.u16(coff + 2)?);
        let optional_size = usize::from(reader.u16(coff + 16)?);
        let characteristics = reader.u16(coff + 18)?;
        let optional = coff + 20;
        let (directory_count_at, directories_at) = match reader.u16(optional)? {
            PE32_MAGIC => (optional + 92, optional + 96),
            PE32_PLUS_MAGIC => (optional + 108, optional + 112),
            magic => bail!("unknown optional header magic 0x{magic:x}"),
        };
        let entry_point = reader.u32(optional + 16)?;
        let subsystem = reader.u16(optional + 68)?;
        let directory_count = reader.u32(directory_count_at)? as usize;
        if directories_at + 8 * directory_count > optional + optional_size {
            bail!("{directory_count} data directories do not fit the optional header");
        }
        let directories = (0..directory_count)
            .map(|i| {
                let at = directories_at + 8 * i;
                Ok((reader.u32(at)?, reader.u32(at + 4)?))
            })
            .collect::<Result<Vec<_>>>()?;

        let table = optional + optional_size;
        let sections = (0..section_count)
            .map(|i| {
                let at = table + 40 * i;
                let name = reader.bytes(at, 8)?;
                let end = name.iter().position(|byte| *byte == 0).unwrap_or(8);
                Ok(Section {
                    name: String::from_utf8_lossy(&name[..end]).into_owned(),
                    virtual_size: reader.u32(at + 8)?,
                    virtual_address: reader.u32(at + 12)?,
                    raw_size: reader.u32(at + 16)?,
                    raw_offset: reader.u32(at + 20)?,
                    characteristics: reader.u32(at + 36)?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        for section in &sections {
            reader
                .bytes(section.raw_offset as usize, section.raw_size as usize)
                .with_context(|| format!("raw data of section {}", section.name))?;
        }

        Ok(Image {
            machine,
            characteristics,
            entry_point,
            subsystem,
            sections,
            directories,
        })
    }

    fn directory(&self, index: usize) -> (u32, u32) {
        self.directories.get(index).copied().unwrap_or((0, 0))
    }

    fn section_of(&self, rva: u32) -> Option<&Section> {
        self.sections.iter().find(|section| {
            let size = section.virtual_size.max(section.raw_size);
            rva >= section.virtual_address && rva - section.virtual_address < size
        })
    }

    /// The file offset of an RVA, if a section's raw data holds it.
    fn offset_of(&self, rva: u32) -> Result<usize> {
        let Some(section) = self.section_of(rva) else {
            bail!("RVA 0x{rva:x} lies in no section");
        };
        let delta = rva - section.virtual_address;
        if delta >= section.raw_size {
            bail!("RVA 0x{rva:x} lies past the raw data of {}", section.name);
        }
        Ok(section.raw_offset as usize + delta as usize)
    }

    /// The offset field of the first entry of the resource directory at
    /// file offset `at` with numeric id `id`, or with any numeric id.
    fn resource_entry(reader: &Reader, at: usize, id: Option<u16>) -> Result<Option<u32>> {
        let named = usize::from(reader.u16(at + 12)?);
        let ids = usize::from(reader.u16(at + 14)?);
        for i in named..named + ids {
            let entry = at + 16 + 8 * i;
            let name = reader.u32(entry)?;
            if name & RESOURCE_HIGH_BIT == 0 && id.is_none_or(|id| name == u32::from(id)) {
                return Ok(Some(reader.u32(entry + 4)?));
            }
        }
        Ok(None)
    }

    /// The data of the resource with type `kind` and numeric name `id`, in
    /// its first language, or `None` when the image has no such resource.
    pub fn resource(&self, bytes: &[u8], kind: u16, id: u16) -> Result<Option<Vec<u8>>> {
        let (rva, size) = self.directory(DIRECTORY_RESOURCE);
        if rva == 0 || size == 0 {
            return Ok(None);
        }
        let reader = Reader(bytes);
        let rsrc = self.offset_of(rva)?;
        let mut at = rsrc;
        for level in [Some(kind), Some(id), None] {
            let Some(offset) = Self::resource_entry(&reader, at, level)? else {
                return Ok(None);
            };
            let subdirectory = offset & RESOURCE_HIGH_BIT != 0;
            if subdirectory == level.is_none() {
                bail!("resource directory for type {kind} name {id} is malformed");
            }
            at = rsrc + (offset & !RESOURCE_HIGH_BIT) as usize;
        }
        let data_rva = reader.u32(at)?;
        let data_size = reader.u32(at + 4)? as usize;
        Ok(Some(
            reader.bytes(self.offset_of(data_rva)?, data_size)?.to_vec(),
        ))
    }

    /// The exported names, the number of exported functions, the ordinal
    /// base, and the RVA of the first function.
    pub fn exports(&self, bytes: &[u8]) -> Result<Exports> {
        let (rva, size) = self.directory(DIRECTORY_EXPORT);
        if rva == 0 || size == 0 {
            return Ok(Exports::default());
        }
        let reader = Reader(bytes);
        let at = self.offset_of(rva)?;
        let base = reader.u32(at + 16)?;
        let function_count = reader.u32(at + 20)?;
        let name_count = reader.u32(at + 24)?;
        let functions_at = reader.u32(at + 28)?;
        let names_at = reader.u32(at + 32)?;
        let ordinals_at = reader.u32(at + 36)?;
        let mut names = Vec::new();
        for i in 0..name_count.min(64) as usize {
            let name_rva = reader.u32(self.offset_of(names_at)? + 4 * i)?;
            let ordinal = reader.u16(self.offset_of(ordinals_at)? + 2 * i)?;
            let name_at = self.offset_of(name_rva)?;
            let tail = reader.bytes(name_at, bytes.len() - name_at)?;
            let end = tail
                .iter()
                .position(|byte| *byte == 0)
                .unwrap_or(tail.len());
            names.push((String::from_utf8_lossy(&tail[..end]).into_owned(), ordinal));
        }
        let first_function = if function_count > 0 {
            Some(reader.u32(self.offset_of(functions_at)?)?)
        } else {
            None
        };
        Ok(Exports {
            base,
            function_count,
            names,
            first_function,
        })
    }
}

/// The export directory's contents.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Exports {
    pub base: u32,
    pub function_count: u32,
    /// Each name with its index into the function table.
    pub names: Vec<(String, u16)>,
    pub first_function: Option<u32>,
}

/// Checks a built layout DLL: machine, DLL flag, no entry point, native
/// subsystem, exactly the sections `.data` (readable, executable, not
/// writable), `.rsrc` and `.reloc`, no imports, and the single export
/// `KbdLayerDescriptor` at ordinal 1 whose code lies in `.data`.
// [spec:kbdgen:req:kbdl.image.verify]
// [spec:kbdgen:req:kbdl.image]
pub fn verify(bytes: &[u8], machine: u16) -> Result<()> {
    let image = Image::parse(bytes)?;
    if image.machine != machine {
        bail!(
            "machine is 0x{:04x}, expected 0x{machine:04x}",
            image.machine
        );
    }
    if image.characteristics & IMAGE_FILE_DLL == 0 {
        bail!("IMAGE_FILE_DLL is not set");
    }
    if image.entry_point != 0 {
        bail!(
            "AddressOfEntryPoint is 0x{:x}, expected 0",
            image.entry_point
        );
    }
    if image.subsystem != IMAGE_SUBSYSTEM_NATIVE {
        bail!("subsystem is {}, expected 1 (native)", image.subsystem);
    }

    let mut names: Vec<&str> = image
        .sections
        .iter()
        .map(|section| section.name.as_str())
        .collect();
    names.sort_unstable();
    if names != [".data", ".reloc", ".rsrc"] {
        let actual: Vec<&str> = image
            .sections
            .iter()
            .map(|section| section.name.as_str())
            .collect();
        bail!(
            "sections are {}, expected exactly .data, .rsrc and .reloc",
            actual.join(", ")
        );
    }
    let Some(data) = image
        .sections
        .iter()
        .find(|section| section.name == ".data")
    else {
        bail!("no .data section");
    };
    let flags = data.characteristics;
    if flags & IMAGE_SCN_MEM_EXECUTE == 0
        || flags & IMAGE_SCN_MEM_READ == 0
        || flags & IMAGE_SCN_MEM_WRITE != 0
    {
        bail!(
            ".data has characteristics 0x{flags:08x}, expected executable, readable and not writable"
        );
    }

    let (import_rva, import_size) = image.directory(DIRECTORY_IMPORT);
    if import_rva != 0 || import_size != 0 {
        bail!("the import directory is not empty (RVA 0x{import_rva:x}, size {import_size})");
    }

    let exports = image.exports(bytes)?;
    if exports.function_count != 1
        || exports.names.len() != 1
        || exports.names[0] != (EXPORT_NAME.to_owned(), 0)
        || exports.base != 1
    {
        bail!(
            "exports are {:?} with {} function(s) and ordinal base {}, expected only {EXPORT_NAME} at ordinal 1",
            exports.names,
            exports.function_count,
            exports.base
        );
    }
    let in_data = exports
        .first_function
        .and_then(|rva| image.section_of(rva))
        .is_some_and(|section| section.name == ".data");
    if !in_data {
        bail!("{EXPORT_NAME} does not lie in .data");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The Võro layout built by `kbdl.build` for x64.
    const X64: &[u8] = include_bytes!("testdata/kbdvro-x64.dll");
    /// The same layout built for wow64.
    const WOW64: &[u8] = include_bytes!("testdata/kbdvro-wow64.dll");
    /// The x64 layout linked without the `/MERGE` and `/SECTION` arguments,
    /// which `LoadKeyboardLayout` rejects.
    const UNMERGED: &[u8] = include_bytes!("testdata/kbdvro-x64-unmerged.dll");
    /// The v4 Võro golden layout built for x64, with its engine model.
    const V4_X64: &[u8] = include_bytes!("testdata/kbdvro4-x64.dll");

    fn pe(bytes: &[u8]) -> usize {
        u32::from_le_bytes(bytes[0x3c..0x40].try_into().unwrap()) as usize
    }

    fn patch(bytes: &[u8], at: usize, value: &[u8]) -> Vec<u8> {
        let mut copy = bytes.to_vec();
        copy[at..at + value.len()].copy_from_slice(value);
        copy
    }

    #[track_caller]
    fn rejects(bytes: &[u8], machine: u16, needle: &str) {
        let error = format!("{:#}", verify(bytes, machine).unwrap_err());
        assert!(error.contains(needle), "{error:?} lacks {needle:?}");
    }

    fn section_header(bytes: &[u8], name: &str) -> usize {
        let coff = pe(bytes) + 4;
        let count = usize::from(u16::from_le_bytes([bytes[coff + 2], bytes[coff + 3]]));
        let optional_size = usize::from(u16::from_le_bytes([bytes[coff + 16], bytes[coff + 17]]));
        let table = coff + 20 + optional_size;
        (0..count)
            .map(|i| table + 40 * i)
            .find(|at| bytes[*at..*at + 8].starts_with(name.as_bytes()))
            .unwrap()
    }

    // [spec:kbdgen:req:kbdl.image.verify/test]
    // [spec:kbdgen:req:kbdl.image/test]
    // [spec:kbdgen:req:kbdl.wow64/test]
    #[test]
    fn built_layouts_pass() {
        verify(X64, 0x8664).unwrap();
        verify(WOW64, 0x014c).unwrap();
        let image = Image::parse(X64).unwrap();
        let names: Vec<&str> = image.sections.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, [".data", ".rsrc", ".reloc"]);
        assert_eq!(image.sections[0].characteristics, 0x6000_0020);
        let exports = image.exports(X64).unwrap();
        assert_eq!(exports.names, vec![(EXPORT_NAME.to_owned(), 0)]);
        assert_eq!((exports.base, exports.function_count), (1, 1));
        assert_eq!(Image::parse(WOW64).unwrap().machine, 0x014c);
    }

    // [spec:kbdgen:req:kbdl.image.verify/test]
    // [spec:kbdgen:req:ldml.kbdl.model-resource+1/test]
    // [spec:kbdgen:req:tsf.data.resource/test]
    #[test]
    fn embedded_models_keep_the_image_valid() {
        verify(V4_X64, 0x8664).unwrap();
        let image = Image::parse(V4_X64).unwrap();
        let model = image.resource(V4_X64, 10, 1).unwrap().unwrap();
        let model = kbd_engine::Model::from_bytes(&model).unwrap();
        assert_eq!(model.keyboard().locale, "vro");

        let image = Image::parse(X64).unwrap();
        assert_eq!(image.resource(X64, 10, 1).unwrap(), None);
        let version = image.resource(X64, 16, 1).unwrap().unwrap();
        let key: Vec<u8> = "VS_VERSION_INFO"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect();
        assert_eq!(&version[6..6 + key.len()], key.as_slice());

        let rsrc = image.sections.iter().find(|s| s.name == ".rsrc").unwrap();
        let truncated = &X64[..rsrc.raw_offset as usize + 20];
        assert!(image.resource(truncated, 16, 1).is_err());
    }

    // [spec:kbdgen:req:kbdl.image.verify/test]
    // [spec:kbdgen:req:kbdl.image/test]
    #[test]
    fn unmerged_and_mismatched_images_fail() {
        rejects(UNMERGED, 0x8664, "sections are");
        rejects(X64, 0xaa64, "machine is 0x8664, expected 0xaa64");
        rejects(WOW64, 0x8664, "machine is 0x014c");
    }

    // [spec:kbdgen:req:kbdl.image.verify/test]
    #[test]
    fn each_header_property_is_checked() {
        let coff = pe(X64) + 4;
        let optional = coff + 20;

        let characteristics = u16::from_le_bytes([X64[coff + 18], X64[coff + 19]]);
        let not_dll = patch(
            X64,
            coff + 18,
            &(characteristics & !IMAGE_FILE_DLL).to_le_bytes(),
        );
        rejects(&not_dll, 0x8664, "IMAGE_FILE_DLL");

        rejects(
            &patch(X64, optional + 16, &0x1000u32.to_le_bytes()),
            0x8664,
            "AddressOfEntryPoint",
        );
        rejects(
            &patch(X64, optional + 68, &2u16.to_le_bytes()),
            0x8664,
            "subsystem is 2",
        );

        let data = section_header(X64, ".data");
        let writable = patch(X64, data + 36, &0xe000_0020u32.to_le_bytes());
        rejects(&writable, 0x8664, ".data has characteristics 0xe0000020");
        let not_executable = patch(X64, data + 36, &0x4000_0040u32.to_le_bytes());
        rejects(&not_executable, 0x8664, ".data has characteristics");

        rejects(
            &patch(X64, data, b".text\0\0\0"),
            0x8664,
            "sections are .text, .rsrc, .reloc",
        );

        let import = optional + 112 + 8;
        rejects(
            &patch(X64, import, &0x1000u32.to_le_bytes()),
            0x8664,
            "import directory",
        );
        rejects(
            &patch(X64, import + 4, &20u32.to_le_bytes()),
            0x8664,
            "import directory",
        );

        let image = Image::parse(X64).unwrap();
        let export_at = image.offset_of(image.directories[0].0).unwrap();
        rejects(
            &patch(X64, export_at + 16, &2u32.to_le_bytes()),
            0x8664,
            "ordinal base 2",
        );
        rejects(
            &patch(X64, export_at + 20, &2u32.to_le_bytes()),
            0x8664,
            "2 function(s)",
        );
        let name_rva = u32::from_le_bytes(
            X64[image
                .offset_of(u32::from_le_bytes(
                    X64[export_at + 32..export_at + 36].try_into().unwrap(),
                ))
                .unwrap()..][..4]
                .try_into()
                .unwrap(),
        );
        let name_at = image.offset_of(name_rva).unwrap();
        rejects(&patch(X64, name_at + 3, b"\0"), 0x8664, "[(\"Kbd\", 0)]");
        rejects(&patch(X64, name_at, b"X"), 0x8664, "XbdLayerDescriptor");
        rejects(
            &patch(X64, optional + 112, &[0; 8]),
            0x8664,
            "exports are []",
        );
    }

    // [spec:kbdgen:req:kbdl.image.verify/test]
    #[test]
    fn malformed_buffers_are_errors_not_panics() {
        rejects(b"", 0x8664, "truncated");
        rejects(b"ZM\0\0", 0x8664, "MZ");
        let mut header = vec![0u8; 0x40];
        header[..2].copy_from_slice(b"MZ");
        header[0x3c..0x40].copy_from_slice(&0x40u32.to_le_bytes());
        rejects(&header, 0x8664, "truncated");
        header.extend_from_slice(b"PE\0\0");
        header.resize(0x40 + 4 + 20 + 2, 0);
        rejects(&header, 0x8664, "magic 0x0");
        rejects(
            &patch(X64, 0x3c, &0xffff_fff0u32.to_le_bytes()),
            0x8664,
            "truncated",
        );
        for len in (0..X64.len()).step_by(97) {
            assert!(
                verify(&X64[..len], 0x8664).is_err(),
                "prefix of {len} bytes"
            );
        }
        let coff = pe(X64) + 4;
        rejects(
            &patch(X64, coff + 2, &0xffffu16.to_le_bytes()),
            0x8664,
            "truncated",
        );
        rejects(
            &patch(X64, coff + 20 + 108, &0xffffu32.to_le_bytes()),
            0x8664,
            "data directories",
        );
    }
}
