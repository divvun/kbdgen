# Windows keyboard layout C

The Windows target emits, for each bundle layout with a `windows` section, a
self-contained keyboard-layout translation unit (`.c`), a resource script
(`.rc`) and a module-definition file (`.def`), and compiles them into
keyboard layout DLLs for x86, x64, arm64 and the WOW64 variant installed to
`SysWOW64`. The generated C declares its own mirror of the `kbd.h` structures
and includes no header, so the same source builds with MSVC (`cl`, `rc`,
`link`) on Windows and with `clang-cl`, `llvm-rc` and `lld-link` on any host.
These rules replace the MSKLC path (`klc.*`, `windows.dll*` in
[windows.md](windows.md)); the user-visible layout semantics of `klc.*` are
restated here in table terms, and its known defects are corrected.

In these rules a *cell* is one (key, column) slot of the character table; a
*unit* is one UTF-16 code unit; `WCH_NONE` = `0xF000`, `WCH_DEAD` =
`0xF001`, `WCH_LGTR` = `0xF002`. "Warn" means a `tracing` warning naming the
layout, key position and layer; "fatal" means generation fails with an error
naming the same, and no output is written for that layout.

Sources:

- MSKLC 1.4 `inc/kbd.h` and `kbdutool.exe` v3.40 (from
  <https://download.microsoft.com/download/6/f/5/6f5ce43a-e892-4fd1-b9a6-1a0cbb64e6e2/MSKLC.exe>;
  kbdutool output was inspected for the Võro, Kildin Sami and Northern Sami
  KLCs and for probe KLCs exercising SGCap, dead keys and 3-unit ligatures)
- Microsoft Windows-driver-samples, `input/layout` (`kbdus`, `all_kbds/kbdfr`,
  `all_kbds/kbdgr`: `.c`, `.h`, `.def`, `.rc`, `.vcxproj`):
  <https://github.com/microsoft/Windows-driver-samples/tree/main/input/layout>
- ReactOS `sdk/include/ndk/kbd.h`:
  <https://github.com/reactos/reactos/blob/master/sdk/include/ndk/kbd.h>
- Wine `include/kbd.h` and `include/winuser.rh` (virtual-key values):
  <https://github.com/wine-mirror/wine/tree/master/include>
- Microsoft `windows` crate `KBDTABLES` binding:
  <https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/UI/Input/KeyboardAndMouse/struct.KBDTABLES.html>
- Struct sizes and offsets below were checked by compiling with clang for
  `i686`, `x86_64` and `aarch64-pc-windows-msvc`; the WOW64 pointer
  representation and the link flags were checked with `clang-cl`/`lld-link`
  16, including that clang rejects static initialisation of `__ptr64`
  fields.

## Translation unit

> [spec:kbdgen:def:kbdc.source]
> For each bundle layout with a `windows` section, in bundle order, kbdgen
> writes `<name>.c`, `<name>.rc` and `<name>.def` into the output directory,
> where `<name>` is the keyboard name of `kbdc.metadata`. Each file is ASCII
> only with LF line breaks and is byte-for-byte deterministic for a given
> bundle (no timestamps, paths or tool versions). The `.c` file contains no
> `#include` and no dependency on SDK or WDK headers: every type, constant and
> virtual-key value it uses is defined in the file itself, with a `KBDC_`
> prefix for types and macros. The only configuration inputs are the
> compiler-predefined `_WIN64` and the define `KBDC_WOW64`, which selects the
> WOW64 variant on a 32-bit x86 compile.

> [spec:kbdgen:syn:kbdc.escaping]
> In the `.c` file every UTF-16 unit is written as a hexadecimal integer
> literal `0x%04x` (lowercase, at least four digits). Character and wide-string
> literals (`'a'`, `L"..."`) MUST NOT be used; every string is a
> brace-initialised `KBDC_WCHAR` array of units terminated by `0x0000`
> (`{0x0045, 0x0073, 0x0063, 0x0000}`). Supplementary-plane characters appear as
> their two surrogate units. Comments, if any, contain only ASCII and never
> user-supplied text. Virtual keys are written as `0x%02x`, attributes and flag
> combinations as their numeric value.

## Structures

> [spec:kbdgen:def:kbdc.structs]
> The generated C defines these structures, field for field in this order, with
> `KBDC_BYTE` = 8-bit, `KBDC_WORD`/`KBDC_WCHAR` = 16-bit and `KBDC_DWORD` =
> 32-bit unsigned integers, and `P(T)` a pointer slot (`kbdc.structs.pointers`):
>
> | Struct | Fields |
> |---|---|
> | `VK_TO_BIT` | `BYTE Vk; BYTE ModBits` |
> | `MODIFIERS` | `P(VK_TO_BIT) pVkToBit; WORD wMaxModBits; BYTE ModNumber[8]` |
> | `VK_TO_WCHARS<n>` | `BYTE VirtualKey; BYTE Attributes; WCHAR wch[n]` |
> | `VK_TO_WCHAR_TABLE` | `P(VK_TO_WCHARS1) pVkToWchars; BYTE nModifications; BYTE cbSize` |
> | `DEADKEY` | `DWORD dwBoth; WCHAR wchComposed; WORD uFlags` |
> | `LIGATURE<n>` | `BYTE VirtualKey; WORD ModificationNumber; WCHAR wch[n]` |
> | `VSC_LPWSTR` | `BYTE vsc; P(WCHAR) pwsz` |
> | `VSC_VK` | `BYTE Vsc; WORD Vk` |
> | `KBDTABLES` | `P pCharModifiers; P pVkToWcharTable; P pDeadKey; P pKeyNames; P pKeyNamesExt; P pKeyNamesDead; P pusVSCtoVK; BYTE bMaxVSCtoVK; P pVSCtoVK_E0; P pVSCtoVK_E1; DWORD fLocaleFlags; BYTE nLgMax; BYTE cbLgEntry; P pLigature; DWORD dwType; DWORD dwSubType` |
>
> No `#pragma pack` is used; natural alignment applies.

> [spec:kbdgen:req:kbdc.structs.pointers]
> Every pointer field MUST be a pointer slot. On x86, x64 and arm64 a slot is a native pointer. In the WOW64
> variant (32-bit x86 code, 64-bit table layout, the counterpart of `kbd.h`'s
> `BUILD_WOW6432`/`__ptr64`) a slot MUST be an 8-byte, 8-byte-aligned struct of
> a 32-bit pointer followed by a 32-bit zero, initialised as `{ address, 0 }`,
> so that its bytes equal the zero-extended little-endian 64-bit address.
> `__ptr64` MUST NOT be used (clang cannot statically initialise it). The
> slot type and its initialiser MUST be produced by one macro pair so that the
> table initialisers are textually identical across variants.

> [spec:kbdgen:thm:kbdc.structs.layout]
> Under `kbdc.structs` and `kbdc.structs.pointers`, sizes and offsets in bytes
> are (x86 / x64, arm64 and WOW64): `KBDTABLES` 60 / 104, with `bMaxVSCtoVK` at
> 28 / 56, `pVSCtoVK_E0` 32 / 64, `fLocaleFlags` 40 / 80, `nLgMax` 44 / 84,
> `pLigature` 48 / 88, `dwType` 52 / 96, `dwSubType` 56 / 100; `MODIFIERS` 16 /
> 24 with `wMaxModBits` at 4 / 8 and `ModNumber` at 6 / 10;
> `VK_TO_WCHAR_TABLE` and `VSC_LPWSTR` 8 / 16; independent of pointer size,
> `DEADKEY` 8, `VSC_VK` 4 (`Vk` at 2), `VK_TO_WCHARS<n>` 2+2n and
> `LIGATURE<n>` 4+2n. The generated C MUST assert the `KBDTABLES` size and the
> offsets of `bMaxVSCtoVK`, `fLocaleFlags` and `pLigature` at compile time with
> negative-array-size typedefs, so a layout mismatch fails the build.

## Descriptor and tables

> [spec:kbdgen:def:kbdc.tables]
> The single `KBDTABLES` object `KbdTables` holds, in field order:
> `&CharModifiers` (`kbdc.modifiers`); `aVkToWcharTable` (`kbdc.vk-chars`);
> `aDeadKey` if any dead key is emitted, else null (`kbdc.dead-keys`);
> `aKeyNames`; `aKeyNamesExt` (`kbdc.key-names`); null for `pKeyNamesDead`
> (kbdgen has no dead-key names; kbdutool emits null too); `ausVK`; `127`; `aE0VscToVk`; `aE1VscToVk`
> (`kbdc.scancodes`); the locale flags (`kbdc.locale`); then `nLgMax`,
> `cbLgEntry` and `aLigature` (`kbdc.ligatures`), or `0`, `0` and null when no
> ligature is emitted; then `dwType` `0` and `dwSubType` `0`. Every table has
> static storage duration and internal linkage; only `KbdLayerDescriptor` is
> external.

> [spec:kbdgen:req:kbdc.export]
> The C file MUST define `KbdLayerDescriptor(void)` with the `__stdcall`
> calling convention, returning the address of `KbdTables`; in the WOW64
> variant it MUST return that address zero-extended as an unsigned 64-bit
> integer. The `.def` file MUST be exactly `LIBRARY <name>`, `EXPORTS`,
> `    KbdLayerDescriptor @1`, each line LF-terminated. The DLL MUST export no
> other symbol and MUST have no entry point.

> [spec:kbdgen:thm:kbdc.tables.invariants]
> For every generated layout: each `VK_TO_WCHARS` row containing `WCH_DEAD` is
> immediately followed by a row with `VirtualKey` `0xff`; for every dead
> character `d` emitted there, `aDeadKey` contains exactly one entry with
> `dwBoth` = `0x0020 | d << 16`; no two `aDeadKey` entries share `dwBoth`;
> each `WCH_LGTR` cell has exactly one `aLigature` entry with its virtual key
> and modification number, and every entry has such a cell; `nLgMax` is the
> longest ligature's unit count; and no output unit (cell, ligature unit,
> `wchComposed`, base character) lies in `0xF000`–`0xF002`. These follow from
> `kbdc.vk-chars.tokens`, `kbdc.dead-keys.table` and `kbdc.ligatures`.

## Modifiers and columns

> [spec:kbdgen:req:kbdc.modifiers]
> `aVkToBits` MUST be `{0x10, 1}`, `{0x11, 2}`, `{0x12, 4}`, `{0, 0}` (Shift,
> Control, Menu). `CharModifiers` MUST have `wMaxModBits` `7` and `ModNumber`
> `{0, 1, 2, 0x0F, 0x0F, 0x0F, 3, 4}`: no modifier selects column 0, Shift
> column 1, Ctrl column 2, Ctrl+Alt column 3, Shift+Ctrl+Alt column 4, and
> Shift+Ctrl, Alt and Shift+Alt are invalid (`SHFT_INVALID`).

> [spec:kbdgen:def:kbdc.layers]
> The kbdgen `windows` layers map to character-table columns as: `default` 0,
> `shift` 1, `ctrl` 2, `alt` 3 (AltGr, i.e. Ctrl+Alt) and `alt+shift` 4.
> `caps` and `caps+shift` occupy columns 0 and 1 of an SGCAPS row
> (`kbdc.caps.sgcaps`), and `alt+caps` only influences the caps attributes
> (`kbdc.caps`). A column's index is also the ligature modification number. A
> layer absent from the layout yields `WCH_NONE` in every cell of its column.

## Character tables

> [spec:kbdgen:sem:kbdc.vk-chars.tokens]
> Layer tokens are assigned to positions per
> `[spec:kbdgen:req:keys.iso-order.desktop-layers]`. The token `\u{0}`
> (exactly) is "no key". Any other token is escape-decoded per
> `[spec:kbdgen:syn:keys.escape]`; literal and escaped supplementary-plane
> characters are equivalent. A decoded value that is empty, starts with
> U+0000, or exceeds 4 units warns and is "no key". A one-unit value is a
> character, or a dead key when flagged (`kbdc.dead-keys`). A 2–4-unit value
> is a ligature; a dead-key flag on it warns and is ignored. A unit in
> `0xF000`–`0xF002` is fatal. "No key" is `WCH_NONE`. Each cell therefore holds
> exactly one of: a character, `WCH_DEAD`, `WCH_LGTR` or `WCH_NONE`.

> [spec:kbdgen:req:kbdc.vk-chars]
> `aVkToWch5` (type `VK_TO_WCHARS5`) MUST contain, in this order: one row per
> `kbdc.scancodes.iso` position in table order, each followed by its `0xff`
> dead row (`kbdc.dead-keys`) or SGCAPS row (`kbdc.caps.sgcaps`) when present;
> the space row `{0x20, 0, 0x0020, 0x0020, 0x0020, WCH_NONE, WCH_NONE}`; the
> decimal row `{0x6e, 0, d, d, WCH_NONE, WCH_NONE, WCH_NONE}`; and an all-zero
> terminator. A row's `Attributes` come from `kbdc.caps`. Every position is
> emitted even if all its cells are `WCH_NONE`. `aVkToWcharTable` MUST list
> `aVkToWch3` (3), `aVkToWch5` (5), `aVkToWch2` (2), `aVkToWch1` (1), each
> with `cbSize` = the size of its row type, then `{null, 0, 0}`; the numpad
> table comes last so `VkKeyScan` prefers main-block digits.

> [spec:kbdgen:def:kbdc.vk-chars.fixed]
> The fixed tables, each terminated by an all-zero row and with all
> `Attributes` `0`, are: `aVkToWch3` = `0x08` (Back) `{0x0008, 0x0008,
> 0x007f}`, `0x1b` (Escape) `{0x001b, 0x001b, 0x001b}`, `0x0d` (Return)
> `{0x000d, 0x000d, 0x000a}`, `0x03` (Cancel) `{0x0003, 0x0003, 0x0003}`;
> `aVkToWch2` = `0x09` (Tab) `{0x0009, 0x0009}`, `0x6b` `{0x002b, 0x002b}`,
> `0x6f` `{0x002f, 0x002f}`, `0x6a` `{0x002a, 0x002a}`, `0x6d` `{0x002d,
> 0x002d}`; `aVkToWch1` = `0x60`–`0x69` (Numpad 0–9) producing `0x0030`–`0x0039`.

> [spec:kbdgen:req:kbdc.vk-chars.decimal]
> The decimal character `d` MUST be the first Unicode scalar value of the
> layout's `decimal`, or `.` when absent. An empty `decimal`, or a first scalar
> outside the Basic Multilingual Plane or in `0xF000`–`0xF002`, is fatal. Any
> scalar after the first warns and is ignored.

## Caps lock

> [spec:kbdgen:sem:kbdc.caps]
> A row's `Attributes` are computed from the resolved values of `default` (D),
> `shift` (S), `caps` (C), `alt` (A), `alt+shift` (AS) and `alt+caps` (AC);
> two values are equal when both are "no key" or they have equal strings and
> equal dead-key flags. If C is present and differs from both D and S, the
> attribute is `SGCAPS` (`0x02`). Otherwise, if C is absent, it is `CAPLOK`
> (`0x01`) when D ≠ S plus `CAPLOKALTGR` (`0x04`) when A ≠ AS; if C is present,
> `CAPLOK` when C = S plus `CAPLOKALTGR` when AC = AS. Space, decimal, dead
> (`0xff`), SGCAPS-follow and fixed rows have `Attributes` `0`.

> [spec:kbdgen:req:kbdc.caps.sgcaps]
> A row with `SGCAPS` MUST be followed by a row with the same virtual key,
> `Attributes` `0`, `wch[0]` = `caps`, `wch[1]` = `caps+shift`, and `WCH_NONE`
> in columns 2–4. A dead key in either cell MUST be emitted as its plain
> character, and a ligature as `WCH_NONE`, each with a warning (Windows has no
> representation for either; a ligature would collide with the main row's
> modification numbers 0 and 1). If the main row has a dead key, SGCAPS cannot
> be expressed (both need the following row): the key MUST warn and get the
> attributes computed as if `caps` were absent, with no SGCAPS row.

## Dead keys

> [spec:kbdgen:req:kbdc.dead-keys]
> A one-unit value MUST be flagged dead when the raw (undecoded)
> `windows.deadKeys[<layer>]` list contains its decoded string. A cell holding
> a dead key MUST be `WCH_DEAD`, and the row MUST be followed by a row with
> `VirtualKey` `0xff`, `Attributes` `0`, the dead character in each
> `WCH_DEAD` column and `WCH_NONE` in every other column. The dead characters
> of a layout are the distinct dead units emitted, ordered by first emission
> (rows in table order, columns 0–4).

> [spec:kbdgen:req:kbdc.dead-keys.table]
> `aDeadKey` MUST contain exactly one entry per distinct (dead character,
> base unit) pair, however many keys or layers emit that dead character.
> Entries are grouped by dead character in `kbdc.dead-keys` order; within a
> group they follow the YAML order of its `transforms` children (per
> `[spec:kbdgen:req:layout.transforms.dead-key-entries]`), with the
> space-escape entry last. Each entry is `{dwBoth, composed, 0}` with
> `dwBoth` = `base | dead << 16` (`kbd.h` `DEADTRANS(base, dead, composed, 0)`);
> `DKF_DEAD` is never set. The table ends with an all-zero entry.

> [spec:kbdgen:req:kbdc.dead-keys.diagnostics]
> For each dead character: a missing `transforms` entry, a missing
> space-escape child, or a space-escape child that is a branch is fatal; a
> top-level leaf is fatal; a nested branch under a child is fatal (only one
> level of composition is supported). A child whose input or output is not
> exactly one unit, or is in `0xF000`–`0xF002`, MUST warn (naming the dead
> character, input and output) and be omitted; this includes outputs needing
> a combining mark (`b` → `b́`). None of these conditions may panic.

## Ligatures

> [spec:kbdgen:req:kbdc.ligatures]
> Each ligature cell (`kbdc.vk-chars.tokens`) MUST be `WCH_LGTR` and add one
> `aLigature` entry `{virtual key, column, units…}`, in emission order (rows in
> table order, columns 0–4). With `n` the longest ligature's unit count
> (2–4), `aLigature` MUST have type `LIGATURE<n>`, shorter entries MUST be
> padded with `WCH_NONE`, and the table ends with an all-zero entry;
> `nLgMax` = `n` and `cbLgEntry` = `4 + 2n`. Ligatures never occur in SGCAPS
> follow rows (`kbdc.caps.sgcaps`), so (virtual key, column) is unique.

## Scan codes

> [spec:kbdgen:def:kbdc.scancodes]
> `ausVK` is the 127-entry (`0x00`–`0x7e`) `KBD_TYPE` 4 table of `kbd.h`
> (`T00`…`T7E`) with its flags (`KBDEXT` `0x100`, `KBDMULTIVK` `0x200`,
> `KBDSPECIAL` `0x400`, `KBDNUMPAD` `0x800`), i.e. rows of 16 from `0x00`:
>
> `0ff 01b 031 032 033 034 035 036 037 038 039 030 0bd 0bb 008 009`
> `051 057 045 052 054 059 055 049 04f 050 0db 0dd 00d 0a2 041 053`
> `044 046 047 048 04a 04b 04c 0ba 0de 0c0 0a0 0dc 05a 058 043 056`
> `042 04e 04d 0bc 0be 0bf 1a1 26a 0a4 020 014 070 071 072 073 074`
> `075 076 077 078 079 390 291 c24 c26 c21 06d c25 c0c c27 06b c23`
> `c28 c22 c2d c2e 02c 0ff 0e2 07a 07b 00c 0ee 0f1 0ea 0f9 0f5 0f3`
> `0ff 0ff 0fb 02f 07c 07d 07e 07f 080 081 082 083 084 085 086 0ed`
> `0ff 0e9 0ff 0c1 0ff 0ff 087 0ff 0ff 0ff 0ff 0eb 009 0ff 0c2`

> [spec:kbdgen:def:kbdc.scancodes.extended]
> `aE0VscToVk` is, in order, (scan code → value): `10`→`1b1`, `19`→`1b0`,
> `1d`→`1a3`, `20`→`1ad`, `21`→`1b7`, `22`→`1b3`, `24`→`1b2`, `2e`→`1ae`,
> `30`→`1af`, `32`→`1ac`, `35`→`16f`, `37`→`12c`, `38`→`1a5`, `47`→`124`,
> `48`→`126`, `49`→`121`, `4b`→`125`, `4d`→`127`, `4f`→`123`, `50`→`128`,
> `51`→`122`, `52`→`12d`, `53`→`12e`, `5b`→`15b`, `5c`→`15c`, `5d`→`15d`,
> `5f`→`15f`, `65`→`1aa`, `66`→`1ab`, `67`→`1a8`, `68`→`1a9`, `69`→`1a7`,
> `6a`→`1a6`, `6b`→`1b6`, `6c`→`1b4`, `6d`→`1b5`, `1c`→`10d`, `46`→`103`,
> then `{0, 0}`. `aE1VscToVk` is `{0x1d, 0x13}` (Pause), then `{0, 0}`. All
> values are hexadecimal.

> [spec:kbdgen:req:kbdc.scancodes.iso]
> The character table MUST map the 48 ISO positions `E00`…`B10` of
> `[spec:kbdgen:def:keys.iso-order]` to these virtual keys, which equal
> `ausVK` at the listed scan code, so `ausVK` needs no override: `E00` (`29`)
> `0xc0`; `E01`–`E10` (`02`–`0b`) `'1'`–`'9'`, `'0'`; `E11` (`0c`) `0xbd`;
> `E12` (`0d`) `0xbb`; `D01`–`D10` (`10`–`19`) `QWERTYUIOP`; `D11` (`1a`)
> `0xdb`; `D12` (`1b`) `0xdd`; `C01`–`C09` (`1e`–`26`) `ASDFGHJKL`; `C10`
> (`27`) `0xba`; `C11` (`28`) `0xde`; `C12` (`2b`) `0xdc`; `B00` (`56`)
> `0xe2`; `B01`–`B07` (`2c`–`32`) `ZXCVBNM`; `B08` (`33`) `0xbc`; `B09`
> (`34`) `0xbe`; `B10` (`35`) `0xbf`. Letters and digits are their ASCII codes.

## Key names

> [spec:kbdgen:req:kbdc.key-names]
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
> `{0, null}`.

## Locale and metadata

> [spec:kbdgen:req:kbdc.locale]
> `fLocaleFlags` MUST be `KBD_VERSION` (`1`) in the high word and, in the low
> word, `KLLF_ALTGR` (`0x0001`) when any cell of columns 3 or 4 is not
> `WCH_NONE`, else `0`. `KLLF_SHIFTLOCK` and `KLLF_LRM_RLM` are never set.

> [spec:kbdgen:req:kbdc.metadata]
> The keyboard name MUST be `kbd` followed by `windows.config.id` if set,
> otherwise by the first five Unicode scalar values of the language tag
> (`kbdse-FI`, `kbdsjd-C`); a name containing anything but ASCII letters,
> digits, `-` and `_` is fatal. The description and the language name MUST
> both be the layout's `displayNames` entry for its primary language subtag;
> the copyright and company MUST be the project's `copyright` and
> `organisation`. Two layouts with the same keyboard name are fatal.

> [spec:kbdgen:req:kbdc.metadata.locale]
> The LCID MUST be looked up from the layout's own language tag, never from
> `windows.config.locale`: the primary subtag is mapped to ISO 639-3 and the
> LCID record whose (639-3 code, script, region) exactly matches the tag's
> subtags is used, an absent subtag matching only an absent field; with no
> match the LCID is `0x2000`. The locale name MUST be `windows.config.locale`
> if set; otherwise the full language tag if a record matched; otherwise
> `<primary>-<script or Latn>-<region or 001>`.

## Resources

> [spec:kbdgen:req:kbdc.resources]
> The `.rc` file MUST define a `VERSIONINFO` with `FILEVERSION` and
> `PRODUCTVERSION` from the `windows` target `version` (up to three
> dot-separated components) and `build` (fourth), `1,0,0,0` with a warning when
> absent or not 16-bit decimals; `FILEFLAGSMASK 0x3f`, `FILEFLAGS 0`,
> `FILEOS 0x40004`, `FILETYPE 2` (DLL), `FILESUBTYPE 7` (keyboard driver); a
> `StringFileInfo` block `000004B0` with `CompanyName`, `FileDescription`
> (`<description> Keyboard Layout`), `FileVersion`, `InternalName` (`<name>`),
> `LegalCopyright`, `OriginalFilename` (`<name>.dll`), `ProductName`
> (`<description>`) and `ProductVersion`; and `Translation 0x0000, 0x04B0`.
> String tables MUST hold `1000` = description and `1100` = language name
> under `LANGUAGE` (LCID & `0x3ff`), (LCID >> 10), and `1200` = locale name
> under `LANGUAGE 9, 1`. String `1000` is what `Layout Display Name`
> (`@<name>.dll,-1000`) resolves.

> [spec:kbdgen:syn:kbdc.resources.strings]
> Every `.rc` string value MUST be a wide literal `L"..."`. Printable ASCII
> other than `"` and `\` is written as itself; every other unit, including
> `"`, `\` and each surrogate of a supplementary character, is written as
> `\x` followed by exactly four uppercase hexadecimal digits (`V\x00F5ro`).
> No other escape form is used.

## Build

> [spec:kbdgen:req:kbdc.build]
> kbdgen MUST build `<name>.dll` for `x86`, `x64`, `arm64` and `wow64` (x86
> code compiled with `/DKBDC_WOW64`) into `<out>/<arch>/`, with intermediates
> under `<out>/<arch>/build/<name>/`, and only for layouts generated in this
> run. It MUST accept either MSVC (`cl`, `rc`, `link` in each architecture's
> environment) or LLVM (`clang-cl --target=<i686|x86_64|aarch64>-pc-windows-msvc`,
> `llvm-rc`, `lld-link`), so the build also runs on non-Windows hosts. Compile
> with `/nologo /c /O1 /Gy /GS- /Zl /W4 /WX`; link with `/nologo /dll /noentry
> /nodefaultlib /machine:<X86|X64|ARM64> /subsystem:native /opt:ref /opt:icf
> /release /def:<name>.def`, the `.res`, and the section options of
> `kbdc.image`. A tool that fails to start or exits unsuccessfully is fatal,
> naming the stage and exit status.

> [spec:kbdgen:req:kbdc.image]
> Every built DLL MUST place the `KBDTABLES` object, every table and string it
> reaches, and the code of `KbdLayerDescriptor` in one section named `.data`
> whose characteristics are initialised data, execute and read, and not write;
> it MUST contain no other section holding code or data (only `.rsrc`,
> `.reloc` and, if the toolchain emits unwind data, `.pdata`), no writable
> section, and no import table. Linking MUST therefore pass
> `/merge:.rdata=.data /merge:.text=.data /merge:.bss=.data
> /merge:.edata=.data /section:.data,re`, as Microsoft's `kbdus.vcxproj` does.
> A DLL with a separate executable `.text` and writable `.data` is rejected by
> `LoadKeyboardLayout` on Windows 11 even though its tables are valid; the
> build MUST verify these properties of its output and fail otherwise.
