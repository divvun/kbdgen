# Model to Windows layout DLL

The Windows layout DLL generator reads the abstract input `kbdl.input`.
For a v4 layout, that input is derived from the `windows` host document of
its compiled layout (`ldml.model.layout`) by the adapter below. The adapter
replaces `kbdl.input.bundle` for v4 layouts. It also carries the Windows-only
inputs that `input.windows-layers` was waiting for: extra modifiers, Shift
Lock, LRM/RLM, dead-key names, key-name overrides and the 49th key.

The DLL is the table fallback. Whatever cannot be expressed in its tables
remains the text service's job (`docs/spec/tsf.md`).

Sources: `docs/spec/kbdl.md` (`kbdl.input`, `kbdl.layers`, `kbdl.caps`,
`kbdl.dead-keys*`, `kbdl.scancodes*`, `kbdl.key-names`, `kbdl.locale`,
`kbdl.metadata*`); the gap analysis §7.2.

> [spec:kbdgen:def:ldml.kbdl.adapter]
> The adapter's input is a compiled layout with a `windows` keyboard K, the
> layout's `targets.windows`, and the bundle's project and `windows` target.
> Its output is one `kbdl.input`. A layout without a `windows` document has
> no Windows output. The adapter MUST NOT panic. It warns, or fails fatally
> in `kbdl`'s sense, naming the layout, the position, and the layer or
> marker.

> [spec:kbdgen:sem:ldml.kbdl.layers]
> Each `kbdl.layers` layer takes the layer that the engine's selection
> (`ldml.engine.modifiers`, `ldml.engine.altgr`, `Other`) picks for that
> layer's modifier state. Native-only layers are matched exactly, because
> the shortcut rule does not apply here.
>
> | `kbdl` layer | State |
> |---|---|
> | `default`, `shift` | {}, {`shift`} |
> | `caps`, `caps+shift` | {`caps`}, {`caps`, `shift`} |
> | `alt`, `alt+shift`, `alt+caps` | {`altR`} with `altgr`; plus `shift`; plus `caps` |
> | `ctrl` | {`ctrl`} |
> | extra *i*, extra *i*`+shift` | {`extra`*i+1*}; plus `shift` |
>
> A state that no layer serves is absent.

`kbd-engine` exposes no layer-selection API, so the adapter repeats the
engine's selection for these states; a test checks that both agree.

> [spec:kbdgen:sem:ldml.kbdl.positions+2]
> A `kbdl.input` position takes the key at the hardware position whose
> scan code equals that position's scan code in `kbdl.scancodes.iso`. A
> position whose scan code the form lacks is "no key": `B11` is filled
> only when the form has `73`, as `abnt2` and `jis` do, and `B00` is "no
> key" on a form without `56`, such as `us`, `jis` or `ks`. Keys with
> output at scan codes outside the 49 warn and are dropped, such as `jis`
> `7D`. The space row is fixed by `kbdl.vk-chars`, so a space position
> whose output is not U+0020 warns. One example is the NBSP of `space:
> {caps: \u{A0}}`. That row fills only the `default`, `shift` and `ctrl`
> columns, so in the `alt`, `alt+shift`, `alt+caps` and extra-modifier
> layers the DLL's space bar types nothing; a U+0020 output there does
> not warn.

> [spec:kbdgen:sem:ldml.kbdl.values+1]
> A key's value is set as follows:
>
> - no key, a gap, or an empty output: "no key"
> - output a single marker m: a dead value whose text is the marker's
>   *dead identity*, and the dead key's tree entry comes from
>   `ldml.kbdl.dead-tree`
> - plain text output: that text, not dead
> - text and markers mixed, or several markers: warn, "no key"
>
> The dead identity is the display of `\m{m}` if non-empty, else
> `flush[m]`. If both are missing or empty, or two markers have the same
> dead identity, the adapter fails fatally. The layers' *distinct values*
> are listed in first-occurrence order: `kbdl` layers in `kbdl.layers`
> order, then positions.

> [spec:kbdgen:sem:ldml.kbdl.dead-tree+2]
> The adapter derives each dead-key tree by running the engine
> (`ldml.engine.api`) with `CancelOrPass`, `Nfc` and host `windows`, on an
> empty context, pressing keys by `Id`. Pressing a dead key with marker m
> from `State::default()` MUST commit nothing and leave exactly m pending,
> else it is fatal. From that state, each distinct value (`ldml.kbdl.values`)
> other than `" "` is pressed through its first key. Let U be `flush[m]`,
> followed by the value's text unless it is dead. The entry is:
>
> - a branch, built the same way, when nothing is committed and the
>   trailing markers (`Model::pending_markers`) are exactly one marker n
>   not already on the path
> - none, when the committed text equals U. This includes re-pressing a
>   pending marker whose standalone is empty. For a value that is not
>   dead, Windows' unmatched behaviour (`kbdl.dead-keys`) does the same.
>   For a dead value it does not: the engine keeps the value's marker
>   pending, as macOS does, where Windows types both dead ids and keeps
>   nothing pending.
> - otherwise fatal, when n is already on the path: a cycle
> - a leaf, for other committed text with no marker pending; with markers
>   pending, it warns and is omitted
>
> The standalone child `" "` is the text committed by `Emit(" ")`.

> [spec:kbdgen:sem:ldml.kbdl.caps]
> The DLL keeps Windows' caps convention (`kbdl.caps`). It computes
> attributes from the derived `caps` and `caps+shift` values. So Caps+Shift
> on a `CAPLOK` key gives lowercase in the DLL, where the engine gives
> uppercase (`ldml.yaml.implied-layers`). The Windows API offers no way to
> express "Caps+Shift equals Shift" on a `CAPLOK` key. This is a documented
> exception of `tsf.test.differential`.

> [spec:kbdgen:req:ldml.kbdl.windows-inputs]
> The adapter MUST fill the Windows-only parts of `kbdl.input` from the
> model:
>
> - extra modifiers from `windows.extra_modifiers`, in order
> - `shiftLock` and `lrmRlm` from `windows.shift_lock` and
>   `windows.lrm_rlm`
> - dead-key names, mapped from `dead_key_names` through each marker's dead
>   identity
> - key-name overrides from `windows.key_names`, each mapped to the
>   `kbdl.key-names` entry with that English name
> - the decimal separator from `decimal`
>
> In v4 these come from `hardware.<variant>.extraModifiers`,
> `targets.windows.{shiftLock, lrmRlm, keyNames}` and `deadKeys.*.name`
> (`ldml.yaml.*`). The 49th key comes from `form: abnt2`.

> [spec:kbdgen:req:ldml.kbdl.metadata]
> Metadata follows `kbdl.metadata.bundle` and `kbdl.metadata.locale`. The
> only difference is where two values come from: `windows.config.id` and
> `windows.config.locale` are read from `targets.windows.id` and
> `targets.windows.locale`. The description and language name are the
> layout's display name for its primary language subtag.

> [spec:kbdgen:req:ldml.kbdl.classify]
> The adapter MUST report, as one information message per layout, what the
> DLL does not reproduce and only the text service can provide:
>
> - transforms that do not start with a marker, which work on visible text
> - `reorder` groups
> - `backspace` rules other than the generated one
> - dead-key leaves of more than one UTF-16 unit
> - mixed text-and-marker outputs
> - an enabled `normalization`
>
> Each message lists counts and up to ten examples, so authors know what
> users get without the text service, for example at the sign-in screen.

> [spec:kbdgen:req:ldml.kbdl.model-resource+1]
> For each layout, the `RT_RCDATA` resource of `tsf.data.resource` MUST hold
> the `ldml.model.encoding` of the layout's `windows` keyboard K. This is
> the same model the adapter reads, so the text service and the DLL derive
> from one source. A v3 layout first gets its model by in-memory migration
> (`ldml.migrate.*`), with nothing written to disk; its DLL tables still
> come from `kbdl.input.bundle`. If that migration fails, blocks, has no
> `windows` document or does not compile, the DLL is built without the
> resource, which leaves the text service inert for the layout
> (`tsf.data.locate`), and kbdgen warns with the reason, naming the
> blocking defect codes.
