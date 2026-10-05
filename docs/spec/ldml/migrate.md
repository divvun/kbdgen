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

> [spec:kbdgen:sem:ldml.migrate.dead-keys+1]
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
> Entries, roots and tokens are compared after escape decoding
> (`ldml.yaml.escape`). v3 compared them as written (Windows: the raw
> entry against the decoded token; the others: raw against raw), which was
> a defect (`kbdl.dead-keys`). A token that only the decoded comparison
> makes dead becomes dead, reported as M17.

> [spec:kbdgen:sem:ldml.migrate.fields+1]
> The following carry over:
>
> - `displayNames` and `decimal` unchanged
> - `keyNames` `space` and `return`, keeping their `\u{…}` spelling, which
>   v4 decodes
> - `longpress`, renamed `longPress`; a later entry whose output decodes
>   like an earlier one is dropped (M10)
> - `windows.config` {`locale`, `id`} → `targets.windows`
> - `chromeOS.config` → `targets.chromeOS`
> - `iOS.config` and `android.config` → `targets.iOS` and `targets.android`
> - `\s{…}` tokens unchanged; a desktop `\u{0}` stays `\u{0}`, which means
>   no key, and a touch `\u{0}` becomes `\s{gap}` (M14)
>
> Every field v3 never read is dropped and reported (M10), among them
> `windows.config.languageName` and `legacyName`, `windows.space`,
> `android.deadKeys`, other `keyNames`, and unknown fields at any level. A
> `config: null` is omitted silently. Every layout gets
> `impliedLayers: macOS` (default) and `normalization: disabled` (default),
> so neither is written.

> [spec:kbdgen:req:ldml.migrate.output+1]
> The migrator MUST write fresh v4 text, not round-trip the YAML through
> serde:
>
> - top-level fields in `ldml.yaml.schema` order
> - variants in `ldml.yaml.hosts` order, and layers in v3 order
> - rows aligned with single spaces
> - YAML strings quoted only where YAML requires, and where a YAML 1.1
>   reader would take them for a boolean or null (`yes`, `no`, `on`,
>   `off`, `y`, `n`, `true`, `false`, `null`, `~`, the empty string)
>
> Its output for a given input is byte-for-byte deterministic, and loading
> it gives no warnings beyond the reported defects. It writes each file in
> place under its v3 name, keeping no backup; git is the backup.
> `--dry-run` writes nothing and only reports.

## Defects

> [spec:kbdgen:req:ldml.migrate.defects+1]
> The migrator MUST detect and report each of these. A *block* leaves the
> layout unwritten. An entry is checked for M01 before M04. M13, M14 and
> M17 are reported once per layer and token.
>
> | Code | Defect | Action |
> |---|---|---|
> | M01 | dead key listed on a layer that lacks that token | drop, warn |
> | M02 | `deadKeys` names a layer the platform does not have | drop, warn |
> | M03 | transform root never dead on any layer | keep as unreferenced `deadKeys`, warn |
> | M04 | dead key listed with no transform entry | block: plain key or empty dead key? |
> | M05 | desktop layer with other than 48 tokens | block |
> | M06 | `\u` without braces (`\u00AD`); up to four hex digits follow it | rewrite as `\u{00AD}` (no digits: `\u{5C}u`), warn |
> | M07 | v2-format layout (no v3 platform sections) | block |
> | M08 | dead-key identity of more than one scalar | info: Windows cannot make it dead (`ldml.kbdl.values`) |
> | M09 | caps behaviour changes (`ldml.migrate.caps-diff`) | warn, listing positions |
> | M10 | v3 field not carried over | warn, naming the field |
> | M11 | comments dropped | warn, with line numbers |
> | M12 | iOS and Android dead keys differ for one layout | info |
> | M13 | macOS `\u{0}`, which output NUL and now means no key | warn |
> | M14 | `\u{…}` that a platform emitted as written and v4 decodes: iOS, Android or ChromeOS tokens (a touch `\u{0}` becomes `\s{gap}`), `longpress` entries, and transform strings of dead keys used on Windows, ChromeOS or iOS | warn |
> | M15 | iPad `alt` row not aligned with `default` | warn, drop the unaligned flicks |
> | M16 | dead-key root whose standalone differs from the root | info: `standalone` holds the `' '` child |
> | M17 | token that v3 never made dead because its dead-key entry is spelled otherwise, such as an escaped Windows `deadKeys` entry (`ldml.migrate.dead-keys`) | make it dead, warn |

> [spec:kbdgen:req:ldml.migrate.caps-diff+1]
> For each desktop variant, the migrator MUST compare old and new output
> for the states `caps`, `caps shift` and alt+`caps`, at each of the 48 ISO
> positions.
>
> - The old output comes from the v3 platform's rule: Windows per
>   `kbdl.caps` (CAPLOK, SGCAPS, CAPLOKALTGR); macOS per
>   `keylayout.keymaps`, taking the first layer in file order that matches
>   (for `caps shift`, `shift` or `caps+shift`), else the first layer;
>   ChromeOS by its runtime fallback (exact layer, then `caps`, then
>   `shift`, then `default`).
> - The new output comes from the v4 layers after `ldml.yaml.implied-layers`.
>
> Each position that differs is listed under M09, with both outputs. A
> typical case is Windows Caps+Shift on letters, which was lowercase and is
> now uppercase.

> [spec:kbdgen:def:ldml.migrate.report+1]
> The report goes to stdout as text, or with `--report <file>` as YAML,
> with the summary still printed. It has one entry per defect: code,
> layout file, YAML path, row and token where known, message, and action
> taken. A summary counts defects per code, gives each layout's outcome
> (written, dry run or blocked) and lists blocked layouts. The exit status
> is 0 when nothing is blocked and 1 otherwise. Unblocked layouts are still
> written.

> [spec:kbdgen:req:ldml.migrate.equivalence+1]
> For every layout it writes, the migrator MUST load the result as a v4
> layout and check it by running the engine. On each v4 host document it
> presses, at each of the 48 ISO positions (`macOS.space` is honoured but
> not pressed), every layer whose modifier state exists in v3 and is
> neither native-only nor a caps state (`ldml.migrate.caps-diff`), and every
> touch key and south flick. For each dead key it presses the dead key, then
> each compose input that a key types (on macOS only keys of layers with a
> `deadKeys` list), and on desktop then space. Each result MUST equal the
> v3 platform's documented output, unless a reported defect (M04–M17)
> explains the difference. An unexplained difference, a load warning no
> defect explains, or a failure to load is a bug in the migrator, reported
> as M99, and it blocks the layout.

> [spec:kbdgen:req:ldml.migrate.survey]
> Before v4 is declared frozen, `kbdgen ldml migrate --dry-run` MUST be run
> over every bundle that `divvun-keyboard` and `divvun-dev-keyboard` list
> as a dependency, and over every `keyboard-*` repository. Every defect code
> found that `ldml.migrate.defects` does not list MUST be added to it first.
> The gap analysis surveyed 31 layouts; 17 dependency repositories were not
> present locally.
