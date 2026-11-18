# Model to XKB and on-screen keyboard layouts

The Linux target (`docs/spec/linux.md`) writes a static XKB layout, an
IBus engine's data and on-screen keyboard layouts for each layout. The
IBus engine runs the model whole, as the Windows text service does; the
XKB layout is the table fallback, as the Windows layout DLL is, and the
on-screen layouts are static too. This document says how the adapter
derives the XKB layout and the on-screen layouts from the model, and what
it reports they cannot express.

Sources: `docs/spec/ldml/kbdl.md`, `docs/spec/ldml/macos.md` and
`docs/spec/ldml/chromeos.md` as the templates for an adapter; libxkbcommon
`15b2b4e` (`src/state.c`, `NEWS.md`,
`include/xkbcommon/xkbcommon-keysyms.h`, `doc/rules-format.md`); xkeyboard-config `f4d3ed8`
(`types/basic`, `types/extra`, `symbols/kpdl`, `keycodes/evdev`); IBus
`7d228ea` (`src/ibuscomposetable.c` `ibus_keysym_to_unicode`,
`src/ibusenginesimple.c`); libX11 `nls/en_US.UTF-8/Compose.pre`
(`master`); Chromium `ui/events/keycodes/dom/dom_code_data.inc`;
gnome-shell `7fcbec8`, Stevia `810aba7` and plasma-keyboard `1e20db4` as
`docs/spec/linux.md` lists them.

## Input

> [spec:kbdgen:def:ldml.linux.input]
> The target's layouts are, in bundle layout order, every v4 layout with a
> `linux` host document (`ldml.yaml.hosts`: a `linux` or `default`
> hardware variant) and every v3 layout with a `windows` or `chromeOS`
> section. v3 has no Linux section. A v3 layout is migrated in memory
> (`ldml.migrate.*`), with nothing written, and its `linux` document is
> lowered as if the migration had written `hardware.linux: {inherits:
> windows}`, or `{inherits: chromeOS}` when it has no `windows` variant:
> Windows, like XKB, types a third level with Right Alt as AltGr, and
> nearly every v3 layout has a Windows section. Each `linux` document is
> compiled (`ldml.model.layout`) and its keyboard K encoded per
> `ldml.model.encoding` after the engine has loaded it. The on-screen
> layouts present K too, its hardware set as touch (`ldml.linux.touch`):
> the `android` keyboard and authored touch sets are not used, and the
> `linux` host has no touch chain (`docs/spec/linux.md`, Decisions). Fatal,
> naming the layout: a v3 migration that blocks
> (listing the blocking defect codes) or does not compile, and a `linux`
> keyboard without hardware layers. A v4 layout without a `linux`
> document has no Linux output; it is skipped with a debug message
> suggesting `hardware.linux: {inherits: windows}`.

> [spec:kbdgen:def:ldml.linux.adapter]
> The adapter's input is a compiled layout's K. Its
> outputs are the symbols file, the registry entry, the engine description
> and the on-screen layouts of `docs/spec/linux.md`. It MUST NOT panic. It
> reports per `ldml.linux.classify` and `ldml.linux.classify.touch`, or
> fails naming the layout and the position, the layer or the marker.

> [spec:kbdgen:def:ldml.linux.rewrites]
> A model *rewrites committed text* when it has a `simple` transform that
> does not start with a marker, a `reorder` group, a `backspace` rule other
> than the generated one (`ldml.yaml.dead-keys.backspace`), or an enabled
> `normalization`: the features `ldml.kbdl.classify` lists first. Only
> then can an edit's `delete` be non-zero (`ldml.engine.output.segment`),
> since a pending marker is never committed text. The IBus engine holds
> text back for such a model where it cannot delete
> (`linux.ibus.context.fallback`).

## XKB layout

> [spec:kbdgen:def:ldml.linux.keycodes]
> A PC/AT set 1 scan code *s* without `E0` (`ldml.model.hardware`) is the
> evdev key code *e*, which IBus passes (`linux.ibus.keys`), and the XKB
> key code *e* + 8, paired as Chromium's `dom_code_data.inc` pairs them:
> *e* = *s* for *s* from `01` to `58` except `45`, `54` and `55`; and
> `59`↔117, `70`↔93, `71`↔123, `72`↔122, `73`↔89, `79`↔92, `7B`↔94,
> `7D`↔124, `7E`↔121. The XKB layout writes these keys, in this order,
> named as xkeyboard-config's `keycodes/evdev` names them: `29` `<TLDE>`;
> `02`–`0D` `<AE01>`–`<AE12>`; `7D` `<AE13>`; `10`–`1B`
> `<AD01>`–`<AD12>`; `1E`–`28` `<AC01>`–`<AC11>`; `2B` `<BKSL>`; `56`
> `<LSGT>`; `2C`–`35` `<AB01>`–`<AB10>`; `73` `<AB11>`; `39` `<SPCE>`; and
> `<KPDL>` for the decimal (`ldml.linux.decimal`). These are the 49 ISO
> positions of `kbdl.scancodes.iso`, the JIS yen key and the space bar.
> A key is written when K's form has its scan code.

> [spec:kbdgen:sem:ldml.linux.levels+1]
> Each key has eight *states*, the combinations of Shift, Lock (Caps
> Lock) and Level3 (AltGr). State (*s*, *c*, *g*) takes the layer the
> engine selects (`ldml.engine.modifiers`, `ldml.engine.altgr`, `Other`;
> `ldml.engine.extra` with no extra key held) for `shift_l` = *s*, `caps` =
> *c*, and `alt_r` and `altgr` = *g*, as `ldml.kbdl.layers` takes the DLL's
> columns, and the key at the position in that layer; a state no layer
> serves has no key. The model *uses Level3* when a state with *g* has a
> layer; then Right Alt is `ISO_Level3_Shift` (`level3(ralt_switch)`),
> otherwise it stays Alt. So:
>
> - `altR`, `alt` and `ctrl alt` layers are typed with AltGr; Left Alt,
>   which stays Alt for shortcuts, types none of them
> - native-only layers (`ldml.model.native`) are not written: XKB derives
>   Ctrl and Super shortcuts from the base keysyms
> - layers that only `altL`, a Left Ctrl or an extra modifier select are
>   not written; a bound extra-modifier key keeps its usual keysym (Caps
>   Lock, `Control_R`), and a bound `B00`, which has no key
>   (`ldml.yaml.native`), types nothing
>
> A written key with no key in any state is written `[ VoidSymbol ]`, so
> that `pc`'s keysyms for it do not show through: the evdev rules include
> the layout over `pc` with the override merge, which keeps the earlier
> keysym of every level the layout leaves `NoSymbol` (`<LSGT>`, `<BKSL>`
> and `<SPCE>` in `pc`; checked with libxkbcommon 1.7.0 and 1.13.1).

> [spec:kbdgen:sem:ldml.linux.values+1]
> A state's *value* is a keysym:
>
> - no key, a gap or an empty output: `VoidSymbol`, which types nothing
>   and, unlike `NoSymbol`, replaces `pc`'s keysym (`ldml.linux.levels`)
> - one scalar value *c*: the keysym libxkbcommon's `xkb_utf32_to_keysym(c)`
>   gives, the legacy keysym where there is one, else the Unicode keysym
>   `0x01000000` + *c*, so that Compose sequences and shortcuts naming
>   legacy keysyms match. It is written by its name in
>   `xkbcommon-keysyms.h`, a Unicode keysym as `U` and at least four
>   uppercase hexadecimal digits
> - a single marker: its dead keysym (`ldml.linux.dead-keys`); for a dead
>   key without one, the keysym of `flush[m]` when that is one scalar
>   value, else `VoidSymbol`
> - more than one scalar value, or text and markers mixed: `VoidSymbol`.
>   libxkbcommon allows several keysyms per level, but toolkits take one
>   keysym per key (`xkb_state_key_get_one_sym`, as plasma-keyboard's
>   `src/inputmethod.cpp` does), and Compose output would need per-user
>   files (`linux.xkb.desktops`)

> [spec:kbdgen:sem:ldml.linux.semantics+1]
> The XKB layout is taken to type as libxkbcommon does (`src/state.c`): a
> key's type maps the active modifiers among its own to a level, Level1
> when no entry matches, and consumes its own modifiers except those the
> entry preserves; the level's keysym is taken, and `VoidSymbol` and
> `NoSymbol` type nothing. When Lock is active and the type does not consume it, the
> keysym becomes its uppercase, `xkb_keysym_to_upper`: the simple
> uppercase mapping of Unicode, with ẞ for ß since libxkbcommon 1.8
> (`NEWS.md`). Dead keysyms then compose per `ldml.linux.compose`. The
> adapter's choice of types, `ldml.linux.test.typing` and
> `linux.test.xkbcommon` all rest on these semantics.

> [spec:kbdgen:sem:ldml.linux.types+1]
> Each key gets an explicit type from xkeyboard-config's `complete` set
> (`types/basic`, `types/extra`): the first of `ONE_LEVEL`, `TWO_LEVEL`,
> `ALPHABETIC`, then `FOUR_LEVEL`, `FOUR_LEVEL_SEMIALPHABETIC` and
> `FOUR_LEVEL_ALPHABETIC` when the model uses Level3, then
> `FOUR_LEVEL_PLUS_LOCK`, that gives all eight of its values under
> `ldml.linux.semantics`; when the model does not use Level3, no state
> with Level3 can occur, and only the other four count. A type's levels hold the values of the states
> that name them in its map: Level1 (0, 0, 0), Level2 (1, 0, 0), Level3
> (0, 0, 1), Level4 (1, 0, 1), and in `FOUR_LEVEL_PLUS_LOCK`, which gives
> Lock alone a level, Level5 (0, 1, 0). A key that no type serves takes
> the type that gives the most states, the earliest on a tie, and the
> states it gets wrong are reported (`ldml.linux.classify`). A symbols
> file can name only the types its keymap compiles, which come from the
> rules' `types` component; types of the package's own would need rules of
> its own (`doc/rules-format.md`). As examples, a letter with the macOS
> caps rule (`ldml.yaml.implied-layers`), where Caps and Caps+Shift both
> give the uppercase, gets `TWO_LEVEL` or `FOUR_LEVEL`, whose unconsumed
> Lock uppercases; a key whose Caps layer differs from both gets
> `FOUR_LEVEL_PLUS_LOCK`; Windows' rule, Caps+Shift giving the lowercase,
> gets `ALPHABETIC` or `FOUR_LEVEL_SEMIALPHABETIC`.

> [spec:kbdgen:sem:ldml.linux.decimal]
> When K has a `decimal`, `<KPDL>` is written with the type `KEYPAD` and
> the keysyms `KP_Delete` and X, so Num Lock selects X: `KP_Decimal` for
> `.`, `KP_Separator` for `,`, as xkeyboard-config's `kpdl(comma)` writes
> it, else the value of a one-scalar decimal (`ldml.linux.values`). A
> longer decimal keeps `KP_Decimal` and is reported. Without a decimal
> `<KPDL>` is not written, and `pc`'s keypad keysyms stay.

## Dead keys

> [spec:kbdgen:def:ldml.linux.compose]
> The *reference Compose table* is the `<dead_…>` sequences of libX11's
> `nls/en_US.UTF-8/Compose.pre`, vendored in kbdgen with its licence;
> IBus's built-in table and other locales' tables build on it.
> Composition is taken to follow IBus's simple engine
> (`src/ibusenginesimple.c`), which GNOME runs for every XKB source
> (`linux.xkb.desktops`):
>
> - a dead keysym starts a sequence, and a sequence that matches an entry
>   types its output
> - a dead keysym followed by a keysym that continues no sequence types
>   the dead keysym's isolated character, after a space where
>   `ibus_keysym_to_unicode` asks for one, then the second keysym's
>   character ("dead keys are never *really* dead")
> - a longer sequence that fails types nothing
>
> So `<dead_acute> <space>` types `'` and `<dead_diaeresis> <space>`
> types `"`, where the engine types the standalone. GTK's, KWin's,
> plasma-keyboard's and other locales' composition differ in details;
> reports and tests use this one.

> [spec:kbdgen:sem:ldml.linux.dead-keys]
> A dead key, marker m, gets the first dead keysym D, in the order of
> `ibus_keysym_to_unicode`'s table (`src/ibuscomposetable.c`), whose
> isolated or combining character is `flush[m]`, else the display of
> `\m{m}` (`ldml.model.displays`), and that no marker earlier in the marker
> table got: `´` or U+0301 gives `dead_acute`, `ˇ` `dead_caron`, `~`
> `dead_tilde`, `¨` `dead_diaeresis`. A dead key without one is typed as
> its standalone (`ldml.linux.values`).
>
> Whether D composes as the engine does is found by running the engine
> (`ldml.engine.api`; `CancelOrPass`, `Nfc`, host `linux`), as
> `ldml.macos.dead-keys` does: from m pending, each distinct value of the
> written states, the space bar's included, is pressed by key id, and the
> result is compared with what the reference table gives for D followed by
> the value's keysym, through chains of dead keys up to three keys. The
> XKB layout keeps D, and results that differ are reported
> (`ldml.linux.classify`).

## On-screen layouts

> [spec:kbdgen:def:ldml.linux.touch]
> The on-screen layouts show K's desktop layout: its hardware set presented
> as touch (`ldml.model.touch`), one *presented set* for GNOME Shell,
> plasma-keyboard and Stevia alike. Its *default level* is the layer
> selected with no modifier and its *shift level* the layer selected with
> Shift (`ldml.engine.modifiers`), when Shift selects one; other layers are
> not shown. A level's rows are the form's rows without the space bar, a
> row of only the space bar being left out, and a position with no key in
> the level being a gap one key wide; a gap key or a key with no output is
> a gap of its width.
> Two *symbol levels* of three rows follow, the ones GNOME's and Stevia's
> layouts use:
>
> - first: `1 2 3 4 5 6 7 8 9 0`; `@ # € % & - + ( )`; `* " ' : ; ! ?`
> - second: `` ~ ` | • √ π ÷ × ¶ ∆ ``; `£ ¥ $ ¢ ^ ° = { }`;
>   `\ © ® ™ ℅ [ ]`
>
> As on a physical keyboard, every level gets a backspace key at the end
> of its first row, and its last row starts with a shift key on the
> default and shift levels when there is a shift level, or a switch
> between the symbol levels on those, each 1.5 keys wide. Each OSK adds
> its own bottom row with the space bar and return. The `android` keyboard
> and authored touch sets are not used (`docs/spec/linux.md`, Decisions).

> [spec:kbdgen:sem:ldml.linux.touch.keys]
> On every OSK a key is written from its output: plain text is what it
> commits; a single dead-key marker is a dead key, written per the OSK's
> rule; a key mixing text and markers, or of several markers, is left
> out. Its *label* is its display (`ldml.model.displays`, by key id first,
> then by output) when that differs from its output, and for a dead key
> its marker's display, else its standalone (`flush[m]`), else U+25CC.
> Its long-press candidates are the plain outputs of its `long_press`
> keys, in order; `longPress` gives hardware keys candidates too
> (`ldml.yaml.long-press`). Widths are the key's `width`; a gap's width
> spaces the next key. Flicks, multi-tap and `long_press_default` are not
> written. The space key is labelled with `labels.space` and the return
> key with `labels.return` where K has them and the OSK can show them.

> [spec:kbdgen:sem:ldml.linux.touch.gnome]
> `<L>.json` is the presented set in GNOME Shell's format
> (`data/README.osk-layouts`, `data/osk-layouts/*.json`): an object with
> `levels`, `locale` (the tag) and `name` (the display name of
> `linux.xkb.registry`). The levels are named `default`, `shift`, `opt`
> and `opt+shift`, the two symbol levels being the last; the shell starts
> at `default` and leaves `shift` after a key (`js/ui/keyboard.js`), and
> README's `""` for the first is stale. `mode` is `default`, `latched` for
> `shift`, else `locked`. A key is an object:
>
> - text: `strings`, its output then its candidates, and `label`
> - a dead key whose marker has a dead keysym D in the XKB layout
>   (`ldml.linux.dead-keys`) at Level1, 2 or 3 of the first key carrying D
>   in key-code order: `keyval` D in lowercase `0x` hexadecimal and
>   `label`, so that it reaches the IBus engine as a hardware key
>   (`linux.ibus.virtual-keys`; mutter presses no higher level); any other
>   dead key: `strings` with `flush[m]`, left out when that is empty
> - `width`: the key's width rounded to the nearest half key, at least
>   one half, as the shell places keys on a half-key grid (`KEY_SIZE`);
>   gaps add their width to the next key's `leftOffset`
> - the added keys: shift is `{"action": "levelSwitch", "iconName":
>   "osk-shift-symbolic", "level": "shift"}`, with `"level": "default"` on
>   the shift level; backspace `{"action": "delete", "iconName":
>   "osk-delete-symbolic"}`; the symbol-level switch a `levelSwitch` to
>   `opt+shift` labelled `=/<` on `opt`, and to `opt` labelled `?123` on
>   `opt+shift`
>
> Each level ends with a bottom row: a `levelSwitch` (1.5), to `opt`
> labelled `?123`, or on a symbol level to `default` labelled `ABC`;
> `{"action": "emoji", "iconName": "osk-emoji-picker-symbolic"}`; the
> space key, `strings` `[" "]`; `{"action": "languageMenu", "iconName":
> "osk-layout-symbolic"}`; `{"action": "hide", "iconName":
> "osk-hide-symbolic"}`; and return, `{"iconName": "osk-enter-symbolic",
> "keyval": "0xff0d"}` (1.5). The shell aligns rows left, so a row
> narrower than its level's widest gets half the difference, rounded down
> to a half key, as its first key's `leftOffset`, and the space key fills
> the bottom row to the widest row, at least 3 keys.

> [spec:kbdgen:sem:ldml.linux.touch.stevia]
> `<L>.json` is the presented set in Stevia's format
> (`src/pos-osk-widget.c` `parse_layout`, `src/layouts/*.json`): an object
> with `name` (the display name), `locale` (the tag) and `levels`, named
> `""`, `shift`, `opt` and `opt+shift`, the only ones Stevia knows. `mode`
> is written as for GNOME Shell; Stevia ignores it. A key is an array, its
> output then its candidates, or, with a label, a dead key or a width
> other than 1, an object with `symbol`, `symbols` (the candidates),
> `label` and `width`. Stevia has no dead keys: a dead key is `symbol`
> `flush[m]` with its display as `label`, left out when `flush[m]` is
> empty. Stevia adds shift and backspace to each level's second-to-last
> row, and its menu, symbol and return keys to the last
> (`add_common_keys_pre`, `add_common_keys_post`), so the added keys of
> `ldml.linux.touch` are left out; gaps are too, since Stevia centres
> rows. Each level ends with `[","]`, `[" "]` and `["."]`, as Stevia's
> layouts do; Stevia labels the space bar itself.

> [spec:kbdgen:sem:ldml.linux.touch.plasma]
> `contents/layouts/desktop.qml` is the presented set as a plasma-keyboard
> layout (`virtualkeyboard/components/`,
> `layouts/nb_NO/contents/layouts/Qwerty.qml`): imports of `QtQuick`,
> `org.kde.plasma.keyboard.virtualkeyboard`,
> `org.kde.plasma.keyboard.virtualkeyboard.components` and
> `org.kde.plasma.keyboard.layouts.defaultplugin as SharedLayouts`, then a
> `KeyboardLayoutDefinition` with `symbols: SharedLayouts.Symbols {}`,
> `numbers: SharedLayouts.Numbers {}`, `digits: SharedLayouts.Digits {}`,
> `dialpad: SharedLayouts.Dialpad {}`, and as its main layout:
>
> - without a shift level, the default level's `KeyboardLayout`
> - with one, a `KeyboardLayoutLoader` whose `sourceComponent` is the
>   shift level's `KeyboardLayout` while its own `inputEngine` is
>   uppercase (`inputEngine && inputEngine.uppercase`, as
>   `layouts/ko_KR/contents/layouts/Dubeolsik.qml` writes it) and the
>   default level's otherwise, each in a `Component`. The loader declares
>   `readonly property real maxWidthToHeightRatio: -1`: `Keyboard.qml`
>   caps the main layout's width at that ratio times its height unless it
>   is -1, and for a loader without the property the cap is NaN and no tap
>   reaches a key (Verified: `tests/linux-vm/plasma-keyboard/`).
>   plasma-keyboard has no shift layer: while shifted it uppercases a
>   key's text, when that is one UTF-16 unit, for the locale
>   (`textcomposers/directtextcomposer.cpp`), and its labels with them.
>   The shift level's keys carry `noModifier: true`, which keeps their
>   labels as written; what they commit is still uppercased, so a
>   shift-level text that the uppercasing changes is reported
>
> plasma-keyboard's own symbol pages stand in for the symbol levels. A
> level's rows are `KeyboardRow`s of:
>
> - text: `Key { text; alternativeKeys }`, the candidates as a list, with
>   `displayText` for a label and `weight` 100 × width when that is not 1;
>   strings are QML string literals
> - a dead key: `Key { text: flush[m]; displayText: display }`, left out
>   when `flush[m]` is empty
> - the added keys: `ShiftKey {}` and `BackspaceKey {}`, `weight` 150
> - a gap: `FillerKey {}`, with `weight` as for text
>
> Each level ends with nb_NO's bottom row: `SymbolModeKey` (1.5),
> `ChangeLanguageKey`, its `,` and `.` keys around `SpaceKey`, which has
> `displayText` `labels.space` where K has one, `HideKeyboardKey` and
> `EnterKey` (1.5). plasma-keyboard stretches each row to the keyboard's
> width and sizes its keys by weight (`KeyboardRow.qml`), so every row of
> both levels is made as wide as the widest of them, or as the bottom row
> with a `SpaceKey` of 4 if that is wider: a narrower row gets a
> `FillerKey` of half the difference at each end, and the `SpaceKey` takes
> what the bottom row lacks. `EnterKey` shows its icon, not
> `labels.return`.

## Reporting

> [spec:kbdgen:req:ldml.linux.classify]
> For the XKB layout, the adapter MUST report, once per layout and
> feature, with a count and up to ten examples, what it does not
> reproduce:
>
> - outputs of more than one scalar value, and outputs mixing text and
>   markers (`ldml.linux.values`)
> - dead keys without a dead keysym, and dead-key results the reference
>   Compose table gives otherwise, the space bar's and those of keys a dead
>   key does not compose with included (`ldml.linux.dead-keys`)
> - transforms that do not start with a marker, `reorder` groups,
>   `backspace` rules other than the generated one, and an enabled
>   `normalization` (`ldml.linux.rewrites`)
> - states no key type gives (`ldml.linux.types`)
> - extra-modifier layers, Shift Lock and LRM/RLM
> - outputs at scan codes `ldml.linux.keycodes` does not write, and a
>   decimal of more than one scalar value
> - long-press, multi-tap and flick gestures on keys of hardware layers
>
> These are information messages, each saying that the IBus engine types
> the feature, except gestures, which no hardware key makes. Layers only
> `altL` or a Left Ctrl selects are warnings: the engine passes Left Alt
> chords on Linux (`linux.ibus.modifiers`), so nothing there types them.
> That
> `alt` layers are typed with AltGr alone is reported once as
> information. A layout with no message at all gets one information
> message saying so.

> [spec:kbdgen:req:ldml.linux.classify.touch]
> For the on-screen layouts, the adapter MUST warn, once per layout, OSK
> and feature, with a count and up to ten examples, about what that OSK
> does not show or type:
>
> - layers other than the default and shift levels (`ldml.linux.touch`),
>   such as AltGr and Caps layers: on every OSK
> - flicks, multi-tap and `long_press_default`: on every OSK
> - dead keys typed as their standalone, or left out for want of one: on
>   Stevia and plasma-keyboard, and on GNOME Shell those without a dead
>   keysym
> - keys left out for mixing text and markers, and long-press candidates
>   left out for holding markers: on every OSK
> - shift-level texts, candidates included, that plasma-keyboard's
>   uppercasing changes
>
> It MUST inform, once per layout, of each of these that applies to it:
> that GNOME Shell sends a text of several scalar values to the IBus
> engine one scalar at a time, where the levels show one; that on GNOME
> Shell a dead key pressed on screen composes only with the IBus source
> (`linux.osk.gnome.selection`), where they show a dead key; that GNOME
> Shell's on-screen Backspace skips `backspace` transforms while nothing
> is shown as preedit (`linux.ibus.osk-backspace`), where K has
> `backspace` rules other than the generated one; and, always, that
> plasma-keyboard's and Stevia's layouts type without the engine.

## Target integration

> [spec:kbdgen:req:ldml.linux.target]
> `target linux` MUST build every layout of `ldml.linux.input` into one
> output (`linux.output`), v3 and v4 side by side. This replaces the
> `linux` clause of `ldml.yaml.coexistence`.

## Tests

> [spec:kbdgen:req:ldml.linux.test.typing]
> For the golden v4 layouts, the golden v3 layouts migrated in memory, and
> the Windows and macOS feature fixtures as `linux` variants, the XKB
> layout, read back from the written symbols file and simulated per
> `ldml.linux.semantics`, MUST type what the engine types (host `linux`,
> keys sent as `linux.ibus.keys` sends them) at every written key in all
> eight states, then on dead-key paths of up to three keys, each also
> ended by the space bar. Cases the engine passes are not compared.
> Committed text must agree, unless a reported feature
> (`ldml.linux.classify`) explains the difference; each explanation is
> exercised by a synthetic layout.

## Decisions

The operator settled these on review:

- **The v3 source** is the Windows variant, else the ChromeOS one
  (`ldml.linux.input`). ChromeOS runs on XKB too, but fewer layouts have
  it and it lacks `alt+caps`.
- **Lock's uppercasing** is relied on: `TWO_LEVEL` and `FOUR_LEVEL` keys
  use libxkbcommon's case mapping, which follows its Unicode version and
  gained ß → ẞ in 1.8. The type choice falls back to explicit levels where
  the mapping would differ (`ldml.linux.types`), and
  `linux.test.xkbcommon` runs against the installed libxkbcommon.
- **macOS-style `alt` layers** are typed with AltGr only on Linux
  (`ldml.linux.levels`); an author who wants a Linux AltGr layer writes
  `altR` in `hardware.linux`.
- **The on-screen layouts** present the desktop layout, with the fixed
  symbol levels GNOME's and Stevia's own layouts share
  (`ldml.linux.touch`).

Later:

- authored Linux touch layouts (`docs/spec/linux.md`, Decisions)
- `extra1` as Level5, with `ISO_Level5_Shift` on the bound key; until
  then the IBus engine types extra-modifier layers
