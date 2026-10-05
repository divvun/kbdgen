# Migration from v3 to v4

`kbdgen ldml migrate` converts v3 layout files (`layout.schema`) into v4
(`ldml.yaml.*`).

- Behaviour is preserved per platform wherever v4 can express it.
- Where it cannot, or where the operator's macOS rules change what a
  platform did, the migrator reports a numbered defect. It never fixes such
  a case silently.
- A file with a blocking defect is not rewritten.

Sources: the gap analysis §5 (what converts automatically, the
manual-review table, per-bundle survey) and §6; `docs/spec/{layout,kbdl,macos,ios,android,chromeos}.md`.

## Conversion

> [spec:kbdgen:sem:ldml.migrate.platforms]
> Each v3 platform section becomes a v4 variant, and its layer names map to
> modifier sets:
>
> | v3 | v4 variant | Layer → v4 key |
> |---|---|---|
> | `windows.primary` | `hardware.windows` | `default` → `none`, `shift`, `caps`, `caps+shift` → `caps shift`, `alt` → `altR`, `alt+shift` → `altR shift`, `alt+caps` → `altR caps`, `ctrl` (native-only) |
> | `macOS.primary` | `hardware.macOS` | as Windows but `alt` → `alt`, `alt+shift` → `alt shift`, `alt+caps` → `alt caps`; `cmd`, `cmd+shift` → `cmd shift`, `cmd+alt` → `cmd alt`, `cmd+alt+shift` → `cmd alt shift` (native-only) |
> | `chromeOS.primary` | `hardware.chromeOS` | as Windows |
> | `iOS.primary`, `iPad-9in`, `iPad-12in` | `touch.iOS` sizes `phone`, `tablet`, `tablet-large` | `default` → `base`; `shift`, `caps`, `symbols-1`, `symbols-2` keep their ids; `alt` and `alt+shift` → `flicks: {s: …}` of `base` and `shift` |
> | `android.primary`, `tablet-600` | `touch.android` sizes `phone`, `tablet` | `default` → `base`, `shift` |
>
> Desktop layers re-split their 48 tokens into rows of 13, 12, 12 and 11 on
> `form: iso`. `macOS.space` entries become `space`.

> [spec:kbdgen:sem:ldml.migrate.dead-keys]
> Each top-level `transforms` entry becomes a `deadKeys` node with the same
> identity. Its `' '` child becomes `standalone`, and its other children,
> in order, become `compose`. Deeper branches become nested nodes. A token
> that a platform treated as dead on a layer becomes `\d{X}` there.
>
> | Platform | Dead on a layer when |
> |---|---|
> | Windows, macOS, ChromeOS, iOS | the layer's `deadKeys` list contains the token |
> | Android | the token equals any transform root, on every layer (`android.keys`) |
>
> Tokens are compared after escape decoding (`ldml.yaml.escape`).
> Comparing the raw entries instead was a v3 defect (`kbdl.dead-keys`).

> [spec:kbdgen:sem:ldml.migrate.fields]
> The following carry over:
>
> - `displayNames` and `decimal` unchanged
> - `keyNames`, with escapes decoded
> - `longpress`, renamed `longPress`
> - `windows.config` {`locale`, `id`} → `targets.windows`
> - `chromeOS.config` → `targets.chromeOS`
> - `iOS.config` and `android.config` → `targets.iOS` and `targets.android`
> - `\s{…}` tokens unchanged; `\u{0}` stays `\u{0}`, which means no key
>
> These are dropped and reported (M10): `windows.config.languageName` and
> `legacyName`, `windows.space` and `android.deadKeys`, all of which v3
> never read. A `config: null` is omitted silently. Every layout gets
> `impliedLayers: macOS` (default) and `normalization: disabled` (default),
> so neither is written.

> [spec:kbdgen:req:ldml.migrate.output]
> The migrator MUST write fresh v4 text, not round-trip the YAML through
> serde:
>
> - top-level fields in `ldml.yaml.schema` order
> - variants in `ldml.yaml.hosts` order, and layers in v3 order
> - rows aligned with single spaces
> - YAML strings quoted only where YAML requires
>
> Its output for a given input is byte-for-byte deterministic, and loading
> it gives no warnings beyond the reported defects. It writes each file in
> place under its v3 name, keeping no backup; git is the backup.
> `--dry-run` writes nothing and only reports.

## Defects

> [spec:kbdgen:req:ldml.migrate.defects]
> The migrator MUST detect and report each of these. A *block* leaves the
> layout unwritten.
>
> | Code | Defect | Action |
> |---|---|---|
> | M01 | dead key listed on a layer that lacks that token | drop, warn |
> | M02 | `deadKeys` names a layer the platform does not have | drop, warn |
> | M03 | transform root never dead on any layer | keep as unreferenced `deadKeys`, warn |
> | M04 | dead key listed with no transform entry | block: plain key or empty dead key? |
> | M05 | desktop layer with other than 48 tokens | block |
> | M06 | `\u` without braces (`­`) | rewrite as `\u{00AD}`, warn |
> | M07 | v2-format layout (no v3 platform sections) | block |
> | M08 | dead-key identity of more than one scalar | info: Windows cannot make it dead (`ldml.kbdl.values`) |
> | M09 | caps behaviour changes (`ldml.migrate.caps-diff`) | warn, listing positions |
> | M10 | v3 field not carried over | warn, naming the field |
> | M11 | comments dropped | warn, with line numbers |
> | M12 | iOS and Android dead keys differ for one layout | info |
> | M13 | macOS `\u{0}`, which output NUL and now means no key | warn |
> | M14 | iOS, Android or ChromeOS token with `\u{…}`, which was emitted raw and is now decoded | warn |
> | M15 | iPad `alt` row not aligned with `default` | warn, drop the unaligned flicks |
> | M16 | dead-key root whose standalone differs from the root | info: `standalone` holds the `' '` child |

> [spec:kbdgen:req:ldml.migrate.caps-diff]
> For each desktop variant, the migrator MUST compare old and new output
> for the states `caps`, `caps shift` and alt+`caps`, at every position.
>
> - The old output comes from the v3 platform's rule: Windows per
>   `kbdl.caps` (CAPLOK, SGCAPS, CAPLOKALTGR); macOS per
>   `keylayout.keymaps`, where unmatched states use the first layer;
>   ChromeOS by its runtime fallback (exact layer, then `caps`, then
>   `shift`, then `default`).
> - The new output comes from the v4 layers after `ldml.yaml.implied-layers`.
>
> Each position that differs is listed under M09, with both outputs. A
> typical case is Windows Caps+Shift on letters, which was lowercase and is
> now uppercase.

> [spec:kbdgen:def:ldml.migrate.report]
> The report goes to stdout as text, or with `--report <file>` as YAML. It
> has one entry per defect: code, layout file, YAML path, row and token
> where known, message, and action taken. A summary counts defects per code
> and lists blocked layouts. The exit status is 0 when nothing is blocked
> and 1 otherwise. Unblocked layouts are still written.

> [spec:kbdgen:req:ldml.migrate.equivalence]
> For every layout it writes, the migrator MUST check the result by running
> the engine. On each v4 host document, at each hardware position, it
> presses every layer whose modifier state exists in v3 and is not
> native-only. For each dead key and its compose inputs, it presses the
> dead key and then the input key. Each result MUST equal the v3 platform's
> documented output, unless a reported defect (M04–M16) explains the
> difference. An unexplained difference is a bug in the migrator, reported
> as M99, and it blocks the layout.

> [spec:kbdgen:req:ldml.migrate.survey]
> Before v4 is declared frozen, `kbdgen ldml migrate --dry-run` MUST be run
> over every bundle that `divvun-keyboard` and `divvun-dev-keyboard` list
> as a dependency, and over every `keyboard-*` repository. Every defect code
> found that `ldml.migrate.defects` does not list MUST be added to it first.
> The gap analysis surveyed 31 layouts; 17 dependency repositories were not
> present locally.
