# Model to ChromeOS input method

The ChromeOS target (`docs/spec/chromeos.md`) writes an input-method
extension whose service worker runs the engine (`ldml.engine.*`),
compiled to WebAssembly, on each layout's `chromeOS` keyboard. Unlike
the Windows layout DLL and the macOS `.keylayout`, nothing is translated
into a native table: the extension types what the engine types, and only
what a ChromeOS key event cannot carry is lost.

The Windows text service is the reference host (`docs/spec/tsf.md`,
`crates/kbd-tsf`): keys are identified by scan code, edits are applied as
the triple of `ldml.engine.action`, and the state resets as
`ldml.engine.state` lists. Where ChromeOS differs, this document says
how.

Sources: `docs/spec/ldml/kbdl.md` and `docs/spec/ldml/macos.md` as the
template for an adapter; `docs/spec/tsf.md` (`tsf.keys.identity`,
`tsf.edit.ops`, `tsf.edit.reset`, `tsf.edit.cache`,
`tsf.security.disabled`); the Chrome extension reference for
`chrome.input.ime`, the `input_components` manifest key and the Manifest
V3 `content_security_policy`; UI Events KeyboardEvent `code` values.

## Input

> [spec:kbdgen:def:ldml.chromeos.input]
> The target's layouts are, in bundle layout order, every v4 layout with a
> `chromeOS` host document (`ldml.yaml.hosts`: a `chromeOS` or `default`
> hardware variant) and every v3 layout with a `chromeOS` section. A v3
> layout is migrated in memory (`ldml.migrate.*`), with nothing written,
> and its migrated text is lowered the same way. Each layout's `chromeOS`
> document is compiled (`ldml.model.layout`) and its keyboard encoded per
> `ldml.model.encoding` after the engine has loaded it. Its input
> component takes `targets.chromeOS.locale` (else `en-US`) as its
> language and `targets.chromeOS.xkbLayout` (else `us`) as its layout,
> from the v3 `chromeOS.config` after migration. Fatal, naming the
> layout: a v3 migration that blocks (listing the blocking defect codes),
> that has no `chromeOS` keyboard, or that does not compile; a keyboard
> without hardware layers. A v4 layout without a `chromeOS` document has
> no ChromeOS output.

## The engine module

> [spec:kbdgen:req:ldml.chromeos.wasm]
> `crates/kbd-wasm` MUST be a `cdylib` over `kbd-engine` and `kbd-model`
> that builds for `wasm32-unknown-unknown` with no imports. It exports a
> C ABI of integers, with no generated bindings:
>
> - `kbd_input(len)` sizes an input buffer and gives its address; the host
>   writes a model or a UTF-8 context there before the call that reads it
> - `kbd_load(len)` loads a model into a session, the model with its own
>   `State`, and gives a handle ≥ 1, or 0 with the error as output;
>   `kbd_unload(handle)`
> - `kbd_key(handle, key, modifiers, len, flags)` runs `Model::key` with
>   the session's state and keeps the next state. `key` is a scan code
>   below `0x100`, `0x100` `Decimal`, `0x101` `Backspace` or `0x102`
>   `Commit`; `modifiers` has one bit per `ModifierState` field, from
>   `shift_l` (bit 0) to `extra3` (bit 11); `flags` bit 0 is `at_start`
>   and bit 1 `authoritative`. It returns 0 for `Pass`, 1 for `Edit`, or
>   a negative error for no such session, an unknown key or modifier bit,
>   or a context that is not UTF-8
> - after an edit, `kbd_delete()`, and `kbd_output()` / `kbd_output_len()`
>   holding the inserted text then the preedit, split at `kbd_insert_len()`
> - `kbd_reset`, `kbd_pending` (the count of `Model::pending_markers`),
>   `kbd_context_len`, `kbd_extra_modifiers` (two bits per bound
>   component: 1 Right Ctrl, 2 Caps Lock, 3 `B00`) and `kbd_shift_lock`
>
> Sessions use `Nfc`, `CancelOrPass` and host `chromeOS`. No export may
> panic. The crate keeps `kbd-engine`'s lints against panics, `unwrap`,
> `expect`, indexing and `unreachable!`; its only unsafe construct is
> `no_mangle` on the exports, and no export dereferences a raw pointer.
> Plain exports suffice for a dozen integer functions, so there is no
> `wasm-bindgen` and no tool beyond cargo.

## Keys

> [spec:kbdgen:sem:ldml.chromeos.keys]
> A key down is classified by `KeyboardEvent.code`, never by `key`, as
> `tsf.keys.identity` classifies by scan code:
>
> - `ShiftLeft`, `ShiftRight`, `ControlLeft`, `ControlRight`, `AltLeft`,
>   `AltRight`, `MetaLeft`, `MetaRight`, `OSLeft`, `OSRight` and
>   `CapsLock` are modifiers: they pass and reset nothing
> - `Backspace` is `Backspace`; `NumpadDecimal` is `Decimal`, unless its
>   `key` is `Delete` (Num Lock off), when it is a frame key
> - a code with a PC/AT set 1 scan code without `E0` is `Scan` of it: the
>   writing-system keys (`Backquote` `29` … `Slash` `35`, `IntlBackslash`
>   `56`, `IntlRo` `73`, `IntlYen` `7D`), `Space` `39`, and the other
>   non-extended keys (`Escape`, `Tab`, `Enter`, `F1`–`F12`, the numpad
>   digits and operators, `NumpadEqual`, `NumpadComma`, `KanaMode`,
>   `Lang1`, `Lang2`, `Convert`, `NonConvert`, `ScrollLock`), which the
>   engine passes unless a form names them
> - every other code, such as the arrows, `Delete` or `NumpadEnter`, is a
>   frame key: it passes and resets
>
> Key ups pass.

> [spec:kbdgen:sem:ldml.chromeos.modifiers]
> The event's flags say which modifiers are down, and the key downs and
> ups seen of the modifier keys say which side, Left when none was seen:
>
> - `shiftKey` and `ctrlKey` give `shift_l`/`shift_r` and
>   `ctrl_l`/`ctrl_r`
> - Right Alt is AltGr: `alt_r` and `altgr` when `altgrKey` is set or
>   `altKey` is set with `AltRight` held, since the XKB layout beneath
>   reports Right Alt either way. `alt_l` when `altKey` is set with
>   `AltLeft` held or without `AltRight` held. On ChromeOS, Left Alt is the
>   shortcut modifier, so the engine passes it (`ldml.engine.shortcuts`)
> - `cmd` while the Search key (`MetaLeft`, `MetaRight`) is held, so
>   Search shortcuts pass
> - `caps` is `capsLock`, or with `windows.shift_lock` the host's own
>   Shift Lock, which Caps Lock sets and Shift clears
>   (`tsf.keys.locale-flags`)
> - `extra[n-1]` while the bound `CapsLock` or `IntlBackslash` key is
>   held; a bound Right Ctrl stays `ctrl_r`, which the engine rebinds
>   (`ldml.engine.extra`)
>
> The keys seen held are forgotten on focus, blur, activation and
> deactivation, since their key ups may go elsewhere.

## Semantics

> [spec:kbdgen:sem:ldml.chromeos.semantics]
> A key down while no text field has focus passes. Otherwise a frame key,
> or an engine key the engine passes, *passes*: if a composition is shown
> or markers are pending, the edit of a `Commit` is applied first; then
> the state resets and the key goes to the application (the listener
> returns `false`). An engine edit is applied and the key is handled:
>
> 1. If a composition is shown and the edit deletes, inserts, or shows no
>    preedit, `clearComposition`.
> 2. `deleteSurroundingText` with offset −*u* and length *u*, where *u* is
>    the UTF-16 length of the last `delete` scalar values of the context
>    sent (`tsf.edit.units`); ChromeOS counts characters in UTF-16 units.
> 3. `commitText` of `insert`.
> 4. `setComposition` of the preedit with the cursor at its end, when it
>    differs from the shown one.
>
> A `password` field shows no preedit and keeps no context
> (`tsf.security.disabled`). A key that arrives before the engine and the
> layout's model have loaded is answered with `keyEventHandled` once they
> have, and keys apply in arrival order. A failed IME call resets the
> state and the context. Focus, blur, activation, deactivation and
> `onReset`, which ChromeOS sends when it ends the input session (the caret
> clicked elsewhere, the composition settled by ChromeOS), reset the state
> and forget the context. Nothing is pending across a blur: ChromeOS
> invalidates the context before the glue could commit to it, and keeps a
> shown composition as text. ChromeOS may stop an idle service worker;
> the engine state and the record of a shown composition go with it, so a
> dead key left pending that long is dropped. Typed text and keys are
> never logged.

> [spec:kbdgen:sem:ldml.chromeos.context]
> ChromeOS reports the text around the caret only afterwards, through
> `onSurroundingTextChanged`, at most 100 characters each way, and does
> not say whether that text includes the composition. The glue keeps the
> committed text before the caret, at most 128 scalar values: updated by
> its own edits, kept when a key passes, since a change brings a
> notification, and forgotten on reset. Each of its IME calls records the
> state it leads to. A reading whose text before the caret agrees with
> one of those states, with the composition stripped or not, where two
> texts agree when one ends with the other, is of that state: of the
> latest, it completes the context; of an earlier one, it lags and is
> dropped. Any other reading replaces the context; the engine state is
> kept, because the engine drops markers whose text is gone
> (`ldml.engine.context`). A reading with a selection makes the next key
> reset the state and start from an empty context, as typing replaces the
> selection (`tsf.edit.session`). `at_start` is set when the reading's
> `offset` is 0. A caret moved within text that the context still agrees
> with goes unnoticed, unless ChromeOS resets.

## Reporting

> [spec:kbdgen:req:ldml.chromeos.classify]
> The engine runs whole, so transforms, `reorder`, backspace rules,
> markers, normalization, LRM/RLM and extra modifiers all work. kbdgen
> MUST warn, once per layout and feature, with a count and up to ten
> examples, for what a ChromeOS key event cannot reach:
>
> - modifier sets with `altL`, on non-native layers
> - form positions with output whose scan code is not among the codes of
>   `ldml.chromeos.keys`
> - long-press, multi-tap and flick gestures on keys of hardware layers
> - displays other than the automatic ones (`ldml.macos.classify`), and
>   space and return labels no display shows: the extension draws no
>   keyboard

## Target integration

> [spec:kbdgen:req:ldml.chromeos.target]
> `target chromeos` MUST build every layout of `ldml.chromeos.input` into
> one extension (`chromeos.extension`), v3 and v4 side by side. This
> replaces the `chromeos` clause of `ldml.yaml.coexistence`.

## Tests

> [spec:kbdgen:req:ldml.chromeos.test.typing]
> For the golden v4 layouts, the golden v3 layout with a `chromeOS`
> section, a v3 ChromeOS rendering of the Võro fixture, the macOS and
> Windows feature fixtures as `chromeOS` variants, and a fixture whose
> edits delete (text transforms, an astral character, normalization), the
> generated extension's `background.js`, `engine.wasm` and model MUST type
> what the engine types natively under the semantics above. Cases are
> every form position under no modifier, either Shift, Caps Lock, Caps
> Lock and Shift, AltGr reported as Alt and as AltGr, AltGr with Shift or
> Caps Lock, Left Alt, either Ctrl, Ctrl with AltGr, Search, and each bound
> extra-modifier key; then dead-key paths up to three keys, each ended
> also by Backspace, the decimal key, a frame key and Enter; and for the
> deleting fixture every pair of positions. The harness drives the real
> service worker in Deno through a fake `chrome.input.ime` whose field
> reports the text before the caret with and without the composition, on
> time and one key late. Handled keys, committed text and composition
> MUST agree. Without Deno the test says so and passes.

> [spec:kbdgen:req:ldml.chromeos.test.golden]
> For a bundle with a v3 `chromeOS` layout, two v4 layouts and a v3
> layout without `chromeOS`, `manifest.json`, every
> `_locales/*/messages.json` and the warnings of `ldml.chromeos.classify`
> MUST equal checked-in golden files byte for byte.
