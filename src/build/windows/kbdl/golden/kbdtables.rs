//! Decodes the `KBDTABLES` that a built layout DLL's `KbdLayerDescriptor`
//! returns into text, following every pointer, so that a golden file pins
//! what Windows reads rather than the toolchain's code and section layout.

use std::fmt::Write as _;

use anyhow::{Context, Result, bail};

use super::super::image::{Image, Reader};

/// More entries than any terminated table of a layout has; a missing
/// terminator is an error rather than a runaway read.
const MAX_ENTRIES: usize = 1024;

const MACHINE_X86: u16 = 0x014c;
const MACHINE_X64: u16 = 0x8664;
const MACHINE_ARM64: u16 = 0xaa64;

/// A DLL image read at virtual addresses, with its tables' pointer size.
struct Memory<'a> {
    image: Image,
    reader: Reader<'a>,
    pointer: usize,
}

impl Memory<'_> {
    fn at(&self, va: u64) -> Result<usize> {
        let rva = va
            .checked_sub(self.image.image_base)
            .and_then(|rva| u32::try_from(rva).ok())
            .with_context(|| format!("address 0x{va:x} is outside the image"))?;
        self.image.offset_of(rva)
    }

    fn u8(&self, at: usize) -> Result<u8> {
        let byte = self.reader.bytes(at, 1)?;
        Ok(byte[0])
    }

    /// A table pointer: 4 bytes, or 8 in the 64-bit layout, where a WOW64
    /// DLL's upper half is zero.
    fn pointer(&self, at: usize) -> Result<u64> {
        let low = u64::from(self.reader.u32(at)?);
        if self.pointer == 4 {
            return Ok(low);
        }
        Ok(low | u64::from(self.reader.u32(at + 4)?) << 32)
    }

    fn units(&self, at: usize, count: usize) -> Result<Vec<u16>> {
        let mut units = Vec::with_capacity(count);
        for i in 0..count {
            units.push(self.reader.u16(at + 2 * i)?);
        }
        Ok(units)
    }

    /// The NUL-terminated UTF-16 string at `va`, quoted.
    fn string(&self, va: u64) -> Result<String> {
        let at = self.at(va)?;
        let mut units = Vec::new();
        for i in 0..MAX_ENTRIES {
            match self.reader.u16(at + 2 * i)? {
                0 => return Ok(format!("{:?}", String::from_utf16_lossy(&units))),
                unit => units.push(unit),
            }
        }
        bail!("the string at 0x{va:x} has no terminator")
    }

    /// The file offsets of the entries of size `size` at `va`, up to the
    /// first for which `end` holds.
    fn entries(
        &self,
        va: u64,
        size: usize,
        end: impl Fn(usize) -> Result<bool>,
    ) -> Result<Vec<usize>> {
        let start = self.at(va)?;
        let mut offsets = Vec::new();
        for i in 0..MAX_ENTRIES {
            let at = start + size * i;
            if end(at)? {
                return Ok(offsets);
            }
            offsets.push(at);
        }
        bail!("the table at 0x{va:x} has no terminator")
    }
}

fn hex_units(units: &[u16]) -> String {
    let mut out = String::new();
    for unit in units {
        if !out.is_empty() {
            out.push(' ');
        }
        let _ = write!(out, "{unit:04x}");
    }
    out
}

/// The address `KbdLayerDescriptor` returns, from its code: `lea rax`
/// on x64, `mov eax` (with `xor edx, edx` under WOW64) on x86, and
/// `adrp x0` + `add x0` on arm64.
fn descriptor(memory: &Memory, bytes: &[u8]) -> Result<u64> {
    let image = &memory.image;
    let rva = image
        .exports(bytes)?
        .first_function
        .context("no exported function")?;
    let at = image.offset_of(rva)?;
    let code = memory.reader.bytes(at, 12)?;
    let u32_at = |i: usize| u32::from_le_bytes([code[i], code[i + 1], code[i + 2], code[i + 3]]);
    match image.machine {
        MACHINE_X64 if code[..3] == [0x48, 0x8d, 0x05] => {
            let target = i64::from(rva) + 7 + i64::from(u32_at(3) as i32);
            Ok(image.image_base + u64::try_from(target)?)
        }
        MACHINE_X86 => match code[..5].iter().position(|byte| *byte == 0xb8) {
            Some(i) => Ok(u64::from(u32_at(i + 1))),
            None => bail!("x86 descriptor code {code:02x?} has no mov eax"),
        },
        MACHINE_ARM64 => {
            let (adrp, add) = (u32_at(0), u32_at(4));
            if adrp & 0x9f00_001f != 0x9000_0000 || add & 0xffc0_03ff != 0x9100_0000 {
                bail!("arm64 descriptor code {code:02x?} is not adrp x0 + add x0");
            }
            let pages = ((((adrp >> 5) & 0x7ffff) << 2 | (adrp >> 29) & 3) as i32) << 11 >> 11;
            let pages = i64::from(pages);
            let page = (i64::from(rva) & !0xfff) + (pages << 12);
            let target = page + i64::from((add >> 10) & 0xfff);
            Ok(image.image_base + u64::try_from(target)?)
        }
        machine => bail!("machine 0x{machine:04x}: unexpected descriptor code {code:02x?}"),
    }
}

/// `CharModifiers`: the modifier bits and the modification numbers.
fn modifiers(memory: &Memory, out: &mut String, va: u64) -> Result<()> {
    let p = memory.pointer;
    let at = memory.at(va)?;
    let bits = memory.pointer(at)?;
    let max = memory.reader.u16(at + p)?;
    let entries = memory.entries(bits, 2, |at| Ok(memory.u8(at)? == 0))?;
    let bits = entries
        .iter()
        .map(|at| {
            Ok(format!(
                "{:02x}:{:02x}",
                memory.u8(*at)?,
                memory.u8(at + 1)?
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    let numbers = (0..=usize::from(max))
        .map(|i| Ok(format!("{:02x}", memory.u8(at + p + 2 + i)?)))
        .collect::<Result<Vec<_>>>()?;
    writeln!(out, "CharModifiers: wMaxModBits {max}")?;
    writeln!(out, "  aVkToBits: {}", bits.join(" "))?;
    writeln!(out, "  ModNumber: {}", numbers.join(" "))?;
    Ok(())
}

/// `aVkToWcharTable`: each character table's rows as `vk attributes:
/// units`.
fn char_tables(memory: &Memory, out: &mut String, va: u64) -> Result<()> {
    let p = memory.pointer;
    writeln!(out, "aVkToWcharTable:")?;
    for table in memory.entries(va, 2 * p, |at| Ok(memory.pointer(at)? == 0))? {
        let rows = memory.pointer(table)?;
        let width = usize::from(memory.u8(table + p)?);
        let size = memory.u8(table + p + 1)?;
        writeln!(out, "  VK_TO_WCHARS<{width}> (cbSize {size}):")?;
        for row in memory.entries(rows, usize::from(size), |at| Ok(memory.u8(at)? == 0))? {
            let units = memory.units(row + 2, width)?;
            writeln!(
                out,
                "    {:02x} {:02x}: {}",
                memory.u8(row)?,
                memory.u8(row + 1)?,
                hex_units(&units)
            )?;
        }
    }
    Ok(())
}

/// `aDeadKey` as `dwBoth -> wchComposed flags`.
fn dead_keys(memory: &Memory, out: &mut String, va: u64) -> Result<()> {
    writeln!(out, "aDeadKey:")?;
    if va == 0 {
        return Ok(());
    }
    for entry in memory.entries(va, 8, |at| Ok(memory.reader.u32(at)? == 0))? {
        writeln!(
            out,
            "  {:08x} -> {:04x} flags {}",
            memory.reader.u32(entry)?,
            memory.reader.u16(entry + 4)?,
            memory.reader.u16(entry + 6)?
        )?;
    }
    Ok(())
}

/// A `VSC_LPWSTR` table as `scan code "name"`.
fn key_names(memory: &Memory, out: &mut String, title: &str, va: u64) -> Result<()> {
    let p = memory.pointer;
    writeln!(out, "{title}:")?;
    for entry in memory.entries(va, 2 * p, |at| Ok(memory.u8(at)? == 0))? {
        let name = memory.string(memory.pointer(entry + p)?)?;
        writeln!(out, "  {:02x} {name}", memory.u8(entry)?)?;
    }
    Ok(())
}

/// `aKeyNamesDead`: one string per dead character.
fn dead_key_names(memory: &Memory, out: &mut String, va: u64) -> Result<()> {
    writeln!(out, "aKeyNamesDead:")?;
    if va == 0 {
        return Ok(());
    }
    for entry in memory.entries(va, memory.pointer, |at| Ok(memory.pointer(at)? == 0))? {
        writeln!(out, "  {}", memory.string(memory.pointer(entry)?)?)?;
    }
    Ok(())
}

/// `ausVK`, sixteen virtual keys per line.
fn scancodes(memory: &Memory, out: &mut String, va: u64, count: usize) -> Result<()> {
    let units = memory.units(memory.at(va)?, count)?;
    writeln!(out, "ausVK ({count}):")?;
    for chunk in units.chunks(16) {
        writeln!(out, "  {}", hex_units(chunk))?;
    }
    Ok(())
}

/// A `VSC_VK` table as `scan code: virtual key`.
fn extended_scancodes(memory: &Memory, out: &mut String, title: &str, va: u64) -> Result<()> {
    writeln!(out, "{title}:")?;
    for entry in memory.entries(va, 4, |at| Ok(memory.u8(at)? == 0))? {
        writeln!(
            out,
            "  {:02x}: {:04x}",
            memory.u8(entry)?,
            memory.reader.u16(entry + 2)?
        )?;
    }
    Ok(())
}

/// `aLigature` as `vk column: units`.
fn ligatures(memory: &Memory, out: &mut String, va: u64, max: u8, size: u8) -> Result<()> {
    writeln!(out, "aLigature (nLgMax {max}, cbLgEntry {size}):")?;
    if va == 0 {
        return Ok(());
    }
    for entry in memory.entries(va, usize::from(size), |at| Ok(memory.u8(at)? == 0))? {
        let units = memory.units(entry + 4, usize::from(max))?;
        writeln!(
            out,
            "  {:02x} {}: {}",
            memory.u8(entry)?,
            memory.reader.u16(entry + 2)?,
            hex_units(&units)
        )?;
    }
    Ok(())
}

/// The descriptor of the layout DLL `bytes` and every table it points
/// to, as text. `wide` selects the 64-bit table layout, that of x64, arm64
/// and WOW64 DLLs.
pub(super) fn decode(bytes: &[u8], wide: bool) -> Result<String> {
    let memory = Memory {
        image: Image::parse(bytes)?,
        reader: Reader(bytes),
        pointer: if wide { 8 } else { 4 },
    };
    let p = memory.pointer;
    let at = memory.at(descriptor(&memory, bytes)?)?;
    let field = |i: usize| memory.pointer(at + i * p);
    let flags = 10 * p;
    let lg_max = memory.u8(at + flags + 4)?;
    let lg_size = memory.u8(at + flags + 5)?;
    let ligature = (flags + 6).next_multiple_of(p);
    let mut out = String::new();
    modifiers(&memory, &mut out, field(0)?)?;
    char_tables(&memory, &mut out, field(1)?)?;
    dead_keys(&memory, &mut out, field(2)?)?;
    key_names(&memory, &mut out, "aKeyNames", field(3)?)?;
    key_names(&memory, &mut out, "aKeyNamesExt", field(4)?)?;
    dead_key_names(&memory, &mut out, field(5)?)?;
    scancodes(
        &memory,
        &mut out,
        field(6)?,
        usize::from(memory.u8(at + 7 * p)?),
    )?;
    extended_scancodes(&memory, &mut out, "aE0VscToVk", field(8)?)?;
    extended_scancodes(&memory, &mut out, "aE1VscToVk", field(9)?)?;
    writeln!(
        out,
        "fLocaleFlags: 0x{:08x}",
        memory.reader.u32(at + flags)?
    )?;
    let ligature_va = memory.pointer(at + ligature)?;
    ligatures(&memory, &mut out, ligature_va, lg_max, lg_size)?;
    writeln!(
        out,
        "dwType {}, dwSubType {}",
        memory.reader.u32(at + ligature + p)?,
        memory.reader.u32(at + ligature + p + 4)?
    )?;
    Ok(out)
}
