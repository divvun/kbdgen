# Model to Android layout resources

The Android target writes static resources into a clone of
`giellakbd-android` (`docs/spec/android.md`): per layout, the rows and row
keys XML for phone and tablet, and `assets/layouts/{tag}.json` with the
dead-key table and speller. For a v4 layout, these are derived from the
`android` host document of its compiled layout (`ldml.model.layout`) by the
adapter below. The app does not run the engine yet (`ldml.crate.ffi` is
deferred), so whatever the static resources cannot express is lost on
Android until it does.

Both v3 and v4 layouts feed one *Android layout input*, and one writer turns
that input into files. So v3 output stays as `android.*` specifies, and the
two paths can be compared file by file.

Sources: `docs/spec/android.md` (`android.layout-xml*`, `android.keys*`,
`android.transforms`, `android.metadata*`); `docs/spec/ldml/kbdl.md` as the
template for an adapter; the AOSP LatinIME `KeySpecParser` and
`MoreKeySpec` syntax, of which giella-ime is a fork.

## Input

> [spec:kbdgen:def:ldml.android.input]
> An *Android layout input* has the layout's tag, its `displayNames`, an
> optional speller `{path, package_url}`, a dead-key table (dead key →
> ordered `{next: output}`), a `preserve_case` flag, and a *phone* and an
> optional *tablet* sheet. A sheet has a `default` and an optional `shift`
> layer, each a list of rows. A row is a list of cells. A cell is
> `Shift`, `Backspace`, `Role` (another role key, drawn as nothing),
> `Spacer{width}`, or `Key{spec, width, dead, hint, more}`, with width in
> thousandths. The writer implements `android.layout-xml*`, `android.keys*`
> and `android.transforms` on it. The v3 path builds it from the bundle
> as those rules read tokens; other `\s{…}` tokens stay raw `Key`s.

> [spec:kbdgen:def:ldml.android.adapter]
> The adapter's input is a compiled layout with an `android` keyboard K,
> the layout's `targets.android`, and the bundle's project. Its output is
> one Android layout input. A layout without an `android` document, or
> whose `android` keyboard has no touch set, has no Android output; a
> hardware set presented as touch is not used. The adapter MUST NOT panic.
> It warns, or fails fatally, naming the layout, the touch set, the layer,
> and the row and column or the marker.

## Sheets and layers

> [spec:kbdgen:sem:ldml.android.sizes]
> The phone sheet (`res/xml`) comes from the touch set that
> `touch_set_for_width(0)` selects (`ldml.engine.touch`). If the selected
> set has a `min_device_width`, the adapter fails fatally: phones would
> have no layout. The tablet sheet (`res/xml-sw600dp`) comes from
> `touch_set_for_width(95)`, 95 mm being Android's 600 dp. When that is
> the phone set, there is no tablet sheet and nothing is written to
> `xml-sw600dp`, so Android's resource fallback uses the phone files.
> Every other touch set warns and is dropped, since giella-ime has no
> further size qualifier. Divergence: v3 required both sheets.

> [spec:kbdgen:sem:ldml.android.layers]
> In each sheet, touch layer `base` becomes the `default` layer and touch
> layer `shift`, if any, the `shift` layer. The writer puts `shift` in the
> `<case>` element for `alphabetManualShifted`, `alphabetShiftLocked` and
> `alphabetShiftLockShifted` (`android.layout-xml.rows`). Every other
> layer, including `caps`, `symbols-1` and `symbols-2`, warns and is
> dropped: the app supplies its own symbol keyboards. Row *r* of a layer is
> row *r* of the input. When `base` and `shift` have different row counts,
> the adapter warns and keeps both as they are, as v3 did.

## Keys

> [spec:kbdgen:sem:ldml.android.cells]
> Each position of a touch row becomes one cell:
>
> - role `shift`: `Shift`; role `backspace`: `Backspace`
> - any other role: `Role`, with a warning (`ldml.android.dropped`)
> - a gap: `Spacer` with the key's width
> - output a single marker m of a dead key: `Key` with `dead`, and the
>   *dead identity* of `ldml.kbdl.values` as its text
> - plain text output: `Key` with that text
> - an empty output, text mixed with markers, or several markers: warn, and
>   `Spacer` with the key's width
>
> When `normalization` is `Enabled`, texts are written NFC, the engine's
> output form. A key's `layer_id` other than through a role warns and is
> ignored.

> [spec:kbdgen:syn:ldml.android.key-spec]
> A `Key`'s spec is its text escaped for `KeySpecParser`: each `\` and `|`
> is preceded by `\`, and so is a leading `!`. A key with a display
> (`ldml.model.displays`, by key id first, then by output) different from
> its text gets the spec `escape(display)|escape(text)`; a dead key never
> does, since its text is already its display. A `more` entry is escaped
> the same way, and also has each `,` preceded by `\`. Hint labels are
> written unescaped. For v3 tokens this equals `android.keys`, where a lone
> `\` becomes `\\`, except where a token contains `|`, `,` or a leading
> `!`, which v3 wrote raw.

> [spec:kbdgen:sem:ldml.android.widths]
> Let L be the largest sum, over the `default` layer's rows, of the row's
> cell widths in key widths (thousandths / 1000), and w = 100/L on phone
> and 90/L on tablet. A cell of width 1000 gets no `keyWidth`, as in
> `android.keys`. Another `Key` or `Spacer` width *k* gets
> `latin:keyWidth="{w·k/1000}%p"`, formatted as the shortest round-trip
> `f64`. On rows after the first, `Shift` gets `{f:.2}%` with
> f = (100 − w·T)/S, where T is the sum of the widths of the row's `Key`
> and `Spacer` cells and S the number of its `Shift`, `Backspace` and
> `Role` cells. With default widths this is `android.keys` exactly.
> `stretch` is ignored.

> [spec:kbdgen:sem:ldml.android.long-press]
> A key's `long_press` candidates, in order, give `more`; the first
> candidate's text is its `hint`. A candidate that is a gap, a role, or
> has a marker in its output warns and is skipped. The writer applies the
> row rules of `android.keys` and `android.keys.number-row` unchanged: on
> the first row, `hint` is replaced by the digit and only `moreKeys` is
> written. `long_press_default`, `multi_tap` and `flick` warn and are
> dropped (`ldml.android.dropped`). Because v4 lowering gives candidates to
> every key whose output matches (`ldml.yaml.long-press`), this equals v3's
> lookup by exact token wherever the token needs no decoding.

> [spec:kbdgen:req:ldml.android.preserve-case]
> `preserve_case` MUST be set exactly when the layout's tag is `lut`, as
> in `android.keys`. The writer then adds
> `latin:keyLabelFlags="preserveCase"` to every `Key`.

## Dead keys

> [spec:kbdgen:sem:ldml.android.dead-keys]
> The dead-key table has one entry per marker m of a `dead` cell, in order
> of first occurrence, keyed by m's dead identity. Entries come from the
> engine (`ldml.engine.api`; `CancelOrPass`, `Nfc`, host `android`): on an
> empty context, press the dead key by `Id`, `Emit` a probe, and take the
> committed text C. Probes are, in order: `" "`; each literal continuation c of a
> `simple` rule whose pattern is exactly `\m{m}` followed by unquantified
> `Char`s; then each distinct `Key` text of the sheets. Entry c → C is
> written for `" "` and literal continuations always, and for other
> probes when C differs from `flush[m]` followed by the probe.

> [spec:kbdgen:sem:ldml.android.dead-keys.limits]
> The table has one level, as giella-ime reads it. A probe that leaves a
> marker pending, such as a chained dead key or a second dead key, warns
> and gets no entry. A dead cell whose marker gives no entries at all
> still gets `latin:deadKey="True"` and an empty table. Two markers with
> the same dead identity are fatal, as in `ldml.kbdl.values`. Dead keys no
> android cell reaches are absent; v3 wrote every top-level transform.

> [spec:kbdgen:req:ldml.android.speller]
> `targets.android.spellerPath` and `spellerPackageKey` MUST give the
> input's speller: absent both, the speller is `null`; a `spellerPackageKey`
> without `spellerPath`, or one that is not a URL, is fatal. Divergence:
> the dead-key table is written whether or not a speller is configured.
> v3 wrote `{}` without `android.config`, which left `deadKey` keys typing
> nothing (`android.transforms`).

## Reporting

> [spec:kbdgen:req:ldml.android.dropped]
> The adapter MUST warn, once per layout and feature, with a count and up
> to ten positions, for each touch feature the static resources cannot
> express: touch sets and layers dropped by `ldml.android.sizes` and
> `ldml.android.layers`; roles other than `shift` and `backspace`;
> `bottomRow: authored`, since the app always appends its own bottom row
> (`row_qwerty4`); layer switches on keys; flicks; multi-tap;
> `long_press_default`; and skipped cells and candidates. The hardware set
> and the `labels` are ignored without a warning, because giella-ime
> draws neither.

> [spec:kbdgen:req:ldml.android.classify]
> The adapter MUST report, as one information message per layout, what
> the static resources do not reproduce and only the engine on Android
> could provide (`ldml.crate.ffi`):
>
> - transforms that do not start with a marker
> - `reorder` groups
> - `backspace` rules other than the generated one
> - chained dead keys and dead keys following dead keys
> - an enabled `normalization`
>
> It reuses `ldml.kbdl.classify`'s categories and format: counts and up to
> ten examples.

## Target integration

> [spec:kbdgen:req:ldml.android.target]
> `target android` MUST build each v4 layout through the adapter and each
> v3 layout as before, in bundle layout order, into one writer run.
> Everything outside the input follows `docs/spec/android.md` unchanged:
> the clone, dependencies, `method.xml`, `spellchecker.xml`, strings,
> icons and Gradle. For a v4 layout the tag and `displayNames` come from
> the compiled layout. Where `android.*` panics (no `en` display name, a
> bad speller URL), the v4 path fails with an error naming the layout. This
> replaces the `android` clause of `ldml.yaml.coexistence`.

## Tests

> [spec:kbdgen:req:ldml.android.test.writer]
> The writer MUST be callable without git, network or Gradle, on a
> directory holding a minimal giella-ime skeleton checked in with the
> tests: `res/xml/method.xml` and `spellchecker.xml` with one `<subtype>`
> each, `res/values/strings.xml`, `strings-appname.xml`, and one
> `res/values-{t}` directory. Every test below runs it on a fresh copy.

> [spec:kbdgen:req:ldml.android.test.golden]
> For the golden v4 layouts that have an `android` touch variant, and for a
> v4 fixture exercising widths, gaps, displays, escapes, dropped features
> and a tablet-less layout, the written `res/xml*`, `res/values*` and
> `assets/layouts` files MUST equal checked-in golden files byte for byte.
> The adapter's warnings and classification are part of the golden.

> [spec:kbdgen:req:ldml.android.test.differential]
> For each v3 fixture with an `android` section, migrated in memory, the
> writer's files from the v3 input and from the adapter's input MUST be
> byte-identical, except for differences each explained by one of:
> a reported defect (M03, M10, M14, M16, M17); an automatic display
> (`ldml.yaml.displays.auto`); `ldml.android.key-spec` escaping; a
> dropped role (`ldml.android.cells`); the `" "` entry or entry order of
> the dead-key table; or `ldml.android.speller`'s divergence. Unexplained
> differences fail the test, as in `ldml.migrate.equivalence`. The
> fixtures are `vro` and a synthetic v3 layout with dead keys on both
> sheets, number-row long press, `lut` and a speller.

## Open questions

These need an operator decision before or during implementation; none is
a rule yet.

1. **Unmatched dead keys in giella-ime.** The table only lists matches.
   What does the app do with a dead key followed by an unlisted character,
   or by another dead key? The macOS rule (standalone, then the key) needs
   the app to do that; otherwise every probe must be written explicitly.
2. **KeySpec syntax.** Confirm that giella-ime's `KeySpecParser` and
   `MoreKeySpec` match AOSP's (`\` escapes, `label|output`, `!` prefixes,
   `,` splitting), including in `keyHintLabel`.
3. **Other roles.** `return`, `tab`, `symbols`, `keyboard`: drop them
   (proposed), or map them to the app's key styles if its
   `key_styles_common` defines them?
4. **`caps` layer.** Drop it (proposed), or write it to a separate `<case>`
   for `alphabetShiftLocked`?
5. **`preserveCase`.** Keep the hard-coded `lut` rule (proposed, for
   equivalence), or derive it from whether `shift` is the uppercase of
   `base`, or make it a `targets.android` option?
6. **Tablet fallback.** With no tablet set, write nothing to
   `xml-sw600dp` (proposed), or write the phone set there with the
   tablet's 90/L width?
7. **Engine model asset.** Should the port already write
   `assets/layouts/{tag}.android.dvkb` (`ldml.model.encoding`) so the
   later FFI work only touches the app?
