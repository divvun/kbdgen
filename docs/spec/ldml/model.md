# Keyboard model

The model is kbdgen's keyboard: a superset of an LDML `keyboard3` document.
It is stored after resolution, so on the device nothing is left to resolve:

- imports are spliced in, and implied keys and forms are added
- string variables are substituted
- escapes are decoded and text is normalized as the settings require
- markers are interned
- transform syntax is parsed into pattern trees
- reorder rules are merged and sorted

The LDML subset keeps LDML's meaning exactly. The superset adds the
following, all first-class:

- the modifier components `cmd` and `extra1`–`extra3`
- native-only layers
- the host a keyboard is built for
- the numpad decimal key
- flush outputs for markers
- touch roles, labels and bottom-row policy
- dead-key names
- the Windows options

The engine (`ldml.engine.*`) and the native compilers (`ldml.kbdl.*`) read
only this model. A layout compiles to one keyboard per *host document*
(`ldml.yaml.hosts`). Together these form a compiled layout.

Sources: UTS #35 Part 7 §Element Hierarchy, §Markers, §Normalization,
§Regex-like Syntax, §Element: reorder; `ldmlKeyboard3.dtd`;
`docs/spec/kbdl.md` (`kbdl.input`, `kbdl.layers`, `kbdl.scancodes.extra-modifiers`,
`kbdl.locale`, `kbdl.key-names`); `docs/spec/tsf.md` (`tsf.data.version`,
`tsf.engine.model`).

## Text

> [spec:kbdgen:def:ldml.model.text]
> A *text* is a sequence of elements. Each element is either a Unicode
> scalar value or a *marker reference*, which is a `u16` index into the
> keyboard's marker table. Markers are out of band: they are never mapped to
> private-use characters, so application text may contain any scalar value.
> The marker table lists marker names (NMTOKEN) in order of first appearance
> in document order. `\m{.}` is not an entry; it occurs only in patterns.
> The *plain text* of a text is its scalar values with every marker removed.

## Keyboard

> [spec:kbdgen:def:ldml.model.keyboard]
> A keyboard has these fields. Every index refers to a table of the same
> keyboard.
>
> | Field | Content | Origin |
> |---|---|---|
> | `host` | `windows` `macOS` `chromeOS` `linux` `iOS` `android` `web`, or none for a keyboard read from foreign XML | Extension |
> | `locale`, `locales` | BCP 47 tag; additional tags, in order | LDML |
> | `conforms_to`, `version` | 45–49; optional semver | LDML |
> | `info` | `name` (required), `author`, `layout`, `indicator`, `attribution` | LDML |
> | `normalization` | `Enabled` (the LDML default) or `Disabled` | LDML |
> | `markers` | `ldml.model.text` | LDML |
> | `keys` | `ldml.model.keys` | LDML + roles |
> | `flicks` | `{id, segments: [(directions, key)]}`, with directions from `n` `ne` `e` `se` `s` `sw` `w` `nw` | LDML |
> | `displays` | `ldml.model.displays` | LDML |
> | `hardware` | optional `ldml.model.hardware` | LDML + Extension |
> | `touch` | `ldml.model.touch` | LDML + Extension |
> | `sets`, `classes` | `ldml.model.pattern` | LDML |
> | `simple`, `backspace` | `ldml.model.transforms` | LDML |
> | `context_len` | `ldml.model.context-len` | Extension |
> | `decimal` | optional numpad decimal output text | Extension |
> | `flush` | marker → text (`ldml.model.flush`) | Extension |
> | `dead_key_names` | marker → name | Extension |
> | `windows` | `ldml.model.windows` | Extension |
> | `emoji` | `ldml.model.emoji` | Extension |

> [spec:kbdgen:def:ldml.model.keys]
> The key table holds every key the document defines, implied keys first,
> then the rest in document order. Each key has:
>
> - `id`, an NMTOKEN, unique in the table
> - `output`, a text, empty when absent
> - `gap`
> - `layer_id`, an optional touch layer id
> - `width`, in thousandths of a key width (default 1000)
> - `stretch`
> - `long_press` (key indices in order) and `long_press_default`
> - `multi_tap` (key indices)
> - `flick` (an optional flick index)
> - `role`: Extension, one of `shift` `backspace` `return` `tab` `caps`
>   `keyboard` `symbols` `shiftSymbols`, which hosts draw and handle
>   (`ldml.yaml.touch.roles`)
>
> Keys that no row references are kept, because tests press keys by id.

> [spec:kbdgen:def:ldml.model.displays]
> `displays` lists the `display` entries in order. Each pairs a target with
> a display string. The target is either an output text, markers allowed, or
> a key index (`keyId`). Display strings are decoded and have their
> variables substituted, but are never normalized. Where targets repeat, the
> last entry wins.
>
> `display_base` is `displayOptions@baseCharacter`. `labels` (Extension) has
> optional `space` and `return` label strings, which hosts show on those
> keys. The `space` label is also exported as LDML
> `<display keyId="space">`.

## Layers

> [spec:kbdgen:def:ldml.model.hardware]
> The hardware layer set has:
>
> - `form`: an id and rows of scan codes (`u8`, PC/AT set 1 without the
>   `E0` prefix), implied or custom
> - `min_device_width`: optional
> - `layers`: each with an optional `id`, its modifier sets
>   (`ldml.model.modifiers`), and rows of key indices
>
> Row *r* of a layer is at most as long as form row *r*. Position (*r*,
> *c*) is scan code `form.rows[r][c]`. A position past a row's end, or in a
> row the layer omits, has no key. A keyboard has at most one hardware set.

> [spec:kbdgen:def:ldml.model.modifiers]
> A modifier set is either `Other` or a set of components. The components
> are LDML's `alt`, `altL`, `altR`, `caps`, `ctrl`, `ctrlL`, `ctrlR` and
> `shift`, plus the Extension components `cmd` (Command, Windows or Super)
> and `extra1`, `extra2`, `extra3` (`ldml.model.windows`). `none` is the
> empty set.
>
> Each layer's sets are stored sorted by size, then by component order. The
> component order is the list above, which is LDML's canonical order
> followed by the extensions. These combinations are rejected:
>
> - `alt` with `altL` or `altR`, which LDML only warns about
> - `ctrl` with `ctrlL` or `ctrlR`, likewise
> - left with right sides mixed, such as `altL ctrlR` or `altL altR`

> [spec:kbdgen:def:ldml.model.native]
> Extension: a hardware layer is *native-only* when any of its sets contains
> `cmd`, or contains `ctrl`, `ctrlL` or `ctrlR` with no alt component. Such
> layers exist for native tables: shortcut mappings and C0 controls for
> `kbdl` column 2 and the `.keylayout` command maps. The engine never
> selects them, because its shortcut rule passes those events
> (`ldml.engine.shortcuts`). LDML export writes them only in kbdgen's
> namespace (`ldml.xml.special`). An LDML-only consumer never sees them, so
> it never swallows Ctrl or Cmd shortcuts.

> [spec:kbdgen:def:ldml.model.touch]
> `touch` lists the touch layer sets in ascending `min_device_width`, with a
> set that has none first. Widths are distinct whole millimetres from 1 to
> 999. A set has:
>
> - Extension: a `name` (`phone`, `tablet`, …) and a `bottom_row` of `host`
>   or `authored`
> - its layers, each an `id` with rows of key indices of any length
> - `base`, the index of the layer with id `base`
>
> A keyboard with no touch set may present its hardware set as touch
> (`ldml.engine.touch`). The base is then the layer whose sets are exactly
> {`none`}.

> [spec:kbdgen:def:ldml.model.windows]
> Extension. `windows` carries `kbdl.input`'s Windows-only parts, used by
> the engine on Windows and by the `kbdl` adapter:
>
> - `extra_modifiers`: an ordered list of at most three distinct physical
>   keys (`rightCtrl`, `capsLock`, `B00`); the *i*-th binds component
>   `extra`*i+1* (`kbdl.scancodes.extra-modifiers`)
> - `shift_lock` and `lrm_rlm` (`kbdl.locale`)
> - `key_names`: entry name → replacement name, where entry names are the
>   English names of `kbdl.key-names` (`Caps Lock`, `Right Alt`, …)
>
> Dead-key names live in `dead_key_names`. The 49th key is simply the
> `abnt2` form's `73` position.

> [spec:kbdgen:def:ldml.model.emoji]
> Extension. `emoji` carries what `tsf.emoji.trigger` and `tsf.emoji.data`
> need from the bundle:
>
> - `key`: an optional preserved key, given as a scan code and a modifier
>   set (`ldml.model.modifiers`)
> - `annotations`: a list of `{emoji, name, keywords}` entries in the
>   layout's language, in source order
>
> The engine never interprets `annotations`. `Model::preserved_keys()`
> returns `key`. The emoji picker's own behaviour belongs to `tsf.emoji.*`.

## Transforms

> [spec:kbdgen:def:ldml.model.transforms]
> `simple` and `backspace` each hold an ordered list of groups. A group is
> one of:
>
> - `Rules`: a non-empty ordered list of `{from: Pattern, to: Replacement}`;
>   an absent `to` is the empty replacement
> - `Reorder`: reorder rules after LDML's split-and-merge, sorted into match
>   priority (longest `from`, then the longest summed `before`). Each rule is
>   `{before: [Class], from: [Class], order: [i8], tertiary: [i8],
>   tertiary_base: [bool], pre_base: [bool]}`, with every list padded to the
>   length of `from`. A Class is a scalar value or a set of scalar ranges.

> [spec:kbdgen:def:ldml.model.pattern]
> A Pattern is an optional start anchor plus an alternation of sequences.
> Each item may carry a quantifier `{min,max}`, with `0 ≤ min ≤ max ≤ 9` and
> `max ≥ 1`; `?` means `{0,1}`. Atoms are:
>
> - `Char(c)`
> - `Any`, for `.`
> - `Class(i)`: an entry of `classes`, holding ranges, a negation flag and
>   marker members
> - `Fixed(k)`: one of `\s \S \d \D \w \W \t \r \n \f \v`, with LDML's fixed
>   definitions
> - `Marker(m)` or `AnyMarker`
> - `Set(s)`: an entry of `sets`, a list of texts tried as alternatives in
>   order
> - `Group` (non-capturing) or `Capture(n)`, with `n` from 1 to 9
>
> A Replacement is a sequence of `Text(text)`, `Group(n)` (0 is the whole
> match) and `MapSet{group, from: s, to: t}`.

> [spec:kbdgen:req:ldml.model.context-len]
> `context_len` is one more than the largest number of text elements that
> any pattern of `simple` or `backspace` can match. It also covers
> `ldml.engine.output.segment`. A model whose `context_len` exceeds 64 MUST
> be rejected, because `tsf.engine.api` bounds a host context at 64 scalar
> values. Every pattern is bounded, since LDML forbids unbounded
> quantifiers, so the maximum is computable. LDML is silent on any limit.

> [spec:kbdgen:def:ldml.model.flush]
> Extension. `flush` maps a marker to the text that a pending marker stands
> for when input is interrupted. The engine shows it as preedit
> (`ldml.engine.preedit`) and commits it (`ldml.engine.commit`). v4 dead keys
> set it to their standalone output. Keyboards read from foreign XML have
> none, so their markers are dropped as LDML specifies for a context change.

## Compiled layouts

> [spec:kbdgen:def:ldml.model.layout]
> A *compiled layout* has:
>
> - `tag`, the layout's normalised language tag
> - `display_names`, a map from tag to name
> - `keyboards`, a list of distinct keyboards
> - `hosts`, a map from each host to a keyboard index
>
> Hosts whose keyboards are equal in every field except `host` share one
> entry, which then has no `host`. Its engine follows the rules for no host,
> and each consumer supplies its own host. Native compilers and host
> packaging read the keyboard of their host.

## Invariants and encoding

> [spec:kbdgen:req:ldml.model.invariants]
> Building or decoding a keyboard MUST check every invariant below, and on a
> violation return an error naming it. Neither may panic.
>
> - Indices are in range, and key ids are unique.
> - `long_press_default` is in `long_press`, and no key lists itself in
>   `multi_tap`.
> - Gap keys have no output, `layer_id`, gestures or display.
> - No two non-native hardware layer sets overlap (`ldml.engine.modifiers`),
>   and at most one layer has `Other`.
> - `extra`*n* is used only if `windows.extra_modifiers` has an *n*-th entry.
> - Rows fit the form. Every touch set has a `base` layer. Every `layer_id`
>   names a layer of some touch set.
> - Patterns never match the empty text. They have at most nine captures,
>   none nested. A `MapSet` group captures exactly one `Set`, and both of its
>   sets have the same length.
> - With `Enabled` normalization, texts are NFD (`ldml.model.nfd`).
> - `context_len` ≤ 64.

> [spec:kbdgen:req:ldml.model.nfd]
> With `Enabled` normalization, the following MUST be in NFD after the marker
> algorithm of §Normalization and Markers:
>
> - key outputs
> - pattern `Char` runs and `Set` items
> - replacement texts
> - display targets
> - reorder classes
> - `decimal` and `flush` texts
>
> Each class range MUST contain only NFD scalar values (`ldml.xml.nfd-classes`).
> With `Disabled`, the same texts are the authored scalar values after
> escape decoding, unchanged.

> [spec:kbdgen:req:ldml.model.deterministic]
> The model uses only `Vec`, `BTreeMap`, integers, `bool`, `String` and
> enums. Widths are stored as integer thousandths. Equal resolved documents
> MUST yield equal models. Equal models MUST encode to identical bytes on
> every platform.

> [spec:kbdgen:syn:ldml.model.encoding]
> A keyboard's binary form, the engine model of `tsf.data.resource`, is:
>
> 1. the ASCII magic `DVKB`
> 2. major version 1 and minor version 0, each a little-endian `u16`
>    (`tsf.data.version`)
> 3. the `postcard` encoding of the keyboard
>
> A minor version may only append fields at the end of the top-level
> structure, and a reader ignores trailing bytes. Divergence:
> `tsf.engine.model` reserves major 1 for an interim model. There is none,
> so this model is major 1. Files are named `<tag>.<host>.dvkb`
> (`ldml.cli.compile`).

> [spec:kbdgen:req:ldml.model.decode]
> Decoding MUST return an error, and MUST NOT panic or allocate without
> bound, on:
>
> - a wrong magic or an unknown major version
> - truncated or invalid postcard data
> - a declared length beyond the remaining input
> - a violated invariant
>
> It MUST accept a higher minor version of a known major version.
