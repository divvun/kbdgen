//! Checks the text service's PE files before they are written: the three
//! DLLs cargo builds and the Arm64X forwarder, including the Arm64EC view
//! that the forwarder's ARM64X dynamic relocations produce.

use anyhow::{Context, Result, bail};

use super::{ARCHITECTURES, EXPORTS, FORWARDER};
use crate::build::windows::kbdl::image::{Image, Reader};

const IMAGE_FILE_DLL: u16 = 0x2000;
const DIRECTORY_LOAD_CONFIG: usize = 10;
/// `IMAGE_DYNAMIC_RELOCATION_ARM64X`, the symbol of the dynamic relocation
/// group that turns an Arm64X image's native view into its Arm64EC view.
const DYNAMIC_RELOCATION_ARM64X: u64 = 6;
/// Offsets in `IMAGE_LOAD_CONFIG_DIRECTORY64`.
const CHPE_METADATA_POINTER: usize = 0xc8;
const DYNAMIC_RELOCATION_TABLE_OFFSET: usize = 0xe0;
const DYNAMIC_RELOCATION_TABLE_SECTION: usize = 0xe4;
/// DLLs of the C runtime, which `+crt-static` links into the image
/// instead; a text service loaded into every process must not need them.
const CRT_PREFIXES: [&str; 4] = ["vcruntime", "msvcp", "ucrtbase", "api-ms-win-crt-"];

fn u64_at(reader: &Reader, at: usize) -> Result<u64> {
    Ok(u64::from(reader.u32(at)?) | u64::from(reader.u32(at + 4)?) << 32)
}

/// The file offset of an RVA, which may lie in the headers.
fn file_offset(image: &Image, rva: u32) -> Result<usize> {
    let headers_end = image
        .sections
        .iter()
        .map(|section| section.virtual_address)
        .min()
        .unwrap_or(u32::MAX);
    if rva < headers_end {
        Ok(rva as usize)
    } else {
        image.offset_of(rva)
    }
}

/// The file offset of the load configuration, if the image has one large
/// enough to hold the CHPE and dynamic relocation fields.
fn load_config(image: &Image, bytes: &[u8]) -> Result<Option<usize>> {
    let (rva, _) = image.directory(DIRECTORY_LOAD_CONFIG);
    if rva == 0 {
        return Ok(None);
    }
    let at = image.offset_of(rva)?;
    let size = Reader(bytes).u32(at)? as usize;
    Ok((size >= DYNAMIC_RELOCATION_TABLE_SECTION + 2).then_some(at))
}

/// The RVA of the CHPE metadata the load configuration points at, or 0.
fn chpe_metadata(image: &Image, bytes: &[u8]) -> Result<u32> {
    let Some(config) = load_config(image, bytes)? else {
        return Ok(0);
    };
    let reader = Reader(bytes);
    let pointer = u64_at(&reader, config + CHPE_METADATA_POINTER)?;
    if pointer == 0 {
        return Ok(0);
    }
    let pe = reader.u32(0x3c)? as usize;
    let image_base = u64_at(&reader, pe + 24 + 24)?;
    u32::try_from(pointer.wrapping_sub(image_base))
        .ok()
        .with_context(|| format!("CHPE metadata pointer 0x{pointer:x} lies outside the image"))
}

/// Applies one block of ARM64X relocations, starting at `at`, to `view`.
/// Returns the block's size.
fn apply_block(image: &Image, reader: &Reader, at: usize, view: &mut [u8]) -> Result<usize> {
    let page = reader.u32(at)?;
    let size = reader.u32(at + 4)? as usize;
    if size < 8 {
        bail!("ARM64X relocation block at 0x{at:x} has size {size}");
    }
    let mut entry_at = at + 8;
    while entry_at < at + size {
        let entry = reader.u16(entry_at)?;
        entry_at += 2;
        if entry == 0 {
            continue;
        }
        let target = file_offset(image, page + u32::from(entry & 0xfff))?;
        let meta = entry >> 14;
        let value: Vec<u8> = match (entry >> 12) & 3 {
            0 => vec![0; 1 << meta],
            1 => {
                let value = reader.bytes(entry_at, 1 << meta)?.to_vec();
                entry_at += value.len();
                value
            }
            2 => {
                let delta = u64::from(reader.u16(entry_at)?) * if meta & 2 == 0 { 4 } else { 8 };
                entry_at += 2;
                let old = u64_at(&Reader(view), target)?;
                let new = if meta & 1 == 0 {
                    old.wrapping_add(delta)
                } else {
                    old.wrapping_sub(delta)
                };
                new.to_le_bytes().to_vec()
            }
            kind => bail!("unknown ARM64X relocation type {kind}"),
        };
        view.get_mut(target..target + value.len())
            .with_context(|| format!("ARM64X relocation at 0x{target:x} is past the end"))?
            .copy_from_slice(&value);
    }
    Ok(size)
}

/// The Arm64EC view of an Arm64X image: `bytes` with the ARM64X dynamic
/// relocations applied, as the loader applies them in an x64 or Arm64EC
/// process. `None` when the image has no such relocations.
pub fn ec_view(bytes: &[u8]) -> Result<Option<Vec<u8>>> {
    let image = Image::parse(bytes)?;
    let Some(config) = load_config(&image, bytes)? else {
        return Ok(None);
    };
    let reader = Reader(bytes);
    let offset = reader.u32(config + DYNAMIC_RELOCATION_TABLE_OFFSET)? as usize;
    let section = usize::from(reader.u16(config + DYNAMIC_RELOCATION_TABLE_SECTION)?);
    let Some(section) = section.checked_sub(1) else {
        return Ok(None);
    };
    let section = image
        .sections
        .get(section)
        .context("the dynamic relocation table names no section")?;
    let table = section.raw_offset as usize + offset;
    if reader.u32(table)? != 1 {
        bail!("dynamic relocation table version is not 1");
    }
    let end = table + 8 + reader.u32(table + 4)? as usize;
    let mut view = bytes.to_vec();
    let mut found = false;
    let mut at = table + 8;
    while at < end {
        let symbol = u64_at(&reader, at)?;
        let blocks = at + 12;
        let blocks_end = blocks + reader.u32(at + 8)? as usize;
        if symbol == DYNAMIC_RELOCATION_ARM64X {
            found = true;
            let mut block = blocks;
            while block < blocks_end {
                block += apply_block(&image, &reader, block, &mut view)?;
            }
        }
        at = blocks_end;
    }
    Ok(found.then_some(view))
}

/// Checks that `image` exports exactly the four COM entry points, each
/// forwarded to `module` when it is given, and none forwarded otherwise.
fn check_exports(image: &Image, bytes: &[u8], module: Option<&str>) -> Result<()> {
    let exports = image.exports(bytes)?;
    let names: Vec<&str> = exports
        .names
        .iter()
        .map(|(name, _)| name.as_str())
        .collect();
    if names != EXPORTS || exports.function_count != EXPORTS.len() as u32 {
        bail!(
            "exports are {names:?} with {} function(s), expected exactly {EXPORTS:?}",
            exports.function_count
        );
    }
    let expected: Vec<(String, String)> = module
        .map(|module| {
            EXPORTS
                .iter()
                .map(|name| ((*name).to_owned(), format!("{module}.{name}")))
                .collect()
        })
        .unwrap_or_default();
    if exports.forwards != expected {
        bail!(
            "forwarders are {:?}, expected {expected:?}",
            exports.forwards
        );
    }
    Ok(())
}

/// Checks a text service DLL cargo built: machine, DLL flag, exactly the
/// four COM exports and no C runtime DLL among its imports.
// [spec:kbdgen:req:tsf.arch.builds+1]
pub fn verify_tip(bytes: &[u8], machine: u16) -> Result<()> {
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
    check_exports(&image, bytes, None)?;
    let imports = image.imports(bytes)?;
    if let Some(crt) = imports.iter().find(|name| {
        let name = name.to_ascii_lowercase();
        CRT_PREFIXES.iter().any(|prefix| name.starts_with(prefix))
    }) {
        bail!("imports {crt}; the C runtime must be linked statically (+crt-static)");
    }
    Ok(())
}

/// Checks the Arm64X forwarder: an ARM64 DLL without entry point or
/// imports, with CHPE metadata, whose native view forwards the four COM
/// exports to the arm64 DLL and whose Arm64EC view, an x64 machine,
/// forwards them to the x64 DLL.
// [spec:kbdgen:req:tsf.arch.arm64x+1]
pub fn verify_forwarder(bytes: &[u8]) -> Result<()> {
    let [_, x64, arm64] = &ARCHITECTURES;
    let image = Image::parse(bytes)?;
    if image.machine != arm64.machine {
        bail!(
            "{FORWARDER}: machine is 0x{:04x}, expected ARM64",
            image.machine
        );
    }
    if image.characteristics & IMAGE_FILE_DLL == 0 || image.entry_point != 0 {
        bail!("{FORWARDER}: not a DLL without entry point");
    }
    if !image.imports(bytes)?.is_empty() {
        bail!("{FORWARDER}: has imports");
    }
    check_exports(&image, bytes, Some(arm64.module))
        .with_context(|| format!("{FORWARDER}: native view"))?;
    let Some(ec) = ec_view(bytes)? else {
        bail!("{FORWARDER}: has no ARM64X dynamic relocations, so it is plain ARM64");
    };
    let ec_image = Image::parse(&ec)?;
    if ec_image.machine != x64.machine {
        bail!(
            "{FORWARDER}: Arm64EC view has machine 0x{:04x}, expected x64",
            ec_image.machine
        );
    }
    let chpe = chpe_metadata(&ec_image, &ec)?;
    if chpe == 0 || Reader(&ec).u32(ec_image.offset_of(chpe)?)? == 0 {
        bail!("{FORWARDER}: Arm64EC view has no CHPE metadata");
    }
    check_exports(&ec_image, &ec, Some(x64.module))
        .with_context(|| format!("{FORWARDER}: Arm64EC view"))
}
