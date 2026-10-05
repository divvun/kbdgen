# Keyboard engine

The engine (`kbd-engine`) is kbdgen's runtime. Given a model
(`ldml.model.*`), a host-owned state, the text before the caret, and a key
event, it computes either *pass* or an edit. It is a pure function, so a
host can ask what a key would do without committing to it (`tsf.keys.claim`).
Every host runs this engine: the Windows text service (`docs/spec/tsf.md`),
macOS IMKit, the iOS and Android keyboard apps, wasm for ChromeOS and the
web, and IBus and Fcitx. On the LDML subset of the model it behaves as UTS #35
Part 7 specifies. The few places that go beyond LDML are marked
"Extension" or "Divergence".

Sources: UTS #35 Part 7 (§Element: layer and its modifier subsections,
§Element: key, §Element: flicks, §Element: transforms, §Markers, §Element:
transform, §Regex-like Syntax, §Replacement syntax, §Element: reorder,
§Backspace Transforms, §Normalization, §Platform Behaviors in Edge Cases);
`docs/spec/tsf.md` (`tsf.engine.api`, `tsf.engine.contract`,
`tsf.edit.ops`, `tsf.edit.reset`, `tsf.keys.*`); ECMAScript 2024 §22.2
(RegExp) for match priority; `docs/spec/kbdl.md` (`kbdl.locale`,
`kbdl.scancodes.extra-modifiers`).

## API

> [spec:kbdgen:def:ldml.engine.api]
> `kbd-engine` exposes:
>
> - `Model::from_bytes(&[u8]) -> Result<Model, Error>`, which decodes per
>   `ldml.model.encoding`
> - `Model::from_keyboard(Keyboard, Options) -> Result<Model, Error>`
> - `Model::context_len() -> usize`, which is at most 64
> - `Model::preserved_keys()`, the model's `emoji` key (`ldml.model.emoji`)
> - `Model::touch_set_for_width(u16) -> Option<usize>` and
>   `Model::touch_set_by_name(&str) -> Option<usize>`
> - the value type `State`, whose `State::default()` is the reset state
> - `Model::pending_markers(&State) -> Vec<&str>`, the names of the trailing
>   marker run, for tools such as `ldml.kbdl.dead-tree`
> - the pure function `Model::key(&self, &State, &Context, &KeyEvent) ->
>   (Action, State)`
>
> `Context` holds `text`: at most `context_len` scalar values before the
> caret, from the application or the host's cache. It also holds the flags
> `authoritative` and `at_start`. `at_start` says that `text` begins at a
> start of text, such as the document start or a paragraph boundary.
> `Action` is `Pass`, or `Edit { delete, insert, preedit, layer }`.

> [spec:kbdgen:def:ldml.engine.event]
> A `KeyEvent` is a key, modifiers and a `repeat` flag. The engine treats a
> repeat like any other press.
>
> | Key | Meaning |
> |---|---|
> | `Scan(u8)` | hardware key, PC/AT set 1 scan code without `E0` (`ldml.model.hardware`) |
> | `Decimal` | the numpad decimal key |
> | `Backspace` | Backspace |
> | `Touch { set, layer, row, col, gesture }` | a touch key; `gesture` is `Tap`, `LongPress(n)`, `MultiTap(n)` or `Flick(directions)` |
> | `Id { id, gesture }` | a key named by id; used by tests (`ldml.test.cldr`) |
> | `Emit(text)` | text handled as a key's output; used by tests |
> | `Commit` | the host is about to change the context (`ldml.engine.commit`) |
>
> Modifiers are `shift_l`, `shift_r`, `caps` (lock state), `ctrl_l`,
> `ctrl_r`, `alt_l`, `alt_r`, `altgr`, `cmd` and `extra: [bool; 3]`.
> `altgr` means the host treats Right Alt as AltGr.

> [spec:kbdgen:def:ldml.engine.state]
> A `State` holds `tail`: the last elements of the engine's view of the
> context, with markers. It covers at most `context_len` scalar values plus
> any markers among or after them. `State::default()` has an empty `tail`.
> A `State` has no other content. It implements `Clone`, `Eq` and
> `Default`. Hosts store it, and reset it to the default whenever:
>
> - the caret moves
> - focus changes
> - text changes by any means other than an applied `Edit`
> - they pass a key other than a lone modifier
>
> This matches `tsf.edit.reset`. On `Pass`, `key` returns the input state
> unchanged.

> [spec:kbdgen:def:ldml.engine.action]
> `Edit { delete, insert, preedit, layer }`: the host deletes `delete`
> scalar values before the caret, inserts `insert` as committed text, then
> shows `preedit` after the caret as an uncommitted composition, where empty
> means none. That is the triple (*d*, *s*, *p*) of `tsf.edit.ops`.
> Extension: `layer`, when present, is the touch layer the host shows next
> (`ldml.engine.touch`). `delete` never exceeds the scalar count of
> `Context.text`. `insert` and `preedit` never contain markers. An edit may
> be (0, "", *p*), which consumes a key with no text change.

> [spec:kbdgen:req:ldml.engine.contract]
> The engine MUST:
>
> - be deterministic, so equal models, states, contexts and events give
>   equal results
> - never panic and never perform I/O or read a clock
> - bound its work per event polynomially in the model size and
>   `context_len` (`ldml.engine.match`)
> - satisfy the `delete` and `preedit` bounds of `ldml.engine.action`
> - treat a non-authoritative context exactly like an authoritative one
>
> These meet `tsf.engine.contract`, with the divergences listed in
> `ldml.engine.tsf`.

## Context

> [spec:kbdgen:sem:ldml.engine.context]
> Each event first builds the working context C:
>
> 1. Let X be `Context.text`, or its NFD when normalization is `Enabled`.
> 2. If X ends with the plain text of `State.tail`, then C is the part of X
>    before that suffix followed by `tail`. This keeps `tail`'s markers.
> 3. Otherwise C is X with no markers. This applies LDML's rule that a
>    context change removes all markers.
>
> Afterwards, C is the context for every pattern. The new state's `tail` is
> cut from the processed C (`ldml.engine.output.segment`). LDML is silent on
> how a host proves the context unchanged; the suffix test is our answer.

## Hardware keys

> [spec:kbdgen:sem:ldml.engine.hardware]
> A `Scan(c)` event is handled in this order:
>
> 1. If the keyboard has no hardware set, or `c` is in no form row: pass.
>    The exception is the `B00` key bound as an extra modifier
>    (`ldml.engine.extra`).
> 2. Normalise the modifiers (`ldml.engine.extra`). Pass if the shortcut
>    rule applies (`ldml.engine.shortcuts`).
> 3. Select the layer (`ldml.engine.modifiers`, `ldml.engine.altgr`). Pass
>    if none is selected.
> 4. Take the key at `c`'s position in that layer. If there is none, or it
>    is a gap, or its output is empty, the result is (0, "", the current
>    preedit) with an unchanged state. This is macOS's "no output".
>    Hardware ignores `layer_id` and gestures.
> 5. Otherwise insert the output (`ldml.engine.insert`).

> [spec:kbdgen:sem:ldml.engine.modifiers]
> A modifier set S other than `Other` matches the modifier state M when all
> of these hold:
>
> - `shift` is in S exactly when `shift_l` or `shift_r` is set.
> - `caps` is in S exactly when `caps` is set.
> - `cmd` is in S exactly when `cmd` is set.
> - `extra`*n* is in S exactly when `extra[n-1]` is set.
> - For alt: `alt` needs `alt_l` or `alt_r`; `altL` needs `alt_l` and not
>   `alt_r`; `altR` needs `alt_r` and not `alt_l`; with no alt component,
>   both must be clear.
> - Ctrl works the same way.
>
> The selected layer is the non-native layer (`ldml.model.native`) with a
> matching set. If there is none, it is the `Other` layer, if any. Matching
> is exact, with no caps or shift fallback. LDML is silent on whether
> `altL` tolerates a held right Alt; here it does not.

> [spec:kbdgen:sem:ldml.engine.altgr]
> Extension. Suppose `altgr` and `alt_r` are set, ctrl is clear, and no
> layer matches. The engine then matches again with `ctrl_l` set, before
> trying `Other`. A Right Alt used as AltGr thereby reaches layers written
> either as `altR` (as in CLDR `mt.xml`) or as `ctrl alt` (as in `fr.xml`).
> On Windows, the host clears the Left Ctrl that the system synthesises with
> AltGr (`tsf.keys.altgr`). LDML is ambiguous about how AltGr relates to
> `ctrl alt`.

> [spec:kbdgen:req:ldml.engine.shortcuts]
> The engine MUST pass, without matching any layer, an event that meets any
> of:
>
> - `cmd` is set
> - a ctrl modifier is set and no alt modifier is
> - `alt_l` is set and the keyboard's host is `windows`, where Left Alt
>   drives menus
>
> So shortcuts reach the application, and native-only layers are never
> used for typing. Divergence from LDML: a `ctrl` layer that LDML would
> select is ignored. Divergence from `tsf.engine.contract`: Ctrl with AltGr
> reaches a layer that names both ctrl and alt.

> [spec:kbdgen:sem:ldml.engine.extra]
> Extension. Before matching, the bindings in `windows.extra_modifiers`
> rewrite the modifiers:
>
> - A `rightCtrl` binding to `extra`*n* moves `ctrl_r` into `extra[n-1]`.
> - A `capsLock` binding clears `caps`. The host sets `extra[n-1]` while the
>   key is held.
> - A `B00` binding works the same way: the host sets `extra[n-1]` while the
>   key is held, and a `Scan(0x56)` event gives (0, "", preedit), so the
>   modifier key is consumed.
>
> This mirrors `kbdl.scancodes.extra-modifiers`, so the engine and the
> layout DLL select the same columns.

> [spec:kbdgen:sem:ldml.engine.decimal]
> Extension. `Decimal` passes when the keyboard has no `decimal`, or when
> the shortcut rule applies. Otherwise it inserts `decimal` as a key output
> (`ldml.engine.insert`), whatever the other modifiers are.

## Touch keys

> [spec:kbdgen:sem:ldml.engine.touch]
> `touch_set_for_width(w)` returns the touch set with the greatest
> `min_device_width` ≤ `w`, where a set with none counts as 0. When the
> keyboard has no touch sets, it returns the hardware set presented as touch
> (`ldml.model.touch`).

> [spec:kbdgen:sem:ldml.engine.touch.gestures]
> A `Touch` event names its set, layer, row and column. A position with no
> key, a gap or a role key passes; role keys are the host's own. Otherwise
> the gesture resolves the key:
>
> - `Tap`: the key itself
> - `LongPress(n)`: `long_press[n-1]` for *n* ≥ 1, or `long_press_default`
>   for *n* = 0
> - `MultiTap(n)`, for *n* ≥ 2: entry `(n-1) mod (len+1)` of
>   [key, `multi_tap`…]
> - `Flick(d)`: the flick segment whose directions equal `d`
>
> A gesture with no target consumes the key with no output. The resolved
> key's output is inserted. Its `layer_id`, if any, is returned as
> `Edit.layer` after output processing (LDML: output, then switch). Gestures
> defined on the target key are ignored, and LDML is silent past the end of
> a multi-tap list. A host that shows each tap restores its pre-tap state
> and context before sending the next `MultiTap`.

> [spec:kbdgen:sem:ldml.engine.test-keys]
> `Id { id, gesture }` resolves the key with that id from the key table,
> whatever the layer, then applies the gesture as in `ldml.engine.touch`.
> An unknown id consumes the event with no output, which matches
> keyboardTest3's "as if the user attempted" rule. `Emit(text)` inserts
> `text`, already escape-decoded, as a key output.

## Output processing

> [spec:kbdgen:sem:ldml.engine.insert]
> To insert an output O:
>
> 1. Append O to C.
> 2. With normalization `Enabled`, normalize C (`ldml.engine.normalization`).
> 3. If O is non-empty (a lone marker counts), run the `simple` transform
>    groups (`ldml.engine.transforms`).
> 4. Compute the edit (`ldml.engine.output.segment`).
>
> LDML is silent on whether a key with no output runs transforms. Here it
> does not, so layer-switch keys never rewrite text.

> [spec:kbdgen:sem:ldml.engine.transforms]
> The groups are processed in order, each on the result of the previous
> one:
>
> - A `Rules` group applies the first of its rules whose `from` matches the
>   end of C (`ldml.engine.match`). It replaces the matched elements with
>   the expanded `to` (`ldml.engine.replace`). If no rule matches, C is
>   left as it is.
> - A `Reorder` group applies `ldml.engine.reorder`.
>
> With normalization `Enabled`, C is normalized after each group. At most
> one rule applies per group, and processing never loops back to an earlier
> group.

> [spec:kbdgen:sem:ldml.engine.match]
> A pattern P matches C when, for some start position s, P matches the
> elements C[s..] exactly. The match used is the one an ECMAScript `u`-flag
> search for `(?:P)$` finds: the smallest s, and at that s the first
> success in backtracking order, with greedy quantifiers and alternatives
> taken left to right. Set items are tried in order. `^` matches only at
> s = 0 with `at_start`.
>
> On element kinds:
>
> - Scalar atoms (`Char`, `Any`, `Fixed`, a class's ranges) never match a
>   marker.
> - `Marker(m)` matches marker m, and `AnyMarker` matches any marker.
> - A negated class never matches a marker.
>
> The matcher MUST take polynomial time, for example by memoising over
> (pattern node, position).

> [spec:kbdgen:sem:ldml.engine.replace]
> A replacement expands as follows:
>
> - `Text` gives its elements.
> - `Group(n)` gives the elements captured by group *n*, markers included,
>   or nothing if the group did not participate. `Group(0)` is the whole
>   match.
> - `MapSet{group, from, to}` finds the first item of `from`, in order,
>   whose elements equal the capture, and gives the item at the same index
>   of `to`.
>
> The result replaces exactly the matched elements.

> [spec:kbdgen:sem:ldml.engine.reorder]
> A `Reorder` group runs the marker algorithm of §Normalization and
> Markers over the whole of C:
>
> 1. Remove the markers.
> 2. Sort the plain text as §Element: reorder specifies: sort keys
>    (primary, index, tertiary, quaternary), runs, prebase and
>    tertiary-base handling.
> 3. Re-add the markers.
>
> At each position the highest-priority rule applies whose `from` matches
> there and whose `before` matches just before it. Positions no rule
> matches are bases (order 0, tertiary 0). LDML is ambiguous about the
> filler base it mentions for a lone prebase. v1 inserts none, which is
> deferred (`ldml.scope.deferred`).

> [spec:kbdgen:sem:ldml.engine.normalization]
> With normalization `Enabled`, the engine converts text to NFD with the
> marker algorithm of §Normalization and Markers at three points:
>
> - the context (`ldml.engine.context`)
> - after appending an output
> - after each transform group
>
> Markers are glued to the following scalar value, or to the end. With
> `Disabled`, the engine changes no text: matching, replacement and output
> use exactly the scalar values that the model and the context contain.
> v4 keyboards default to `Disabled` (`ldml.yaml.normalization`).

> [spec:kbdgen:def:ldml.engine.output.form]
> `Options.output_form` is `Nfc` (the default) or `Nfd`. It applies only to
> keyboards with normalization `Enabled`, whose inserted text is converted
> to that form. LDML lets the calling platform choose the output form.
> Divergence: `tsf.engine.contract` forbids normalization outright. That
> holds for every `Disabled` keyboard, which includes every v4 default. Only
> keyboards that keep LDML's default normalization, such as imported CLDR
> keyboards, normalize.

> [spec:kbdgen:sem:ldml.engine.output.segment]
> The edit compares T (`Context.text`, as given) with the processed C′.
>
> - **`Disabled`:** let *i* be the length of the longest common prefix of T
>   and plain(C′). Then `delete` is |T| − *i* and `insert` is plain(C′)[*i*..].
> - **`Enabled`:** let *i* be the largest index such that:
>   - *i* = |T|, or the canonical decomposition of T[*i*] starts with a
>     scalar of canonical combining class 0;
>   - NFD(T[..*i*]) is a prefix of plain(C′);
>   - the rest of plain(C′) is empty or starts with a class-0 scalar.
>
>   Then `delete` is |T| − *i*, and `insert` is the rest converted to the
>   output form.
>
> So normalization touches only text from the caret's segment onwards. The
> new `tail` is the end of C′ covering `context_len` scalar values, plus any
> markers among or after them.

## Backspace

> [spec:kbdgen:sem:ldml.engine.backspace]
> For `Backspace`:
>
> 1. Extension: if `windows.lrm_rlm` is set and `shift_l` (or else
>    `shift_r`) is held, insert U+200E (or else U+200F) as a key output.
>    This is `kbdl.locale`'s `KLLF_LRM_RLM`.
> 2. Otherwise, pass if the shortcut rule applies.
> 3. Otherwise, run the `backspace` groups over C as
>    `ldml.engine.transforms` does, with nothing appended.
> 4. If any rule in any group matched, the edit is computed as usual.
> 5. Otherwise the default applies (`ldml.engine.backspace.default`).
>
> The `simple` groups are not run afterwards. LDML is ambiguous: it says
> simple transforms run "if processed". We read that as not running them,
> as Keyman does.

> [spec:kbdgen:sem:ldml.engine.backspace.default]
> `Options.backspace` chooses the default when no backspace rule matched:
>
> - `CancelOrPass` (the default; the macOS rule): if C ends with one or
>   more markers, remove that trailing run, giving (0, "", ""). Otherwise
>   pass, so the application deletes as it normally does, usually by
>   grapheme.
> - `CodePoint` (LDML's §Default Backspace Transform, used by
>   `ldml.test.cldr`): delete the last scalar of C together with the
>   markers adjoining it on either side. When C has no scalar, remove any
>   markers, or pass if there are none.
>
> Divergence: LDML's default deletes the last character along with a
> pending marker. `CancelOrPass` cancels only the marker.

## Pending markers

> [spec:kbdgen:sem:ldml.engine.preedit]
> Extension. After every event, `preedit` is the concatenation of
> `flush[m]` for each marker m in the trailing marker run of C′, in order.
> Markers without a flush output contribute nothing. Because the preedit
> equals what `Commit` would insert, a host such as TSF that commits
> preedit text on reset (`tsf.edit.reset`) gets the same result as an
> explicit `Commit`. macOS likewise shows a pending dead key as marked text.

> [spec:kbdgen:sem:ldml.engine.commit]
> Extension. `Commit` replaces the trailing marker run of C with its flush
> outputs, giving (0, *flush text*, ""). It removes every other marker, and
> runs no transforms. With no trailing markers it gives (0, "", ""). Hosts
> without preedit send `Commit` before:
>
> - passing a frame key (Enter, Tab, arrows, Escape) to the application
> - losing focus
>
> This is macOS's behaviour: a dead key followed by a key with no
> transition emits the terminator. LDML is silent; its markers simply
> vanish on a context change.

> [spec:kbdgen:thm:ldml.engine.no-markers-out]
> No `insert` or `preedit` contains a marker. Both are built from plain(C′)
> or from flush texts, and the model's text type keeps markers out of band
> (`ldml.model.text`). So markers never reach an application, as
> §Markers requires.

## TSF boundary

> [spec:kbdgen:req:ldml.engine.tsf]
> `kbd-engine` is the engine crate of `tsf.engine.api`, and its API is a
> superset of that boundary:
>
> - `KeyEvent` adds touch, `Id`, `Emit` and `Commit` keys, and left/right
>   Shift. A TSF host sends AltGr as `alt_r` with `altgr` set.
> - `Context` adds `at_start`; a host that cannot tell MUST pass `false`.
> - `Edit` adds `layer`, which TSF ignores.
> - `preserved_keys()` reads `ldml.model.emoji`.
>
> The divergences are normalization of `Enabled` keyboards
> (`ldml.engine.output.form`) and Ctrl with AltGr (`ldml.engine.shortcuts`).
> `tsf.engine.interim` is superseded: this engine already does what it
> lists, including caps through v4's implied layers.
