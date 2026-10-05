# Layout format v4

Format 4 of `layouts/<tag>.yaml` is shaped after LDML and compiles to the
keyboard model (`ldml.model.*`).

What linguists write stays what they wrote before: rows of characters and
one composition table per dead key. The compiler generates the rest:

- key ids
- marker keys and displays
- the dead-key transform groups
- the caps layers

The vocabulary is LDML's. Layer names are modifier sets, layouts declare
forms and touch sizes, and escapes and normalization follow LDML. Anything
LDML can say can be written verbatim (`variables`, `transforms`,
`backspace`, explicit `keys`), so importing any LDML keyboard is total.

Packaging stays in `targets/*.yaml` and `project.yaml` (`bundle.structure`).
Platform configuration for a single layout moves into a `targets:` block in
the layout file.

Sources: the gap analysis (§4 schema proposal, §5 migration, §6 semantics);
UTS #35 Part 7; `docs/spec/{layout,bundle,kbdl}.md`.

## Detection and schema

> [spec:kbdgen:def:ldml.yaml.detect]
> A layout file is v4 when its top-level mapping has `format: 4`, an
> integer. Without `format` it is v3 (`layout.schema`). Any other value of
> `format` is an error naming the file. A v4 file is loaded under the same
> rules as v3:
>
> - stem tag normalisation and layout order (`bundle.layouts`)
> - the autonym requirement (`bundle.layouts.autonym`)
>
> A bundle may mix v3 and v4 files.

> [spec:kbdgen:def:ldml.yaml.schema]
> A v4 layout has these top-level fields. Every string is a YAML string.
>
> | Field | Type | Default | Lowers to |
> |---|---|---|---|
> | `format` | `4` | required | — |
> | `displayNames` | tag → name, autonym required | required | layout display names |
> | `info` | `name` `author` `layout` `indicator` `attribution` | `name` = autonym | LDML `info` |
> | `version`, `locales` | semver; list of tags | none | LDML `version`, `locales` |
> | `normalization` | `disabled` or `enabled` | `disabled` | `ldml.yaml.normalization` |
> | `decimal` | output string | none | model `decimal` |
> | `keyNames` | `space`, `return` (labels) | none | model `labels` |
> | `ldml` | path, or host → path | none | `ldml.yaml.ldml-ref` |
> | `deadKeys` | `ldml.yaml.dead-keys` | `{}` | markers, keys, groups |
> | `variables`, `transforms`, `backspace`, `keys`, `flicks`, `displays` | `ldml.yaml.verbatim` | empty | LDML as written |
> | `longPress` | output → candidate tokens | `{}` | `ldml.yaml.long-press` |
> | `hardware` | variant name → `ldml.yaml.hardware` | `{}` | hardware set |
> | `touch` | variant name → `ldml.yaml.touch` | `{}` | touch sets |
> | `emoji` | `key`, `annotations` | none | `ldml.yaml.emoji` |
> | `targets` | `ldml.yaml.targets` | `{}` | per-host configuration |

> [spec:kbdgen:req:ldml.yaml.strict]
> Unlike v3, a v4 file with an unknown field, at any depth, MUST fail to
> load, with an error naming the file and the field path. Every error names
> the file, the YAML path (`hardware.macOS.layers.shift`) and, for row
> errors, the row and token index. Warnings use the same addressing.
> Loading v4 never panics.

> [spec:kbdgen:sem:ldml.yaml.normalization]
> `normalization: disabled`, the default, lowers to LDML
> `<settings normalization="disabled"/>`. The engine then emits exactly the
> authored scalar values (`ldml.engine.normalization`), which is the macOS
> rule. `enabled` omits `settings` and gives LDML's NFD matching and NFC
> output. A layout should opt in only if its transforms must match
> canonically equivalent text that came from outside the keyboard.

> [spec:kbdgen:req:ldml.yaml.ldml-ref]
> `ldml: <path>` names an LDML keyboard3 file that defines the keyboard for
> every host. The path is relative to the layout file. The map form `ldml:
> {<host or default>: <path>}` names one file per host, with `default`
> covering the hosts not listed. A layout with `ldml` MUST NOT have:
>
> - `deadKeys`, `variables`, `transforms`, `backspace`, `keys`, `flicks`,
>   `displays`, `longPress`
> - `hardware`, `touch`
> - `normalization`, `info`, `version`, `locales`
>
> Its `displayNames`, `decimal`, `keyNames`, `emoji` and `targets` override
> the file's kbdgen data. Export re-serializes the file as written
> (`ldml.xml.ldml-ref`).

## Text and tokens

> [spec:kbdgen:syn:ldml.yaml.escape]
> Every v4 string that LDML would escape-decode is decoded with LDML's
> grammar (`ldml.xml.escape`):
>
> - outputs and row tokens
> - `deadKeys` identities, inputs and values
> - displays and labels
> - `decimal`
> - verbatim LDML strings
>
> A `\u` not followed by `{` is an error; the migrator rewrites these
> (`ldml.migrate.defects`). Escapes are decoded once, at load. Every
> comparison afterwards uses the decoded scalar values: dead-key identity,
> `longPress` keys, compose inputs, key-id synthesis. `displayNames` and
> `targets` are plain strings.

> [spec:kbdgen:syn:ldml.yaml.rows]
> A *rows* value is a YAML string. Each non-blank line is one row, and
> tokens within a row are separated by ASCII spaces and tabs. Leading and
> trailing blanks and blank lines are ignored. A token never contains
> whitespace; whitespace output is written `\u{20}` or `\s{space}`.

> [spec:kbdgen:syn:ldml.yaml.tokens]
> A token is one of the following, tried in this order. *w* is a decimal
> width greater than 0 with at most three fraction digits.
>
> | Token | Meaning | Where |
> |---|---|---|
> | `\u{0}` | no key at this position | hardware; in touch flick rows, no flick |
> | `\s{gap}`, `\s{gap:w}`, `\s{spacer:w}` | the gap key, width *w* | both |
> | `\s{space}`, `\s{space:w}` | the `space` key (output U+0020), width *w* | both |
> | `\s{R}`, `\s{R:w}` | role key, R from `ldml.yaml.touch.roles` | touch |
> | `\s{"x":w}` | output *x* (literal-token syntax) with width *w* | touch |
> | `\d{X}` | dead key whose `deadKeys` identity is X; X may contain `\u{…}` | both |
> | `\k{id}` | an explicit `keys` entry or implied key, by id | both |
> | `\l{L}`, `\l{L:w}` | key switching to touch layer L | touch |
> | anything else | a literal output in LDML `key@output` syntax: text, `\u{…}`, `\m{…}`, `${…}` | both |
>
> A `\` not starting one of these escapes is a literal backslash.

> [spec:kbdgen:def:ldml.yaml.key-ids]
> Keys come from tokens, and each distinct key definition gets an id:
>
> - A single scalar in `0-9A-Za-z` with no other attribute keeps that
>   character, which is the implied key.
> - `\s{space}` is `space`, and a gap is `gap`.
> - A dead key is `dk-<marker>`.
> - A role key is `<role>`, or `<role>-<target layer>` when it switches
>   layers.
> - `\l{L}` is `layer-<L>`.
> - Other outputs are `u-` followed by the hex scalar values, uppercase, at
>   least four digits each, joined by `-` (`u-00F5`, `u-0061-0301`). If the
>   output contains a marker, the id is `o-<n>`, with *n* counted from 1 in
>   order of first use.

> [spec:kbdgen:sem:ldml.yaml.key-ids.collisions]
> If two definitions differ in any attribute (width, long press, flick,
> layer, role) but would get the same id, the later one in document order
> gets the suffix `-2`, `-3` and so on. Document order is hardware layers,
> then touch sizes by width, then layers, rows and tokens. Ids from `keys`
> are reserved first.

## Hardware

> [spec:kbdgen:def:ldml.yaml.hardware]
> `hardware` maps variant names (`default`, `windows`, `macOS`, `chromeOS`,
> `linux`, `android`) to variants. A variant has:
>
> - `form`: `iso` (default), `us`, `jis`, `ks` or `abnt2`, or a custom
>   `{id, rows: [scan-code strings]}`
> - `inherits`: another variant
> - `impliedLayers`: `macOS` (default) or `none`
> - `layers`: modifier set → rows
> - `space`: modifier set → token, the space position of that layer
> - `extraModifiers` (`ldml.yaml.native`)
>
> With `inherits`, the variant starts from the named variant, fully
> resolved, then replaces `form`, `impliedLayers` and `extraModifiers` when
> it gives them. It replaces `layers` and `space` entries key by key.
> Inheritance cycles are errors.

> [spec:kbdgen:syn:ldml.yaml.modifier-names]
> A `layers` key is one or more LDML modifier sets separated by `,`. Each
> set is space-separated components from `ldml.model.modifiers` (`none`,
> `shift`, `caps`, `alt`, `altL`, `altR`, `ctrl`, `ctrlL`, `ctrlR`,
> `other`, and the Extensions `cmd`, `extra1`, `extra2`, `extra3`).
> Component order does not matter: `shift caps` and `caps shift` are the
> same set. Two keys of one variant naming equal or overlapping sets are an
> error (`ldml.model.invariants`). The exception is native-only sets
> (`ldml.model.native`), which may overlap only each other.

> [spec:kbdgen:req:ldml.yaml.hardware.rows]
> A hardware layer has exactly as many rows as its form has character
> rows: 4 for every implied form. Row *r* has exactly as many tokens as form
> row *r* has scan codes. For `iso` that is 13, 12, 12 and 11; for
> `abnt2` 13, 12, 12 and 12, the last being `B11`. An optional final row
> holds the space position, one token. Without it, the layer gets `space`,
> or the `space` entry for its set. A count mismatch is an error naming the
> layer, the row and both counts, so a 49-token row is caught. `\u{0}`
> marks a position with no key. Trailing positions with no key are left
> out of the exported LDML row.

> [spec:kbdgen:sem:ldml.yaml.implied-layers]
> With `impliedLayers: macOS`, the compiler adds caps states for each
> authored set S that has neither `caps` nor `shift` and is not native-only.
> Let Sh be S with `shift`.
>
> 1. If Sh is authored and S+`caps`+`shift` is not, the Sh layer also gets
>    that set. Caps+Shift is then uppercase, without inversion.
> 2. If S+`caps` is not authored and Sh is not authored either, the S layer
>    also gets S+`caps`.
> 3. If S+`caps` is not authored and Sh is, a layer S+`caps` is created.
>    Each position takes the Sh key when both keys' outputs are plain text
>    and Sh's output is the `str::to_uppercase` of S's and differs from it.
>    Otherwise it takes the S key. A created layer identical to an existing
>    one becomes an extra set of that layer instead.
>
> `none` adds nothing, which is LDML's exact matching.

> [spec:kbdgen:def:ldml.yaml.native]
> Extension. A layer whose key contains `cmd`, or `ctrl`/`ctrlL`/`ctrlR`
> without an alt component, is native-only (`ldml.model.native`). Typical
> keys are `ctrl`, `cmd`, `cmd shift`, `cmd alt` and `cmd alt shift`. Its
> tokens are ordinary, dead keys included. `extraModifiers` is an ordered
> list of up to three distinct keys from `rightCtrl`, `capsLock` and `B00`,
> where the *i*-th binds `extra`*i* (`ldml.model.windows`). Layers using
> `extra`*n* need the *n*-th binding. With a `B00` binding, `B00` positions
> MUST be `\u{0}`. The 49th key needs `form: abnt2`.

## Touch

> [spec:kbdgen:def:ldml.yaml.touch]
> `touch` maps variant names (`default`, `iOS`, `android`) to `{inherits?,
> longPress?, sizes}`. `sizes` maps a size name to:
>
> - `minDeviceWidth`, defaulting to none for `phone`, 95 for `tablet` and
>   190 for `tablet-large`, and required for other names
> - `bottomRow`: `host` (default) or `authored`
> - `layers`: layer id → rows, or → `{rows, flicks}`
>
> Every size MUST have a `base` layer, and the sizes' widths MUST be
> distinct. `inherits` copies another variant's sizes and replaces them
> size by size. With `bottomRow: host` the host draws the symbols, globe,
> space and return row, so the authored rows exclude it.

> [spec:kbdgen:sem:ldml.yaml.touch.roles]
> Role tokens lower to keys with a `role` (`ldml.model.keys`):
>
> - `shift` switches to `shift` from `base`, and to `base` from any other
>   layer.
> - `symbols` switches to `symbols-1` from `base` or `shift`, and to `base`
>   otherwise.
> - `shiftSymbols` switches between `symbols-1` and `symbols-2`.
> - `backspace`, `return`, `tab`, `caps` and `keyboard` are gaps that the
>   host draws and handles.
>
> A role that switches to a layer the size lacks is an error. An unknown
> role is an error. LDML has no key roles, so an LDML-only consumer sees
> layer-switch keys and gaps (`ldml.xml.ldml-view`).

> [spec:kbdgen:sem:ldml.yaml.touch.flicks]
> In `{rows, flicks}`, `flicks` maps a direction list (`s`, `ne s`) to
> rows positionally aligned with `rows`. The token at (*r*, *c*) becomes
> that direction's target for the key at (*r*, *c*). A flick row must have
> the same token count as its main row. `\u{0}` and gap tokens mean no
> flick, and role tokens there must equal the main row's. A key with
> flicks gets flick id `flick-<key id>`. Migrated iPad `alt` layers are
> `flicks: {s: …}`.

> [spec:kbdgen:def:ldml.yaml.long-press]
> `longPress` maps an output to a string of candidate tokens. The
> `ldml.yaml.tokens` grammar applies, and the first candidate is not
> special. Every key in the document whose output equals the decoded
> output gets those candidates as `longPressKeyIds`, hardware keys
> included, since LDML ignores long press on hardware. A touch variant's
> `longPress` replaces global entries with the same output. Default
> candidates and multi-tap need explicit `keys`.

## Dead keys

> [spec:kbdgen:def:ldml.yaml.dead-keys]
> `deadKeys` maps an *identity*, the decoded string authors associate with
> the dead key, to a node. A node has:
>
> - `marker`: an NMTOKEN, by default derived as in
>   `ldml.yaml.dead-keys.keys`
> - `display`: default the identity
> - `standalone`: default the identity, and may be `""`
> - `name`: a Windows dead-key name (top level only)
> - `compose`: an ordered map from input (a key output) to either an output
>   string or a nested node, which is a chained dead key
>
> An input `" "` is an error; that is what `standalone` is for. Identities
> and markers MUST be unique.

> [spec:kbdgen:sem:ldml.yaml.dead-keys.keys]
> The default marker of a top-level node is `dk_` followed by the identity's
> scalars as uppercase hex of at least four digits, joined by `_`
> (`dk_00B4`, `dk_00A0_0330`). A nested node's default is its parent's
> marker, `-`, and the input's scalars encoded the same way. The `\d{X}`
> token lowers to key `dk-<marker>` with output `\m{marker}`. For each
> marker m the compiler also emits:
>
> - the display `\m{m}` → `display`
> - model `flush[m]` = `standalone`, and `dead_key_names[m]` = `name`
>
> Only dead keys reachable from a `\d{}` token of the host document get
> markers, rules and keys, together with their nested nodes.

> [spec:kbdgen:sem:ldml.yaml.dead-keys.compose]
> The compose group is the first `simple` group. For each reachable node, in
> `deadKeys` order and depth first, and each `compose` entry (input c,
> value v) in order, it holds:
>
> - `from="\m{m}<c>"` with c regex-escaped, and `to` = v escaped for `to`,
>   or `\m{child}` for a nested node
> - when c is the identity of a reachable top-level dead key E, also
>   `from="\m{m}\m{e}"` with the same `to`, which reproduces the dead-key
>   chaining of Windows and macOS
>
> Each node's entries end with `from="\m{m}\u{20}"` `to=standalone`.

> [spec:kbdgen:sem:ldml.yaml.dead-keys.fallback]
> The fallback group, second, gives the macOS rule: an unmatched dead key
> emits its standalone output, then the key. For each reachable marker m,
> standalone S:
>
> - for every reachable top-level marker e, `from="\m{m}\m{e}"`
>   `to="S\m{e}"`, so the second dead key stays pending
> - then `from="\m{m}(.{1,9}.{0,9}…)"` `to="S$1"`, with enough `.{0,9}`
>   atoms to cover the longest key output in the document. That length is
>   counted in scalars, after NFD when normalization is enabled.
>
> With `standalone: ""` the dead key is dropped instead. User `transforms`
> groups follow this group.

> [spec:kbdgen:sem:ldml.yaml.dead-keys.backspace]
> When any dead key is reachable, the compiler appends a final `backspace`
> group with the single rule `from="\m{.}"` and no `to`. It runs after the
> user's `backspace` groups. So any LDML engine reading the export cancels
> only the pending dead key, as kbdgen's own default does
> (`ldml.engine.backspace.default`).

## Verbatim LDML

> [spec:kbdgen:def:ldml.yaml.verbatim]
> These fields are LDML as written, with LDML syntax inside the strings:
>
> - `variables`: `{strings: {id: value}, sets: {id: value}, usets: {id: value}}`
> - `transforms` and `backspace`: lists of groups. A group is a list of
>   `{from, to?}`, or `{reorder: [{from, before?, order?, tertiary?,
>   tertiaryBase?, preBase?}]}`.
> - `keys`: `id → {output?, gap?, layer?, width?, stretch?, longPress?,
>   longPressDefault?, multiTap?, flick?, role?}`, where key lists are
>   space-separated ids
> - `flicks`: `id → [{directions, key}]`
> - `displays`: `[{output | keyId, display}]` and `displayBase`
>
> Ids and marker names are kept as written. LDML's own validation applies
> after lowering (`ldml.xml.validate`).

> [spec:kbdgen:sem:ldml.yaml.displays.auto]
> Unless `displays` already covers the output or key, the compiler adds:
>
> - for each dead-key marker, its display (`ldml.yaml.dead-keys.keys`)
> - for each key whose output consists only of characters of general
>   category M, `display` = U+25CC followed by the output
>   (§Non-spacing marks on keytops)
> - `<display keyId="space">` = `keyNames.space`

## Targets and emoji

> [spec:kbdgen:def:ldml.yaml.targets]
> `targets` holds a layout's own platform configuration. In v3 this was
> `<platform>.config`.
>
> | Key | Fields | Use |
> |---|---|---|
> | `windows` | `locale`, `id`, `shiftLock` (bool), `lrmRlm` (bool), `keyNames` (an entry name of `kbdl.key-names` → name) | `kbdl.metadata.bundle`, model `windows` |
> | `chromeOS` | `locale`, `xkbLayout` | ChromeOS manifest |
> | `iOS`, `android` | `spellerPackageKey`, `spellerPath` | speller packaging |
>
> An unknown `keyNames` entry name is an error. These fields reach XML as
> `kbdgen:windows`, `kbdgen:windowsKeyName` and `kbdgen:target`.

> [spec:kbdgen:def:ldml.yaml.emoji]
> `emoji: {key?, annotations?}`. `key` is `{position, modifiers}`, with an
> ISO position name (`keys.iso-order`) and a modifier set. It becomes the
> preserved key of `ldml.model.emoji`. `annotations` is a path, relative to
> the layout file, to a CLDR `annotations` XML file (LDML Part 2) for the
> layout's language, read through `xmlem`. Each `annotation` with
> `type="tts"` gives the name, and the others give `|`-separated keywords.

## Hosts and lowering

> [spec:kbdgen:sem:ldml.yaml.hosts]
> A layout compiles to one *host document* per host. Each host takes the
> first variant present in its chain:
>
> | Host | Hardware chain | Touch chain |
> |---|---|---|
> | `windows` | `windows`, `default` | — |
> | `macOS` | `macOS`, `default` | — |
> | `chromeOS` | `chromeOS`, `default` | — |
> | `linux` | `linux`, `default` | — |
> | `iOS` | — | `iOS`, `default` |
> | `android` | `android`, `default` | `android`, `default` |
> | `web` | `default` | `default` |
>
> A host with neither a hardware nor a touch variant has no document. Every
> document shares the layout-level fields. Equal documents share a model
> (`ldml.model.layout`).

> [spec:kbdgen:sem:ldml.yaml.lowering]
> For each host document, the compiler:
>
> 1. resolves `inherits`
> 2. decodes tokens and assigns key ids
> 3. applies implied layers
> 4. generates the dead-key keys, displays and groups
> 5. builds an `xmlem` source document in `ldml.xml.export` form, with
>    kbdgen data per `ldml.xml.special`, marking generated groups and
>    recording `deadKeys` (`ldml.yaml.import`)
> 6. resolves that document to a model (`ldml.xml.resolve`)
>
> `kbdgen ldml export` writes the same document, so the export is exactly
> what the model was built from.

> [spec:kbdgen:sem:ldml.yaml.import]
> Import (XML → v4) writes the following:
>
> - **Layers:** the document's hardware layers become a variant named by
>   `kbdgen:keyboard@host`, or `default` without it, and touch sets become
>   sizes.
> - **Tokens:** a key is written as a literal or `\s{…}` token when
>   `ldml.yaml.key-ids` would recreate it with the same id and attributes.
>   Otherwise it goes in `keys` and is written as `\k{id}`.
> - **Generated groups:** groups marked with `kbdgen:generated`, and caps
>   sets that `impliedLayers` would recreate, are replaced by `deadKeys`
>   and `impliedLayers: macOS` when re-deriving them reproduces them
>   exactly. Otherwise they stay verbatim, with a warning.
> - **Normalization:** `normalization` follows `settings`, so an absent
>   `settings` gives `enabled`.
>
> Imports are spliced in. Comments are dropped, and their count is
> reported.

> [spec:kbdgen:thm:ldml.yaml.roundtrip]
> For every valid keyboard3 document X:
>
> > resolve(export(import(X))) = resolve(X)
>
> For every v4 layout Y, the models of import(export(Y)) equal those of Y.
>
> Proof sketch: import writes every model field into a v4 construct. That
> construct is either a verbatim LDML field, an explicit key, or sugar
> whose lowering is checked against the original during import. Lowering
> then rebuilds that field. Comments and import structure are not
> preserved through YAML, but neither reaches the model. XML-to-XML keeps
> both (`ldml.xml.roundtrip`).

> [spec:kbdgen:req:ldml.yaml.coexistence]
> Until each target generator is ported to the model:
>
> - `windows` builds v4 layouts through `ldml.kbdl.adapter`, and v3
>   layouts through `kbdl.input.bundle`.
> - `macos`, `ios`, `android` and `chromeos` generators MUST fail on a v4
>   layout with an error naming the layout and the target. They MUST NOT
>   skip it silently.
> - `kbdgen ldml export` and `compile` accept only v4 layouts.
> - `kbdgen ldml migrate` turns v3 into v4.

## Example

An illustration based on the Võro layout (`keyboard-vro`), abbreviated. Rows marked `…` are elided. The migrator itself writes every variant in full, without `inherits`.

```yaml
format: 4
displayNames: {vro: Võro, en: Võro}
keyNames: {space: vaih, return: sisse}
decimal: ','
deadKeys:
  ´: {standalone: ´, compose: {a: á, A: Á, b: b́, B: B́}}   # marker dk_00B4
  ˇ: {standalone: ˇ, compose: {c: č, C: Č}}
  ę: {standalone: ę, compose: {a: á}}
longPress: {a: á ä æ å, o: ó ö õ ø õ̭}
hardware:
  macOS:
    form: iso
    layers:
      none: |
        \d{ˇ} 1 2 3 4 5 6 7 8 9 0 + \d{´}
        q w e r t y u i o p ü õ
        a s d f g h j k l ö ä '
        < z x c v b n m , . -
      shift: |
        \d{~} ! " # € % & / ( ) = ? \u{301}
        Q W E R T Y U I O P Ü Õ
        A S D F G H J K L Ö Ä *
        > Z X C V B N M ; : _
      caps: |
        § 1 2 3 4 5 6 7 8 9 0 + \d{ę}
        Q W E R T Y U I O P Ü Õ
        A S D F G H J K L Ö Ä '
        < Z X C V B N M , . -
      cmd: |
        § 1 2 3 4 5 6 7 8 9 0 - =
        q w e r t y u i o p å ¨
        a s d f g h j k l ø æ '
        ` z x c v b n m , . /
    space: {caps: \u{A0}, alt: \u{A0}}
  windows:
    inherits: macOS
    layers:
      altR: |
        \u{0} \u{0} @ £ $ € \u{0} { [ ] } \ \d{´}
        \u{0} š é ŕ t́ ý u̬ i̬ ó ṕ ü̬ õ̭
        …
touch:
  iOS:
    sizes:
      phone:
        layers:
          base: |
            q w e r t y u i o p ü õ
            a s d f g h j k l ö ä '
            \s{shift:1.25} \s{spacer:0.25} z x c v b n m đ \s{spacer:0.25} \s{backspace:1.25}
      tablet:
        layers:
          base:
            rows: |
              q w e r t y u i o p ü õ \s{backspace}
              \s{spacer:0.25} a s d f g h j k l ö ä \s{return:1.25}
              \s{shift:1.1} z x c v b n m , . \s{shift:2.4}
            flicks:
              s: |
                1 2 3 4 5 6 7 8 9 0 ` ´ \s{backspace}
                \s{spacer:0.25} % # € & * ( ) ' " + @ \s{return:1.25}
                \s{shift:1.1} q _ - = / ; : ! ? \s{shift:2.4}
targets:
  windows: {locale: vro-Latn}
```

The macOS `caps` layer above is authored. Its `caps shift` state is implied
from `shift`. With `impliedLayers: macOS`, the `alt caps` state is derived
from `alt` and `alt shift`, which are not shown here.
