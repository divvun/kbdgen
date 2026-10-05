# Keyboard tests

Three kinds of evidence pin the engine and the formats:

- **CLDR's own keyboardTest3 vectors.** These check the LDML subset
  against an external definition.
- **kbdgen's golden vectors.** These are a YAML superset that adds
  hardware modifiers, preedit and pass checks, and cover the macOS rules and
  every extension.
- **Round trips.** XML, YAML and the binary model.

Sources: CLDR `keyboards/test/README.md` (tech preview),
`ldmlKeyboardTest3.dtd`, `keyboards/test/*.xml`; `docs/spec/tsf.md`
`tsf.test.engine`, `tsf.test.differential`.

## CLDR vectors

> [spec:kbdgen:req:ldml.test.cldr]
> `kbd-ldml` MUST vendor these files from CLDR release 48, under
> `crates/kbd-ldml/testdata/cldr/48/`, and run them in `cargo test`:
>
> - `keyboards/3.0/*.xml`
> - `keyboards/test/*.xml`
> - `keyboards/import/*.xml`
>
> Each keyboard MUST resolve without errors. For each `test`, the harness
> runs `ldml.test.harness` with `Options { backspace: CodePoint,
> output_form: Nfc }`:
>
> - `startContext` sets the document.
> - `keystroke` sends `Id` with the gesture: `flick` → `Flick`,
>   `longPress` → `LongPress`, `tapCount` → `MultiTap`.
> - `emit` sends `Emit`.
> - `backspace` sends `Backspace`.
> - `check` compares NFD(document) with NFD(`result`); expected results are
>   written in mixed forms, such as `pcm` `e\u{323}` and `pt` `Çç`.
>
> A failing vector may be listed as an expected failure only with a reason
> that cites the ambiguity it depends on.

> [spec:kbdgen:def:ldml.test.harness]
> The harness keeps a *document*, the text before the caret, and a `State`.
> For each event it:
>
> 1. passes the last `context_len` scalar values as `Context.text`, with
>    `at_start` true when that is the whole document
> 2. applies an `Edit` by deleting `delete` scalar values, appending
>    `insert`, and recording `preedit` and `layer`
> 3. on `Pass`, changes nothing and records that the event passed
>
> A reset step sets `State::default()`. The harness has no Windows,
> application or OS dependency (`tsf.test.engine`).

## Golden vectors

> [spec:kbdgen:def:ldml.test.vectors]
> A golden vector file is YAML:
>
> - `layout`: a v4 or v3 layout path, or `keyboard`: an XML path
> - `host`, which selects the host document
> - optional `options`: `backspace` and `outputForm`
> - `tests`: a list of `{name, context?, atStart?, steps}`
>
> A step is one of:
>
> - `press: {key, mods?}`, where `key` is an ISO position name
>   (`keys.iso-order`), `space`, or a hex scan code `0xNN`
> - `touch: {size, layer, row, col, gesture?}`
> - `id: <key id>` with optional `gesture`
> - `emit: <text>`
> - `backspace: {mods?}`
> - `decimal`, `commit` or `reset`
> - `expect: {text?, preedit?, pass?, layer?}`
>
> `mods` uses `shift` `shiftL` `shiftR` `caps` `ctrl` `ctrlL` `ctrlR` `alt`
> `altL` `altR` `altgr` `cmd` `extra1` `extra2` `extra3`. Here `shift`,
> `ctrl` and `alt` mean the left key, and `altgr` sets `alt_r` and `altgr`.
> `expect` compares exact scalar values, without normalization.

> [spec:kbdgen:req:ldml.test.golden]
> `crates/kbd-engine/tests/golden/` MUST contain vectors covering each of
> these:
>
> - every row of `ldml.scope.macos-rules`
> - AltGr through `altR` and through `ctrl alt`
> - shortcut pass for Ctrl, Cmd and Windows Left Alt
> - native-only layers never selected
> - extra modifiers bound to each of the three keys
> - LRM/RLM, decimal, and `Other` with no layer
> - every gesture and layer switch
> - each regex feature: classes, captures, `$0`, mapped sets, `^` with and
>   without `at_start`, `\m{.}`, set alternation order, and earliest match
>   start
> - the §Element: reorder Tai Tham example
> - the three examples of §Normalization and Markers, and the `è` plus
>   U+0320 example
> - the backspace ksha example, plus both default policies
> - context mismatch dropping markers
>
> Each migrated fixture bundle needs vectors for its dead keys and caps.

> [spec:kbdgen:req:ldml.test.bundle]
> A bundle MAY contain `tests/*.yaml` in the golden-vector format, with
> paths relative to the bundle. `kbdgen ldml test` runs them
> (`ldml.cli.test`). Bundle loading (`bundle.structure`) ignores `tests/`.

## Round trips and robustness

> [spec:kbdgen:req:ldml.test.roundtrip]
> Tests MUST check:
>
> - `ldml.xml.roundtrip` and `ldml.xml.superset-roundtrip` on every vendored
>   CLDR keyboard and every export of the fixture bundles, with the model
>   equality and the second write byte-identical to the first
> - `ldml.yaml.roundtrip` in both directions on the same sets
> - that encoding and decoding each fixture's models gives equal models
>   (`tsf.test.engine`)
> - that the second encoding is byte-identical to the first

> [spec:kbdgen:req:ldml.test.robust]
> Property tests MUST check that `Model::from_bytes` on arbitrary bytes,
> and `Model::key` on arbitrary contexts and event sequences against the
> fixture models, never panic. They MUST also check that every `Edit` keeps
> to the bounds of `ldml.engine.action`.

> [spec:kbdgen:req:ldml.test.kmc]
> CI SHOULD validate every exported fixture keyboard with Keyman's `kmc`
> LDML compiler, the validator CLDR itself uses. LDML errors fail the run;
> kbdgen's `special` content is out of `kmc`'s scope.

> [spec:kbdgen:req:ldml.test.repertoire]
> keyboardTest3 `repertoire` elements are parsed and skipped with an
> informational note. Running them is deferred (`ldml.scope.deferred`).

> [spec:kbdgen:req:ldml.test.oracle]
> Differential tests against Keyman Core, the other LDML runtime, are
> deferred (`ldml.scope.deferred`). When added, they run on the CLDR
> vectors, and their output may differ only at documented divergences.
