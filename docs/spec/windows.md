# Windows

The Windows target turns each layout's `windows` section into a Microsoft
Keyboard Layout Creator (MSKLC) `.klc` source file and, on Windows hosts only,
compiles each `.klc` into per-architecture keyboard layout DLLs.

## KLC file

> [spec:kbdgen:def:klc.file]
> A KLC file is the MSKLC layout source emitted for each bundle layout that has
> a `windows` section, in bundle layout order. It is written as
> `<keyboard-name>.klc` directly into the output directory, overwriting any
> existing file; a write failure panics. Its parts, in order: the header lines
> `KBD`, `COPYRIGHT`, `COMPANY`, `LOCALENAME`, `LOCALEID` and `VERSION\t1.0`,
> each followed by a blank line; `SHIFTSTATE` and `LAYOUT`; `LIGATURE` (only
> if non-empty); the `DEADKEY` blocks; a constant `KEYNAME`/`KEYNAME_EXT`
> block (`FOOTER_CONTENT` in src/build/windows/klc/file.rs, starting with a
> blank line, `01\tEsc` … `87\tF24`, then `1c\t"Num Enter"` …
> `5d\tApplication`); then `\nDESCRIPTIONS\n\n<lcid>\t<description>\n\n`,
> `LANGUAGENAMES\n\n<lcid>\t<autonym>\n\n` and `ENDKBD\n`. Here `<lcid>` is
> lowercase hexadecimal, zero-padded to at least 4 digits, and unquoted.

> [spec:kbdgen:req:klc.file.encoding]
> The KLC file MUST be written as UTF-16 little-endian with a leading byte
> order mark `FF FE`. Line breaks MUST be a single LF (`U+000A`) everywhere,
> including the constant `KEYNAME` block; no CR is written.

## Metadata

> [spec:kbdgen:req:klc.metadata]
> The keyboard name MUST be `kbd` followed by `windows.config.id` if set,
> otherwise by the first five Unicode scalar values of the language tag
> (`kbdse-FI`, `kbdsjd-C`). It is neither truncated nor validated, and it is
> both the `KBD` name and the output file stem. The description and autonym
> MUST both be the layout's `displayNames` entry for its primary language
> subtag; copyright and company MUST be the project's `copyright` and
> `organisation`. The header lines MUST be `KBD\t<name>\t"<description>"`,
> `COPYRIGHT\t"<copyright>"` and `COMPANY\t"<company>"`, with values inserted
> verbatim (embedded quotes are not escaped).

> [spec:kbdgen:req:klc.metadata.locale]
> The LCID MUST be looked up from the layout's own language tag, never from
> `windows.config.locale`. The primary language subtag is mapped to ISO 639-3,
> and the LCID record whose (639-3 code, script, region) exactly matches the
> tag's subtags is used; an absent subtag matches only an absent field. If
> there is no match, the LCID MUST be `0x2000`. The locale name MUST be
> `windows.config.locale` if set; otherwise the full language tag if a record
> matched; otherwise `<primary>-<script or Latn>-<region or 001>`. The header
> MUST contain `LOCALENAME\t"<locale name>"` and `LOCALEID\t"<lcid>"`, with
> the LCID as eight lowercase zero-padded hexadecimal digits (`"00000c3b"`).

## Layout

> [spec:kbdgen:req:klc.layout]
> The `SHIFTSTATE` block MUST be fixed: `SHIFTSTATE`, a blank line,
> `0 // 4`, `1 // 5 Shift`, `2 // 6 Ctrl`, `6 // 7 Alt`,
> `7 // 8 Alt + Shift`, a blank line. Columns 0–4 are the `default`, `shift`,
> `ctrl`, `alt` and `alt+shift` layers. `LAYOUT`, a blank line, then one row
> per `klc.layout.key-codes` entry in table order follow, each row taking its
> layers' tokens per `[spec:kbdgen:req:keys.iso-order.desktop-layers]`, and
> each optionally followed by its SGCap row (`klc.caps`). Then come the space
> row `39\tSPACE\t0\t0020\t0020\t0020\t-1\t-1`, the decimal row
> `53\tDECIMAL\t0\t<d>\t<d>\t-1\t-1\t-1` (`<d>` is the first character of
> `decimal`, default `.`, rendered as a key), and a blank line. A row is
> `<sc>\t<vk>\t<cap>\t<c0>\t<c1>\t<c2>\t<c3>\t<c4>\n`. A layer absent from the
> YAML yields `-1` in its column.

> [spec:kbdgen:def:klc.layout.key-codes]
> The KLC key-code table is the fixed, ordered list of 48 ISO positions with
> their MSKLC scan code and virtual-key name:
> E00 `29` `OEM_3`, E01–E10 `02`–`0b` `1`–`9`,`0`, E11 `0c` `OEM_MINUS`,
> E12 `0d` `OEM_PLUS`; D01–D10 `10`–`19` `Q W E R T Y U I O P`,
> D11 `1a` `OEM_4`, D12 `1b` `OEM_6`; C01–C09 `1e`–`26`
> `A S D F G H J K L`, C10 `27` `OEM_1`, C11 `28` `OEM_7`, C12 `2b` `OEM_5`;
> B00 `56` `OEM_102`, B01–B07 `2c`–`32` `Z X C V B N M`,
> B08 `33` `OEM_COMMA`, B09 `34` `OEM_PERIOD`, B10 `35` `OEM_2`.

> [spec:kbdgen:sem:klc.layout.tokens]
> Each layer token resolves to "no key" or to a key value (a string plus a
> dead-key flag). The token `\u{0}` (exactly) is "no key". A raw token
> containing a literal character above U+FFFF panics with
> `Unrepresentable key detected!`. Otherwise the token is escape-decoded per
> `[spec:kbdgen:syn:keys.escape]`. A decoded value that is empty, starts with
> U+0000, or exceeds 4 UTF-16 units is logged and becomes "no key". A value of
> exactly one UTF-16 unit is emitted as a character, or as a dead key if
> flagged (`klc.deadkeys`). A longer value, including an escaped
> supplementary-plane character, is a ligature (`klc.ligatures`), and its
> dead-key flag is ignored.

> [spec:kbdgen:syn:klc.layout.key-rendering]
> A rendered key is one of the following:
> a character in U+0021–U+007E written as itself;
> any other single-unit character written as its code point in lowercase
> hexadecimal, zero-padded to 4 digits (space is `0020`, `ä` is `00e4`);
> a dead key written as the character rendering followed by `@`
> (`00b4@`, `^@`);
> a ligature written as `%%`;
> "no key" written as `-1`;
> or, in SGCap rows only, the empty string for skipped columns.

## Caps lock

> [spec:kbdgen:sem:klc.caps]
> A key row's caps mode is computed from its `default`, `shift`, `caps`,
> `alt`, `alt+shift` and `alt+caps` values. Values are equal when both are
> "no key" or have equal strings and dead-key flags. If `caps` is present and
> differs from both `default` and `shift`, the mode is `SGCap`. If `caps` is
> "no key", the mode is the sum of 1 if `default` ≠ `shift` (so digits get
> `1`) and 4 if `alt` ≠ `alt+shift`. Otherwise, the mode is the sum of 1 if
> `caps` = `shift` and 4 if `alt+caps` = `alt+shift`. `alt+caps` is never
> emitted. An `SGCap` row is followed immediately by
> `-1\t-1\t0\t<caps>\t<caps+shift>\t\t\t\n`; these two values use ligature
> columns 0 and 1 and the key's virtual-key name.

## Ligatures

> [spec:kbdgen:req:klc.ligatures]
> Every key value of 2–4 UTF-16 units MUST be emitted as `%%` and MUST add a
> `LIGATURE` row: `<vk>\t<column>`, then `\t<unit>` for each UTF-16 unit as
> 4-digit lowercase hexadecimal, with surrogates written as-is. `<column>` is
> the 0-based `SHIFTSTATE` column: 0 default, 1 shift, 2 ctrl, 3 alt,
> 4 alt+shift. Rows follow emission order (key rows in table order, columns
> 0–4, then the SGCap row) without deduplication. The section MUST be
> `LIGATURE`, a blank line, the rows and a blank line, and it MUST be omitted
> when there are no ligatures.

## Dead keys

> [spec:kbdgen:req:klc.deadkeys]
> A key value MUST be flagged as dead when the raw (undecoded)
> `windows.deadKeys[<layer>]` list contains its decoded string. Every
> single-unit dead key emitted, including in SGCap rows, MUST produce one
> `DEADKEY` block per occurrence, in emission order. If the layout has no
> `transforms`, an error is logged and no blocks are written. A block is `DEADKEY <c as 4-digit lowercase hex>`, a blank line,
> rows, and a blank line. The rows come from the `transforms` entry keyed by
> `c`, interpreted per `[spec:kbdgen:req:layout.transforms.dead-key-entries]`.
> Each child is a row `<from>\t<to>` (one UTF-16 unit each, 4-digit lowercase
> hexadecimal) in YAML order, with the space escape last as `0020\t<to>`. A
> missing entry yields an empty block. Multi-unit rows are skipped with an
> error; empty strings and literal non-BMP characters panic.

## DLL build

> [spec:kbdgen:req:windows.dll]
> The Windows build runs KLC generation, then the DLL step only when kbdgen
> is compiled for Windows (otherwise two warnings are logged).
> The DLL step MUST:
> 1. Fail if any of the `x64`, `x86` or `arm64` MSVC environments is invalid.
> 2. Install the pahkat `msklc` package (`nightly`, from
>    `https://pahkat.uit.no/devtools/`) into
>    `<app-data>/kbdgen/prefix/windows`; a download error exits with status 1.
> 3. For every `*.klc` in the output directory (sorted, including stale files)
>    and each architecture in that order, in `<out>/<arch>/build/<stem>/` run
>    `kbdutool.exe -n -s -u`, then `cl.exe`, `rc.exe` and `link.exe` under
>    that architecture's environment, and move `<stem>.dll` to
>    `<out>/<arch>/<stem>.dll`.
>
> The first command that fails to start or exits unsuccessfully MUST abort
> the build with an error naming the stage and the exit status.

> [spec:kbdgen:req:windows.dll.link]
> The link step MUST produce a native-subsystem, entry-less keyboard DLL whose
> descriptor, code and tables all live in one read-only executable section,
> to match Microsoft's keyboard-layout samples. To do so it MUST pass
> `-nologo -SECTION:INIT,D -OPT:REF -OPT:ICF -IGNORE:4039,4078 -noentry -dll
> -subsystem:native,5.0 -merge:.edata=.data -merge:.rdata=.data
> -merge:.text=.data -merge:.bss=.data -section:.data,re -STACK:0x40000,0x1000
> -osversion:4.0 -version:4.0 /release`. The compile step MUST pass `/MD /c
> /Zp8 /Gy /W3 /WX /Gz /Gm- /EHs-c- /GR- /GF -Z7 /Oxs` together with the
> fixed `-D` define set in src/build/windows/build_klc.rs `cl_command`.
