# macOS

## Keyboard layout document

> [spec:kbdgen:def:keylayout.document]
> A keylayout document is the Apple `.keylayout` XML file emitted for each
> layout that has a `macOS` section (others produce none), built from the
> embedded template `resources/template-macos-layout.xml`. It has an
> `<?xml version="1.1" encoding="UTF-8"?>` declaration, the doctype
> `<!DOCTYPE keyboard PUBLIC "" "file://localhost/System/Library/DTDs/KeyboardLayout.dtd">`,
> and a root `<keyboard group="126" id="…" name="…">` whose children are, in
> order: `<layouts>` holding
> `<layout first="0" last="17" mapSet="default" modifiers="modifiers" />`,
> `<modifierMap defaultIndex="0" id="modifiers">`, `<keyMapSet id="default">`,
> `<actions>` (omitted when empty, since macOS refuses to load an empty one)
> and `<terminators>` (only when a dead-key state exists).

> [spec:kbdgen:def:keylayout.document.name]
> The keyboard name is the layout's language tag with every `-` and `_`
> removed (`se-SE` → `seSE`); `displayNames` play no part. The id is
> computed from the name's UTF-8 bytes: `crc` = CRC-HQX (CRC-16/XMODEM:
> polynomial 0x1021, init 0, unreflected, no final XOR, i.e. Python
> `binascii.crc_hqx(name, 0)`), `n = min(max(crc / 2, 1), 32768)` with
> integer division, and `id` is the decimal string `-n` (`seSE` → `-30047`).
> Collisions are not checked. The name is reused as the `.keylayout`/`.icns`
> file stem and the `KLInfo_` suffix.

> [spec:kbdgen:thm:keylayout.document.id-range]
> Every emitted `id` lies in [-32768, -1]: `crc` is a `u16`, so `crc / 2`
> is in [0, 32767], the lower clamp gives [1, 32767] (the upper clamp never
> binds), and negation gives [-32767, -1]. The id is never 0 or -32768.

## Key maps

> [spec:kbdgen:req:keylayout.keymaps]
> For the layer at zero-based YAML position `i` the generator MUST append
> `<keyMapSelect mapIndex="i"><modifier keys="K"/></keyMapSelect>` to
> `<modifierMap>` and `<keyMap index="i">` to `<keyMapSet>`, so
> `defaultIndex="0"` selects the first YAML layer whatever its name. `K` by
> layer: `default` `command?`; `shift` `anyShift caps? command?`; `caps`
> `caps`; `caps+shift` `anyShift caps command?`; `alt`
> `anyOption command?`; `alt+shift` `anyOption anyShift caps? command?`;
> `alt+caps` `caps anyOption command?`; `ctrl`
> `anyShift? caps? anyOption? anyControl`; `cmd` `command`; `cmd+shift`
> `command anyShift`; `cmd+alt` `command anyOption`; `cmd+alt+shift`
> `command anyOption anyShift`. Each `<keyMap>` holds the layout-derived
> keys, then the fixed non-ISO keys of `keylayout.keymaps.iso-codes` in
> table order, then code 65 with the layout's `decimal` (default `.`), then
> code 49 with `" "`; the `macOS.space` map is ignored.

> [spec:kbdgen:def:keylayout.keymaps.iso-codes]
> The macOS key-code tables. ISO positions, in order: E00–E12 → 10 18 19 20
> 21 23 22 26 28 25 29 27 24; D01–D12 → 12 13 14 15 17 16 32 34 31 35 33 30;
> C01–C12 → 0 1 2 3 5 4 38 40 37 41 39 42; B00–B10 → 50 6 7 8 9 11 45 46 43
> 47 44. Fixed non-ISO keys, in emission order: 36 `\u{D}`, 48 `\u{9}`,
> 51 `\u{8}`, 53 `\u{1B}`, 64 `\u{10}`, 66 `\u{1D}`, 70 `\u{1C}`,
> 71 `\u{1B}`, 72 `\u{1F}`, 76 `\u{3}`, 77 `\u{1E}`; codes 79 80 96–101 103
> 105–107 109 111 113 → `\u{10}`; 114 `\u{5}`, 115 `\u{1}`, 116 `\u{B}`,
> 117 `\u{7F}`, 118 `\u{10}`, 119 `\u{4}`, 120 `\u{10}`, 121 `\u{C}`,
> 122 `\u{10}`, 123 `\u{1C}`, 124 `\u{1D}`, 125 `\u{1F}`, 126 `\u{1E}`; then
> keypad 67 `*`, 69 `+`, 75 `/`, 78 `-`, 81 `=`, 82–89 `0`–`7`, 91 `8`,
> 92 `9`.

> [spec:kbdgen:req:keylayout.keymaps.tokens]
> Layer tokens are assigned to ISO positions per
> `[spec:kbdgen:req:keys.iso-order.desktop-layers]` and kept raw for all
> matching. Layout-derived `<key>` elements MUST be grouped by raw token —
> groups in order of the token's first occurrence, positions within a group
> in ISO order — not emitted in plain ISO order. A plain key is
> `<key code="C" output="O"/>`, an action key `<key code="C" action="actionNNN"/>`.
> Every `output` value written (keys, fixed keys, `when` outputs,
> terminators) is decoded per `[spec:kbdgen:syn:keys.escape]`; `action`,
> `next` and `state` values are not. `\u{0}` is an ordinary output here, not
> an absent key.

## Dead keys and actions

> [spec:kbdgen:sem:keylayout.actions]
> When a transform targets a raw token in a layer, every position holding
> that token there becomes an action key with its own id. The action's
> states are `<when state="none" output="<token>"/>` then one
> `<when state="dead_keyNNN" output="…"/>` per targeting dead key; a state
> identical in `state` and `output` is never added twice. Ids come from
> per-document counters from 0, formatted `dead_key%03d` and `action%03d`;
> action ids are allocated during processing (transforms, then dead-key
> `next` actions), so replaced actions leave gaps and numbering need not
> follow document order. Each action key appends its `<action id>` with its
> `when` children to the single `<actions>` element, in key emission order.

> [spec:kbdgen:req:keylayout.actions.dead-key-next]
> When `transforms` and `macOS.deadKeys` are both present, for every layer
> with a `deadKeys` entry and every position whose raw token is in that
> list, the generator MUST find the token's registered dead-key state —
> panicking with "dead key `X` in target list but not the transforms."
> otherwise — and replace every transition of that token in the layer with
> a new action holding only `<when state="none" next="dead_keyNNN"/>`,
> discarding earlier outputs or actions. A dead-key token repeated within a
> layer is overwritten position by position: all its positions end up
> emitting the last position's code and id, with a duplicated `<action>`,
> and the earlier codes are unmapped.

## Transforms

> [spec:kbdgen:req:keylayout.transforms]
> Transforms, interpreted per `[spec:kbdgen:req:layout.transforms.dead-key-entries]`,
> are applied only when the layout has `transforms` and `macOS.deadKeys`
> (otherwise a "No transforms"/"No dead keys" warning is logged), and only
> to layers with a `deadKeys` entry — to all of them, whatever their list
> contains. Every branch entry is registered, in YAML order, as a dead-key
> state with its space-escape string as terminator output, even if no layer
> lists it. For each position, each branch whose child key equals the raw
> token contributes `{dead_keyNNN, child value}` per `keylayout.actions`.
> When any state was registered, `<terminators>` MUST be the last child of
> `<keyboard>`, with one `<when state="dead_keyNNN" output="<escape>"/>` per
> state in registration order.

## Bundle and installer

> [spec:kbdgen:req:macbundle.plist+1]
> `macos generate` MUST create `<out>/<bundleId>.bundle/Contents/Resources`
> with `bundleId` = `{packageId}.keyboardlayout.{stem}`, `stem` being the
> `.kbdgen` directory's file stem (not `bundleName`). A missing
> `targets/macos.yaml` or `resources/macos` panics before anything is
> written. `Contents/Info.plist` is an XML plist with exactly
> `CFBundleIdentifier` (bundleId), `CFBundleName` (`bundleName`),
> `CFBundleVersion` (`build`), `CFBundleShortVersionString` (`version`), and
> per keylayout `KLInfo_<name>` → {`TISInputSourceID` `<bundleId>.<name>`,
> `TISIntendedLanguage` language tag}; the `KLInfo_*` entries follow the
> CFBundle keys in bundle layout order. `codeSignId` is required but unused.

> [spec:kbdgen:req:macbundle.plist.bundle-resources]
> Each document MUST be written to `Contents/Resources/<name>.keylayout` as
> pretty-printed UTF-8 (two-space indent, `<x … />`, trailing newline) in
> hex entity mode: in attribute values `& ' " < >`, ASCII controls (tab
> included) and other Unicode Separator/Other characters except U+0020
> (U+00A0 included) become `&#xHHHH;` (uppercase, ≥ 4 digits); combining
> marks stay literal. On macOS hosts, an `icon.<tag>.*` in `resources/macos`
> whose tag equals the layout's tag is converted with ImageMagick `convert`
> to 16, 16@2x, 32 and 32@2x PNGs and `iconutil --convert icns` into
> `<name>.icns`; exit codes are ignored. Other hosts skip icons with a
> warning.

> [spec:kbdgen:req:macbundle.plist.strings]
> For each language tag in any generated layout's `displayNames`,
> `Contents/Resources/<tag>.lproj/InfoPlist.strings` MUST be written (full
> tag, so `se.lproj` and `se-SE.lproj` may both exist) with one
> `"<name>" = "<display name>";` line per layout naming itself in that
> language, in bundle layout order, joined by `\n` with no trailing newline.
> Quoting is Rust `Debug` escaping (`\"`, `\\`, `\n`, `\r`, `\t`, `\0`,
> `\u{…}` for non-printables), not Apple `.strings` escaping.

> [spec:kbdgen:req:macbundle.installer]
> The installer step, sequenced per `[spec:kbdgen:sem:pipeline.steps]`,
> MUST package an existing `<out>/<bundleId>.bundle` without regenerating
> it, in a temporary directory: `pkgbuild --component <bundle> --ownership
> recommended --install-location "/Library/Keyboard Layouts" --version
> <version> <tmp>/inner.pkg`; then `distribution.xml`, an
> `<installer-gui-script minSpecVersion="2">` with `<title>` `bundleName`,
> `<options customize="never" rootVolumeOnly="true"/>`, a choices-outline
> `default` → `<bundleId>`, `<choice id="default"/>`, a hidden
> `<choice id="<bundleId>">` referencing `<pkg-ref id="<bundleId>"
> version="0" auth="root" onConclusion="RequireRestart">inner.pkg</pkg-ref>`;
> then `productbuild --distribution <dist> --version <version>
> --package-path <tmp> <out>/<bundleId>.pkg`. Exit codes are ignored and
> the package is unsigned.
