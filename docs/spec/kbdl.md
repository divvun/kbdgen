# Windows keyboard layout DLL

The Windows target builds, for each layout with Windows input, a keyboard
layout DLL from a generated `#![no_std]` Rust crate. kbdgen computes the
`KBDTABLES` tables of `kbd.h`, writes them as `repr(C)` statics, writes the
version and string resources as a binary `.res` itself, and links with `cargo`
and `rust-lld` for x86, x64, arm64 and the WOW64 variant installed to
`SysWOW64`. Building needs only a Rust toolchain with the three
`*-pc-windows-msvc` targets installed, on any host: no MSVC, Windows SDK,
MSKLC, `kbdutool` or `rc.exe`. These rules replace the MSKLC path (KLC
generation and `windows.dll*` in [windows.md](windows.md)) and the earlier
generated-C design; the user-visible layout semantics of KLC generation are
restated here in table terms (`kbdl.vk-chars.values`, `kbdl.caps`,
`kbdl.dead-keys`, `kbdl.ligatures`, `kbdl.metadata`), its known defects are
corrected, and the generator gains chained
dead keys, the 49th ISO key, extra modifier layers, locale flags, dead-key
names and localised key names. Text processing beyond what these tables can
express (multi-unit dead-key output, ligatures over 16 units) belongs to a
text service and is out of scope here.

The generator's input is described abstractly (`kbdl.input`), so that it can
be fed from the current layout model (`kbdl.input.bundle`) or from a future
bundle format.

In these rules a *cell* is one (key, column) slot of the character table; a
*unit* is one UTF-16 code unit; `WCH_NONE` = `0xF000`, `WCH_DEAD` =
`0xF001`, `WCH_LGTR` = `0xF002`; *N* is the number of character-table
columns (`kbdl.layers`). "Warn" means a `tracing` warning naming the layout
and, where applicable, the key position and layer; "fatal" means generation
fails with an error naming the same, and no output is written for that
layout. "Verified" marks behaviour observed on Windows 11 25H2 (build 26200)
x64 with a proof-of-concept crate built exactly as `kbdl.build` specifies,
on macOS arm64 (rustc 1.99.0 and 1.98.1) and on Windows x64 (rustc 1.98.1):
loaded with `LoadKeyboardLayout` and exercised with `ToUnicodeEx`,
`TranslateMessage`, `SendInput` into a text box, `GetKeyNameText` and
`SHLoadIndirectString`, and the WOW64 variant from a 32-bit process. arm64
DLLs were built and inspected but not loaded.

Sources:

- MSKLC 1.4 `inc/kbd.h` and `kbdutool.exe` v3.40 (from
  <https://download.microsoft.com/download/6/f/5/6f5ce43a-e892-4fd1-b9a6-1a0cbb64e6e2/MSKLC.exe>;
  kbdutool output was inspected for the Võro, Kildin Sami and Northern Sami
  KLCs and for probe KLCs exercising SGCap, dead keys and 3-unit ligatures,
  and its `.res` compared byte for byte with `kbdl.resources.format`)
- Microsoft Windows-driver-samples, `input/layout` (`kbdus`, `all_kbds/kbdfr`,
  `all_kbds/kbdgr`: `.c`, `.h`, `.def`, `.rc`, `.vcxproj`):
  <https://github.com/microsoft/Windows-driver-samples/tree/main/input/layout>
- ReactOS `sdk/include/ndk/kbd.h`:
  <https://github.com/reactos/reactos/blob/master/sdk/include/ndk/kbd.h>
- Wine `include/kbd.h` and `include/winuser.rh` (virtual-key values):
  <https://github.com/wine-mirror/wine/tree/master/include>
- Microsoft `windows` crate `KBDTABLES` binding:
  <https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/UI/Input/KeyboardAndMouse/struct.KBDTABLES.html>
- The tables of Windows 11's own `kbdcan.dll` (Canadian Multilingual
  Standard), `kbdfr.dll`, `kbdbr.dll`, `kbdgr.dll` and `kbdjpn.dll`, read
  through their `KbdLayerDescriptor`
- `VERSIONINFO` and string-table resource formats:
  <https://learn.microsoft.com/en-us/windows/win32/menurc/versioninfo-resource>,
  <https://learn.microsoft.com/en-us/windows/win32/menurc/resource-file-formats>

## Input

> [spec:kbdgen:def:kbdl.input]
> The input for one layout is: metadata (`kbdl.metadata`); a decimal
> separator; for each layer of `kbdl.layers` and each of the 49 positions
> `E00`…`B11` of `[spec:kbdgen:def:keys.iso-order]`, a *value*, either "no
> key" or a non-empty string of Unicode scalar values with a dead-key flag; up
> to three *extra modifiers*, ordered, each naming a distinct physical key
> `rightCtrl`, `capsLock` or `B00`; a *dead-key tree* mapping a dead-key
> output to a node, a node being a leaf (an output string) or a branch (an
> ordered map from an input string to a node), where the input `" "` is the
> branch's *standalone* output; dead-key names (dead-key output → name);
> key-name overrides (`kbdl.key-names` entry → name); and the flags
> `shiftLock` and `lrmRlm`. Absent parts are empty, flags false.

> [spec:kbdgen:req:kbdl.input.bundle+1]
> Until the bundle format carries the other parts, kbdgen MUST derive the
> input from a layout's `windows` section and leave extra modifiers, names and
> flags empty. Layers `default` to `ctrl` take tokens per
> `[spec:kbdgen:req:keys.iso-order.desktop-layers]`, so `B11` is "no key" in
> every layer. The token `\u{0}` (exactly) is "no key"; any other token is
> escape-decoded per `[spec:kbdgen:syn:keys.escape]`, and a result that is
> empty or starts with U+0000 warns and is "no key". A value is dead when the
> raw `windows.deadKeys[<layer>]` list contains its decoded string. The tree
> is `transforms` (`[spec:kbdgen:def:layout.transforms]`) with its strings
> used verbatim; a dead value's entry is the top-level key equal to its
> decoded string. The decimal separator is the layout's `decimal`. Where
> those rules panic (an escape outside the Unicode scalar values, a layer of
> fewer than 48 tokens), the adapter MUST instead fail with a fatal error.

## Crate

> [spec:kbdgen:def:kbdl.crate]
> For each layout, kbdgen writes the directory `<out>/build/<name>/`
> (`<name>` from `kbdl.metadata`) containing exactly `Cargo.toml`, `lib.rs`
> and `<name>.res` (`kbdl.resources`); builds use its `target/` and
> `target-wow64/` subdirectories. `Cargo.toml` MUST be, with LF line breaks
> and in this order: `[package]` with `name = "<name>"`, `version = "0.0.0"`,
> `edition = "2024"`, `publish = false`; `[lib]` with `path = "lib.rs"`,
> `crate-type = ["cdylib"]`; `[features]` with `wow64 = []`;
> `[profile.release]` with `opt-level = "s"`, `panic = "abort"`, `lto = true`,
> `codegen-units = 1`, `debug = false`, `incremental = false`; and an empty
> `[workspace]` table, so that an enclosing workspace is never used. There are
> no dependencies, no build script and no `.cargo` configuration.

> [spec:kbdgen:syn:kbdl.source]
> `lib.rs` is ASCII with LF line breaks and byte-for-byte deterministic for a
> given input (no timestamps, paths or tool versions). It contains, in order:
> `#![no_std]` and an `allow` of the `non_camel_case_types`,
> `non_snake_case`, `non_upper_case_globals` and `dead_code` lints; the
> pointer slot (`kbdl.structs.pointers`); the structures (`kbdl.structs`); the
> layout assertions (`kbdl.structs.layout`); the tables (`kbdl.tables`); the
> export (`kbdl.export`); and a `#[panic_handler]` that loops, which no code
> path reaches. It uses only `core`, and `unsafe` appears only in the `Sync`
> implementation and in `#[unsafe(no_mangle)]`.

> [spec:kbdgen:syn:kbdl.escaping]
> In `lib.rs` every UTF-16 unit is written as a lowercase hexadecimal integer
> literal `0x%04x`; `char`, string and byte-string literals MUST NOT carry
> layout text. Every string is a `[u16; n]` static of units terminated by
> `0x0000`, and supplementary-plane characters appear as their two surrogate
> units. Comments, if any, contain only ASCII and never input text. Virtual
> keys are written as `0x%02x`, `ausVK` and `aE0VscToVk` values as `0x%03x`,
> and attributes, flags and counts as their numeric value.

## Structures

> [spec:kbdgen:def:kbdl.structs]
> `lib.rs` defines these `#[repr(C)]` structures, named as in `kbd.h`, field
> for field in this order, with `P<T>` the pointer slot of
> `kbdl.structs.pointers` and variable-length members as const-generic arrays:
>
> | Struct | Fields |
> |---|---|
> | `VK_TO_BIT` | `Vk: u8, ModBits: u8` |
> | `MODIFIERS<const N: usize>` | `pVkToBit: P<VK_TO_BIT>, wMaxModBits: u16, ModNumber: [u8; N]` |
> | `VK_TO_WCHARS<const N: usize>` | `VirtualKey: u8, Attributes: u8, wch: [u16; N]` |
> | `VK_TO_WCHAR_TABLE` | `pVkToWchars: P<()>, nModifications: u8, cbSize: u8` |
> | `DEADKEY` | `dwBoth: u32, wchComposed: u16, uFlags: u16` |
> | `LIGATURE<const N: usize>` | `VirtualKey: u8, ModificationNumber: u16, wch: [u16; N]` |
> | `VSC_LPWSTR` | `vsc: u8, pwsz: P<u16>` |
> | `VSC_VK` | `Vsc: u8, Vk: u16` |
> | `KBDTABLES` | `pCharModifiers: P<()>, pVkToWcharTable: P<VK_TO_WCHAR_TABLE>, pDeadKey: P<DEADKEY>, pKeyNames: P<VSC_LPWSTR>, pKeyNamesExt: P<VSC_LPWSTR>, pKeyNamesDead: P<P<u16>>, pusVSCtoVK: P<u16>, bMaxVSCtoVK: u8, pVSCtoVK_E0: P<VSC_VK>, pVSCtoVK_E1: P<VSC_VK>, fLocaleFlags: u32, nLgMax: u8, cbLgEntry: u8, pLigature: P<()>, dwType: u32, dwSubType: u32` |
>
> No `packed` or `align` attribute is used except on the WOW64 slot; natural
> alignment applies.

> [spec:kbdgen:req:kbdl.structs.pointers]
> Every pointer field MUST be a `P<T>` slot. Without the `wow64` feature,
> `P<T>` is `#[repr(transparent)] struct P<T>(*const T)`. With it (32-bit x86
> code, 64-bit table layout, the counterpart of `kbd.h`'s `BUILD_WOW6432`), it
> MUST be `#[repr(C, align(8))] struct P<T>(*const T, u32)`, initialised as
> `(address, 0)`, so its bytes equal the zero-extended little-endian 64-bit
> address. Slots are built only by one `const fn new(*const T)` and a null
> constant, so table initialisers are textually identical across variants.
> Raw pointers are not `Sync`, so `lib.rs` MUST declare
> `unsafe impl<T> Sync for P<T> {}`, which is sound because every table is
> immutable and free of interior mutability. Building with `wow64` for a
> non-x86 target MUST fail through `compile_error!`.

> [spec:kbdgen:thm:kbdl.structs.layout]
> Under `kbdl.structs` and `kbdl.structs.pointers`, sizes and offsets in bytes
> are (x86 / x64, arm64 and WOW64): `P<T>` 4 / 8; `KBDTABLES` 60 / 104, with
> `bMaxVSCtoVK` at 28 / 56, `pVSCtoVK_E0` 32 / 64, `fLocaleFlags` 40 / 80,
> `nLgMax` 44 / 84, `pLigature` 48 / 88, `dwType` 52 / 96; `MODIFIERS` has
> `wMaxModBits` at 4 / 8; `VK_TO_WCHAR_TABLE` and `VSC_LPWSTR` 8 / 16;
> independent of pointer size, `DEADKEY` 8, `VSC_VK` 4, `VK_TO_WCHARS<n>`
> 2+2n and `LIGATURE<n>` 4+2n. `lib.rs` MUST assert each of these in a
> `const _: () = { … };` block using `assert!`, `size_of` and `offset_of!`,
> so a layout mismatch fails compilation.

> [spec:kbdgen:req:kbdl.export]
> `lib.rs` MUST define
> `#[unsafe(no_mangle)] pub extern "system" fn KbdLayerDescriptor()`
> returning `&KbdTables` as `*const KBDTABLES`, or, with the `wow64` feature,
> that address zero-extended as `u64`. rustc exports it under exactly this
> name on every target, including x86, where the `stdcall` symbol
> `_KbdLayerDescriptor@0` is exported through an `EXPORTAS` directive. No
> `.def` file is used: rust-lld ignores a `/DEF` given after rustc's own, so
> it could neither rename nor renumber the export (verified). The sole export
> has ordinal 1, and the DLL has no entry point.

## Descriptor and tables

> [spec:kbdgen:def:kbdl.tables]
> The single `KBDTABLES` static `KbdTables` holds, in field order:
> `&CharModifiers` (`kbdl.modifiers`); `aVkToWcharTable` (`kbdl.vk-chars`);
> `aDeadKey` or null (`kbdl.dead-keys.table`); `aKeyNames`; `aKeyNamesExt`
> (`kbdl.key-names`); `aKeyNamesDead` or null (`kbdl.dead-keys.names`);
> `ausVK`; `127`; `aE0VscToVk`; `aE1VscToVk` (`kbdl.scancodes`,
> `kbdl.scancodes.extended`); the locale flags (`kbdl.locale`); `nLgMax`,
> `cbLgEntry` and `aLigature`, or `0`, `0` and null (`kbdl.ligatures`); then
> `dwType` `0` and `dwSubType` `0`. Every table is a private, immutable
> `static` (never `const`, which has no single address, nor `static mut`);
> only `KbdLayerDescriptor` is public.

> [spec:kbdgen:thm:kbdl.tables.invariants]
> For every generated layout: each `VK_TO_WCHARS` row containing `WCH_DEAD`
> is immediately followed by a row with `VirtualKey` `0xff`; every dead id
> emitted in a dead row or as the `wchComposed` of a `DKF_DEAD` entry has
> exactly one group in `aDeadKey`; no two `aDeadKey` entries share `dwBoth`;
> each `WCH_LGTR` cell has exactly one `aLigature` entry with its virtual key
> and modification number, and every entry has such a cell; `nLgMax` is
> between 2 and 16 when there are ligatures; every `ModNumber` entry is
> `0x0f` or less than *N*; and no output unit (cell, ligature unit,
> `wchComposed`, dead id) lies in `0xF000`–`0xF002`. These follow from
> `kbdl.vk-chars.values`, `kbdl.dead-keys.table`, `kbdl.dead-keys.chains`,
> `kbdl.ligatures` and `kbdl.modifiers`.

## Modifiers and columns

> [spec:kbdgen:def:kbdl.layers]
> The layers are `default`, `shift`, `ctrl`, `alt` (AltGr, i.e. Ctrl+Alt),
> `alt+shift`, `caps`, `caps+shift`, `alt+caps`, and, for the *i*-th extra
> modifier *m* (from 0), *m* and *m*`+shift`. Character-table columns are:
> `default` 0, `shift` 1, `ctrl` 2, `alt` 3, `alt+shift` 4, *m* 5+2*i* and
> *m*`+shift` 6+2*i*, so *N* is 5 plus twice the number of extra modifiers
> (at most 11). `caps` and `caps+shift` occupy columns 0 and 1 of an SGCAPS
> row (`kbdl.caps.sgcaps`), and `alt+caps` only influences attributes
> (`kbdl.caps`). A column's index is also the ligature modification number. A
> layer absent from the input yields `WCH_NONE` in every cell of its column.

> [spec:kbdgen:req:kbdl.modifiers]
> `aVkToBits` MUST be `{0x10, 0x01}`, `{0x11, 0x02}`, `{0x12, 0x04}` (Shift,
> Control, Menu), then `{v_i, 0x08 << i}` for the *i*-th extra modifier with
> `v` = `0xdf` (`VK_OEM_8`), `0xe8`, `0x97` (unassigned virtual keys), then
> `{0, 0}`. `wMaxModBits` MUST be `7` without extra modifiers, else
> `(0x08 << (n-1)) | 1` for *n* of them; `ModNumber` has `wMaxModBits + 1`
> entries, all `0x0f` (`SHFT_INVALID`) except 0 → 0, 1 → 1, 2 → 2, 6 → 3,
> 7 → 4, `0x08 << i` → 5+2*i* and `(0x08 << i) | 1` → 6+2*i*. Without extra
> modifiers that is `{0, 1, 2, 0x0f, 0x0f, 0x0f, 3, 4}`. With one, the
> binding matches Windows' own `kbdcan.dll`; all three bits select their
> columns (verified).

> [spec:kbdgen:req:kbdl.scancodes.extra-modifiers]
> Each extra modifier's physical key MUST be rebound to its virtual key `v_i`
> (`kbdl.modifiers`): `rightCtrl` by setting the `aE0VscToVk` value for scan
> code `1d` to `v_i | 0x100` (`KBDEXT`, as `kbdcan.dll` does), `capsLock` by
> setting `ausVK[0x3a]` to `v_i`, and `B00` by setting `ausVK[0x56]` to `v_i`.
> A rebound `B00` has no character row, and values given for it warn and are
> ignored; a rebound `capsLock` no longer toggles Caps Lock. Caps Lock never
> affects extra-modifier columns, even in `CAPLOK` rows. (Verified by typing
> through `SendInput`: each of the three keys selects its columns, with and
> without Shift, alongside AltGr.)

## Character tables

> [spec:kbdgen:sem:kbdl.vk-chars.values]
> A value whose UTF-16 encoding is one unit is a character, or a dead key
> when flagged (`kbdl.dead-keys`). A value of 2–16 units is a ligature
> (`kbdl.ligatures`); a dead-key flag on it warns and is ignored. A value of
> more than 16 units (`kbdl.ligatures.limit`) warns and is "no key". A value
> containing a unit in `0xF000`–`0xF002` is fatal. "No key" is `WCH_NONE`.
> A lone supplementary-plane character is therefore a 2-unit ligature. Each
> cell holds exactly one of: a character, `WCH_DEAD`, `WCH_LGTR` or
> `WCH_NONE`.

> [spec:kbdgen:req:kbdl.vk-chars]
> `aVkToWch<N>` (type `VK_TO_WCHARS<N>`) MUST contain, in this order: one row
> per `kbdl.scancodes.iso` position in table order, except a rebound `B00`
> and a `B11` whose values are all "no key", each followed by its `0xff`
> dead row (`kbdl.dead-keys`) or SGCAPS row (`kbdl.caps.sgcaps`) when
> present; the space row `{0x20, 0, 0x0020, 0x0020, 0x0020, WCH_NONE, …}`;
> the decimal row `{0x6e, 0, d, d, WCH_NONE, …}`; and an all-zero
> terminator. A row's `Attributes` come from `kbdl.caps`. Every other position
> is emitted even if all its cells are `WCH_NONE`. `aVkToWcharTable` MUST list
> `aVkToWch3` (3), `aVkToWch<N>` (*N*), `aVkToWch2` (2), `aVkToWch1` (1), each
> with `cbSize` = `2 + 2·nModifications`, then a null entry; the numpad table
> comes last so `VkKeyScan` prefers main-block digits.

> [spec:kbdgen:def:kbdl.vk-chars.fixed]
> The fixed tables, each terminated by an all-zero row and with all
> `Attributes` `0`, are: `aVkToWch3` = `0x08` (Back) `{0x0008, 0x0008,
> 0x007f}`, `0x1b` (Escape) `{0x001b, 0x001b, 0x001b}`, `0x0d` (Return)
> `{0x000d, 0x000d, 0x000a}`, `0x03` (Cancel) `{0x0003, 0x0003, 0x0003}`;
> `aVkToWch2` = `0x09` (Tab) `{0x0009, 0x0009}`, `0x6b` `{0x002b, 0x002b}`,
> `0x6f` `{0x002f, 0x002f}`, `0x6a` `{0x002a, 0x002a}`, `0x6d` `{0x002d,
> 0x002d}`; `aVkToWch1` = `0x60`–`0x69` (Numpad 0–9) producing
> `0x0030`–`0x0039`.

> [spec:kbdgen:req:kbdl.vk-chars.decimal]
> The decimal character `d` MUST be the first Unicode scalar value of the
> decimal separator, or `.` when absent. An empty separator, or a first
> scalar outside the Basic Multilingual Plane or in `0xF000`–`0xF002`, is
> fatal. Any scalar after the first warns and is ignored.

## Caps lock

> [spec:kbdgen:sem:kbdl.caps]
> A row's `Attributes` are computed from the values of `default` (D),
> `shift` (S), `caps` (C), `alt` (A), `alt+shift` (AS) and `alt+caps` (AC);
> two values are equal when both are "no key" or they have equal strings and
> equal dead-key flags. If C is present and differs from both D and S, the
> attribute is `SGCAPS` (`0x02`). Otherwise, if C is absent, it is `CAPLOK`
> (`0x01`) when D ≠ S plus `CAPLOKALTGR` (`0x04`) when A ≠ AS; if C is
> present, `CAPLOK` when C = S plus `CAPLOKALTGR` when AC = AS. Space,
> decimal, dead (`0xff`), SGCAPS-follow and fixed rows have `Attributes` `0`.

> [spec:kbdgen:req:kbdl.caps.sgcaps]
> A row with `SGCAPS` MUST be followed by a row with the same virtual key,
> `Attributes` `0`, `wch[0]` = `caps`, `wch[1]` = `caps+shift`, and
> `WCH_NONE` in columns 2 to *N*−1. A dead key in either cell MUST be emitted
> as its plain character, and a ligature as `WCH_NONE`, each with a warning
> (Windows has no representation for either; a ligature would collide with
> the main row's modification numbers 0 and 1). If the main row has a dead
> key, SGCAPS cannot be expressed (both need the following row): the key MUST
> warn and get the attributes computed as if `caps` were absent, with no
> SGCAPS row.

## Dead keys

> [spec:kbdgen:req:kbdl.dead-keys+1]
> A one-unit value flagged dead MUST make its cell `WCH_DEAD`, and its row
> MUST be followed by a row with `VirtualKey` `0xff`, `Attributes` `0`, the
> dead id of that dead character in each `WCH_DEAD` column and `WCH_NONE` in
> every other column. The *key dead characters* of a layout are the distinct
> dead units emitted, ordered by first emission (rows in table order, columns
> 0 to *N*−1). Each key dead character, and each chained state
> (`kbdl.dead-keys.chains`), is a *dead state*, identified by one unit, its
> *dead id*; on a key with no entry in the state's group, Windows emits the
> dead id, then that key's character. Key dead characters are given ids in
> order, before any chained state. A key dead character's id MUST be the
> standalone output of its tree branch when that is exactly one unit, outside
> `0xF000`–`0xF002` and not already a dead id, so that an unmatched key emits
> the standalone output followed by the key, as on macOS; otherwise it MUST
> be the dead character itself when that is not already a dead id, and
> otherwise the fallback unit of `kbdl.dead-keys.chains`, each with a warning
> naming the dead key. Windows also reports the dead id as the dead key's
> character to `ToUnicodeEx`, `WM_DEADCHAR` and `GetKeyNameText`, and
> `VkKeyScan` finds the dead key under its id rather than under the dead
> character, ahead of any later key producing the same character (verified
> with a standalone output differing from the dead character).

> [spec:kbdgen:req:kbdl.dead-keys.table]
> `aDeadKey` MUST contain one group of entries per dead state: first the key
> dead characters in order, then the chained states in allocation order. A
> group has one entry per child of the state's tree branch, in input order
> with the standalone child last, each `{base | id << 16, composed, flags}`
> (`kbd.h` `DEADTRANS(base, id, composed, flags)`) where `base` is the
> child's input unit; for a leaf, `composed` is its output unit and `flags`
> `0`; for a branch, `composed` is that chained state's dead id and `flags`
> `DKF_DEAD` (`0x0001`). Each dead state has exactly one group however many
> keys or layers reach it. The table ends with an all-zero entry.

> [spec:kbdgen:req:kbdl.dead-keys.chains]
> A branch reached as the child of a branch is a *chained state*: after its
> input Windows keeps the chained dead id pending, to any depth. Chained
> states are allocated depth-first, groups in `kbdl.dead-keys.table` order and
> children in input order. A chained state's dead id MUST be its
> standalone output when that is one unit, outside `0xF000`–`0xF002` and not
> already a dead id; otherwise the highest unit from `0xF8FF` downward that is
> neither a dead id nor emitted anywhere in the tables, with a warning naming
> the input path, and fatal if none remains. On an input with no entry,
> Windows emits the pending dead id, then that input's character. (Verified
> for depths 2 and 3, through plain and dead keys, with that fallback.)

> [spec:kbdgen:req:kbdl.dead-keys.diagnostics]
> It is fatal for a key dead character to have no tree entry or a leaf
> entry, and for any branch, at any depth, to lack a standalone child or to
> have a branch as its standalone child. A child whose input is not exactly
> one unit, or which is a leaf whose output is not exactly one unit, or
> either of which lies in `0xF000`–`0xF002`, MUST warn (naming the input path
> and output) and be omitted; this includes outputs needing a combining mark
> (`b` → `b́`). Tree entries for dead-key outputs that no key emits are
> ignored. None of these conditions may panic.

> [spec:kbdgen:req:kbdl.dead-keys.names+1]
> `aKeyNamesDead` MUST hold, for each key dead character in order that has a
> dead-key name, a slot pointing to a zero-terminated unit array of the dead
> character's dead id (`kbdl.dead-keys`), which is what Windows matches,
> followed by the name's units (`kbd.h` style `L"^" L"CIRCUMFLEX"`), then a
> null slot; `pKeyNamesDead` is null when no entry is emitted. A name for an
> output that is not a key dead character, and an empty name or one
> containing U+0000, warn and are ignored. `GetKeyNameText` then returns the
> name for a key whose `default` value is that dead key (verified with a
> dead id equal to the dead character).

## Ligatures

> [spec:kbdgen:req:kbdl.ligatures]
> Each ligature cell MUST be `WCH_LGTR` and add one `aLigature` entry
> `{virtual key, column, units…}`, in emission order (rows in table order,
> columns 0 to *N*−1). With `n` the longest ligature's unit count (2–16),
> `aLigature` MUST have type `LIGATURE<n>`, shorter entries MUST be padded
> with `WCH_NONE`, and the table ends with an all-zero entry; `nLgMax` =
> `n` and `cbLgEntry` = `4 + 2n`. Ligatures never occur in SGCAPS follow
> rows (`kbdl.caps.sgcaps`), so (virtual key, column) is unique. Without
> ligatures, `nLgMax` and `cbLgEntry` are `0` and `pLigature` is null.

> [spec:kbdgen:thm:kbdl.ligatures.limit]
> Windows delivers at most 16 UTF-16 units per keystroke: typed input
> (`SendInput`, `TranslateMessage`) truncates a longer ligature to its first
> 16 units, although `ToUnicodeEx` with a large buffer returns up to the 125
> units that a byte-sized `cbLgEntry` (`4 + 2n ≤ 255`) allows. MSKLC's limit
> of 4 is a tool limit, not a Windows one. Hence `kbdl.vk-chars.values` caps
> ligatures at 16 units. (Verified with ligatures of 4, 5, 8, 15, 16, 17, 31,
> 32, 64 and 125 units; Shift and extra-modifier columns behave alike.)

## Scan codes

> [spec:kbdgen:def:kbdl.scancodes]
> `ausVK` is the 127-entry (`0x00`–`0x7e`) `KBD_TYPE` 4 table of `kbd.h`
> (`T00`…`T7E`) with its flags (`KBDEXT` `0x100`, `KBDMULTIVK` `0x200`,
> `KBDSPECIAL` `0x400`, `KBDNUMPAD` `0x800`), i.e. rows of 16 from `0x00`,
> before the overrides of `kbdl.scancodes.extra-modifiers`:
>
> `0ff 01b 031 032 033 034 035 036 037 038 039 030 0bd 0bb 008 009`
> `051 057 045 052 054 059 055 049 04f 050 0db 0dd 00d 0a2 041 053`
> `044 046 047 048 04a 04b 04c 0ba 0de 0c0 0a0 0dc 05a 058 043 056`
> `042 04e 04d 0bc 0be 0bf 1a1 26a 0a4 020 014 070 071 072 073 074`
> `075 076 077 078 079 390 291 c24 c26 c21 06d c25 c0c c27 06b c23`
> `c28 c22 c2d c2e 02c 0ff 0e2 07a 07b 00c 0ee 0f1 0ea 0f9 0f5 0f3`
> `0ff 0ff 0fb 02f 07c 07d 07e 07f 080 081 082 083 084 085 086 0ed`
> `0ff 0e9 0ff 0c1 0ff 0ff 087 0ff 0ff 0ff 0ff 0eb 009 0ff 0c2`

> [spec:kbdgen:def:kbdl.scancodes.extended]
> `aE0VscToVk` is, in order, (scan code → value), before the overrides of
> `kbdl.scancodes.extra-modifiers`: `10`→`1b1`, `19`→`1b0`, `1d`→`1a3`,
> `20`→`1ad`, `21`→`1b7`, `22`→`1b3`, `24`→`1b2`, `2e`→`1ae`, `30`→`1af`,
> `32`→`1ac`, `35`→`16f`, `37`→`12c`, `38`→`1a5`, `47`→`124`, `48`→`126`,
> `49`→`121`, `4b`→`125`, `4d`→`127`, `4f`→`123`, `50`→`128`, `51`→`122`,
> `52`→`12d`, `53`→`12e`, `5b`→`15b`, `5c`→`15c`, `5d`→`15d`, `5f`→`15f`,
> `65`→`1aa`, `66`→`1ab`, `67`→`1a8`, `68`→`1a9`, `69`→`1a7`, `6a`→`1a6`,
> `6b`→`1b6`, `6c`→`1b4`, `6d`→`1b5`, `1c`→`10d`, `46`→`103`, then
> `{0, 0}`. `aE1VscToVk` is `{0x1d, 0x13}` (Pause), then `{0, 0}`. All
> values are hexadecimal.

> [spec:kbdgen:req:kbdl.scancodes.iso]
> The character table MUST map the 49 positions `E00`…`B11` of
> `[spec:kbdgen:def:keys.iso-order]` to these virtual keys, which equal
> `ausVK` at the listed scan code: `E00` (`29`) `0xc0`; `E01`–`E10`
> (`02`–`0b`) `'1'`–`'9'`, `'0'`; `E11` (`0c`) `0xbd`; `E12` (`0d`) `0xbb`;
> `D01`–`D10` (`10`–`19`) `QWERTYUIOP`; `D11` (`1a`) `0xdb`; `D12` (`1b`)
> `0xdd`; `C01`–`C09` (`1e`–`26`) `ASDFGHJKL`; `C10` (`27`) `0xba`; `C11`
> (`28`) `0xde`; `C12` (`2b`) `0xdc`; `B00` (`56`) `0xe2`; `B01`–`B07`
> (`2c`–`32`) `ZXCVBNM`; `B08` (`33`) `0xbc`; `B09` (`34`) `0xbe`; `B10`
> (`35`) `0xbf`; `B11` (`73`) `0xc1` (`VK_ABNT_C1`, verified). Letters and
> digits are their ASCII codes.

## Key names

> [spec:kbdgen:req:kbdl.key-names]
> `aKeyNames` MUST map scan codes to: `01` Esc, `0e` Backspace, `0f` Tab,
> `1c` Enter, `1d` Ctrl, `2a` Shift, `36` Right Shift, `37` Num \*, `38` Alt,
> `39` Space, `3a` Caps Lock, `3b`–`44` F1–F10, `45` Pause, `46` Scroll Lock,
> `47` Num 7, `48` Num 8, `49` Num 9, `4a` Num -, `4b` Num 4, `4c` Num 5,
> `4d` Num 6, `4e` Num +, `4f` Num 1, `50` Num 2, `51` Num 3, `52` Num 0,
> `53` Num Del, `54` Sys Req, `57` F11, `58` F12, `7c`–`87` F13–F24.
> `aKeyNamesExt` MUST map: `1c` Num Enter, `1d` Right Ctrl, `35` Num /, `37`
> Prnt Scrn, `38` Right Alt, `45` Num Lock, `46` Break, `47` Home, `48` Up,
> `49` Page Up, `4b` Left, `4d` Right, `4f` End, `50` Down, `51` Page Down,
> `52` Insert, `53` Delete, `54` `<00>`, `56` Help, `5b` Left Windows, `5c`
> Right Windows, `5d` Application. Both are in this order and end with
> `{0, null}`. A key-name override replaces the name of the entry with its
> table and scan code (verified with a non-ASCII name); an override naming no
> entry, or empty, or containing U+0000, warns and is ignored.

## Locale and metadata

> [spec:kbdgen:req:kbdl.locale]
> `fLocaleFlags` MUST be `KBD_VERSION` (`1`) in the high word and, in the low
> word, the OR of `KLLF_ALTGR` (`0x0001`) when any cell of columns 3 or 4 is
> not `WCH_NONE`, `KLLF_SHIFTLOCK` (`0x0002`) when `shiftLock` is set, and
> `KLLF_LRM_RLM` (`0x0004`) when `lrmRlm` is set. (Verified: while a layout
> with `KLLF_SHIFTLOCK` is active, Caps Lock only sets the lock and Shift
> releases it, whereas a layout without it keeps normal toggling; with
> `KLLF_LRM_RLM`, left Shift+Backspace types U+200E and right
> Shift+Backspace U+200F.)

> [spec:kbdgen:req:kbdl.metadata]
> Each layout has a keyboard name, a description, a language name, a locale
> name, an LCID, a company, a copyright, and a version and build. The keyboard
> name names the crate, the DLL and its resources; a name that does not start
> with an ASCII letter or contains anything but ASCII letters, digits, `-` and
> `_` is fatal, as are two layouts with the same name. A metadata string
> containing U+0000 is fatal.

> [spec:kbdgen:req:kbdl.metadata.bundle]
> From the current bundle, the keyboard name MUST be `kbd` followed by
> `windows.config.id` if set, otherwise by the first five Unicode scalar
> values of the language tag (`kbdse-FI`, `kbdsjd-C`). The description and
> the language name MUST both be the layout's `displayNames` entry for its
> primary language subtag; the company and copyright MUST be the project's
> `organisation` and `copyright`; the version and build are the `windows`
> target's `version` and `build`.

> [spec:kbdgen:req:kbdl.metadata.locale]
> The LCID MUST be looked up from the layout's own language tag, never from
> `windows.config.locale`: the primary subtag is mapped to ISO 639-3 and the
> LCID record whose (639-3 code, script, region) exactly matches the tag's
> subtags is used, an absent subtag matching only an absent field; with no
> match the LCID is `0x2000`. The locale name MUST be `windows.config.locale`
> if set; otherwise the full language tag if a record matched; otherwise
> `<primary>-<script or Latn>-<region or 001>`.

## Resources

> [spec:kbdgen:req:kbdl.resources]
> `<name>.res` MUST hold the version resource of `kbdl.resources.version` and
> `kbdl.resources.fields`, and string tables with `1000` = description and
> `1100` = language name under language `LCID & 0xffff`, and `1200` = locale
> name under language `0x0409`. String `1000` is what the registry's
> `Layout Display Name` (`@%SystemRoot%\system32\<name>.dll,-1000`) resolves
> (verified with `SHLoadIndirectString`, also from a 32-bit process against
> the WOW64 DLL). The version `a.b.c.d` takes `a.b.c` from up to three
> dot-separated components of the version (missing ones 0) and `d` from the
> build; when the version is absent or a component is not a decimal 0–65535,
> it warns and uses `1.0.0.0`.

> [spec:kbdgen:syn:kbdl.resources.format]
> A `.res` file is a sequence of entries, integers little-endian, each a
> 32-byte header — `DataSize` u32 (unpadded), `HeaderSize` u32 `32`, type
> u16 `0xffff` + u16 id, name u16 `0xffff` + u16 id, `DataVersion` u32 `0`,
> `MemoryFlags` u16, `LanguageId` u16, `Version` u32 `0`, `Characteristics`
> u32 `0` — then the data zero-padded to a multiple of 4. kbdgen MUST write:
> an empty entry (`DataSize`, ids, flags and language 0); `RT_VERSION` (16),
> name 1, language `0x0409`, flags `0x0030`; then each `RT_STRING` (6) block
> in ascending (block, language) order with flags `0x1030`. String `s` lives
> in block `(s >> 4) + 1` at index `s & 15`; block data is 16 × (u16 unit
> count, units without terminator), empty slots having count 0.

> [spec:kbdgen:syn:kbdl.resources.version]
> A version node is `wLength` u16, `wValueLength` u16, `wType` u16 (`0`
> binary, `1` text), its key as zero-terminated UTF-16, zero padding to a
> 4-byte boundary, its value, then, if it has children, padding to 4 bytes
> and the children, each starting 4-byte aligned. `wLength` counts from the
> node's start to the end of its value or last child, excluding trailing
> padding. The tree is `VS_VERSION_INFO` (binary, value `VS_FIXEDFILEINFO`,
> `wValueLength` 52) with children `StringFileInfo` → `000004B0` → the
> strings, and `VarFileInfo` → `Translation` (binary, value u16 `0x0000`, u16
> `0x04b0`, `wValueLength` 4). Strings are text nodes with `wValueLength` the
> unit count including the terminator; blocks are text nodes with
> `wValueLength` 0. This reproduces `rc.exe`'s output byte for byte.

> [spec:kbdgen:def:kbdl.resources.fields]
> `VS_FIXEDFILEINFO` is 13 u32: `0xfeef04bd`, `0x00010000`, file version MS
> (`a << 16 | b`), file version LS (`c << 16 | d`), product version MS and LS
> (equal to the file version), flags mask `0x3f`, flags `0`, OS `0x00040004`,
> type `2` (`VFT_DLL`), subtype `2` (`VFT2_DRV_KEYBOARD`, as `kbdutool`
> writes), date `0`, `0`. The strings are, in order: `CompanyName`,
> `FileDescription` (`<description> Keyboard Layout`), `FileVersion`
> (`a.b.c.d`), `InternalName` (`<name>`), `LegalCopyright`,
> `OriginalFilename` (`<name>.dll`), `ProductName` (`<description>`) and
> `ProductVersion` (`a.b.c.d`).

## Build

> [spec:kbdgen:req:kbdl.build.toolchain]
> Before building, kbdgen MUST check that `cargo` and `rustc` start and that,
> for each of `i686-pc-windows-msvc`, `x86_64-pc-windows-msvc` and
> `aarch64-pc-windows-msvc`, the directory printed by
> `rustc --print target-libdir --target <triple>` exists; otherwise building
> is fatal with a message naming what is missing and the fix
> (`rustup target add <triple>`). The toolchain must support edition 2024
> (Rust 1.85 or later). `rust-lld` ships with the toolchain; MSVC, the
> Windows SDK and C tools are not used, and the host may run any operating
> system (verified on macOS arm64 and on Windows x64 without MSVC on `PATH`).

> [spec:kbdgen:req:kbdl.build]
> For each layout generated in this run and each variant `x86`
> (`i686-pc-windows-msvc`), `x64` (`x86_64-pc-windows-msvc`), `arm64`
> (`aarch64-pc-windows-msvc`) and `wow64` (`i686-pc-windows-msvc`,
> `--features wow64`, directory `target-wow64`, otherwise `target`), kbdgen
> MUST run in the crate directory `cargo rustc --release --lib --target
> <triple> [--features wow64] --target-dir <crate>/<directory> -- -C
> linker=rust-lld`, then `-C link-arg=X` for each X of `/NOENTRY`,
> `/NODEFAULTLIB`, `/SUBSYSTEM:NATIVE`, the absolute path of `<name>.res`,
> `/MERGE:.rdata=.data`, `/MERGE:.text=.data`, `/MERGE:.bss=.data`,
> `/SECTION:.data,RE`, `/DEBUG:NONE` and `/Brepro`; and copy
> `<directory>/<triple>/release/<lib>.dll`, `<lib>` being `<name>` with `-`
> replaced by `_`, to `<out>/<variant>/<name>.dll`. A cargo failure is fatal,
> naming the layout, variant and exit status.

> [spec:kbdgen:req:kbdl.build.environment]
> kbdgen MUST remove `RUSTFLAGS`, `CARGO_ENCODED_RUSTFLAGS`,
> `CARGO_BUILD_RUSTFLAGS`, `CARGO_BUILD_TARGET` and every `CARGO_TARGET_*`
> and `CARGO_PROFILE_*` variable from cargo's environment, since they would
> add to or replace the link arguments or the profile (verified: an inherited
> `RUSTFLAGS` reaches the linker). rustc itself passes `/DLL`,
> `/OPT:REF,ICF`, `/DEBUG` and `/defaultlib:msvcrt`, hence `/DEBUG:NONE` (no
> PDB) and `/NODEFAULTLIB` (no `msvcrt.lib`, which non-Windows hosts lack);
> `/NOENTRY` because there is no `DllMain`; `/Brepro` replaces the timestamp
> with a content hash. Identical crates then gave byte-identical DLLs on
> macOS and Windows hosts and with rustc 1.98.1 and 1.99.0.

## Image

> [spec:kbdgen:req:kbdl.image]
> Every built DLL MUST place `KbdTables`, every table and string it reaches,
> and the code of `KbdLayerDescriptor` in one section named `.data` that is
> executable and readable and not writable, with `.rsrc` and `.reloc` as its
> only other sections; it MUST have no import table, an entry point of 0, the
> native subsystem, and the single export of `kbdl.export`. Windows 11's
> `LoadKeyboardLayout` rejects a layout DLL whose code and data lie in
> separate sections, even read-only ones, and accepts the merged image
> although rust-lld marks the section as code (characteristics
> `0x60000020`). (Verified: the same crate linked without the `/MERGE` and
> `/SECTION` arguments fails to load.)

> [spec:kbdgen:req:kbdl.image.verify]
> After each build kbdgen MUST read the DLL's PE headers and fail, naming the
> layout, variant and property, unless: the machine is `0x014c` (x86, wow64),
> `0x8664` (x64) or `0xaa64` (arm64); `IMAGE_FILE_DLL` is set;
> `AddressOfEntryPoint` is 0; the subsystem is 1 (native); the sections are
> exactly `.data`, `.rsrc` and `.reloc`, with `.data` having
> `IMAGE_SCN_MEM_EXECUTE` and `IMAGE_SCN_MEM_READ` but not
> `IMAGE_SCN_MEM_WRITE`; the import directory is empty; and the export
> directory has one function and one name, `KbdLayerDescriptor`, ordinal
> base 1. rust-lld emits no `.pdata` for the single leaf function on x64 and
> arm64 (verified); a toolchain that does fails this check.

> [spec:kbdgen:req:kbdl.wow64]
> The `wow64` DLL is installed to `SysWOW64` beside the `x64` DLL in
> `System32`, under the same file name. A 32-bit process's
> `LoadKeyboardLayout` hands that file to the 64-bit kernel, which reads its
> tables with the 64-bit layout, so the `wow64` variant MUST use the slots
> of `kbdl.structs.pointers` and the export of `kbdl.export`. (Verified from
> 32-bit PowerShell: the `wow64` build loads and produces the same output as
> the `x64` build; a plain `x86` build in its place fails to load, as does a
> missing `SysWOW64` copy.)
