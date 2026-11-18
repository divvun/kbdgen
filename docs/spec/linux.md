# Linux

`target linux` writes, for every layout with a `linux` keyboard
(`ldml.linux.input`), two tiers and three on-screen keyboards:

- an XKB layout with its registry entry, which every Wayland desktop
  compiles into its keymap (`linux.xkb.*`)
- the data of an IBus engine that runs `kbd-engine` on the layout's model
  (`linux.ibus.*`); the engine program itself is shared by all keyboards
  and released separately, as the Windows text service is
- on-screen keyboard (OSK) layouts showing the layout's desktop layout,
  for GNOME Shell, in a generated extension; for plasma-keyboard, as
  KPackages; and for Stevia, as files for upstream (`linux.osk.*`)

The two tiers match the other targets: Windows has a layout DLL and a TSF
text service, macOS a static `.keylayout` with what it cannot express
reported, ChromeOS the engine in WebAssembly. The XKB layout types wherever
the engine does not run: on KDE Plasma while plasma-keyboard is the input
method, on Phosh, on desktops without IBus, and when the engine fails. The
engine's base keymap is the same layout (`linux.ibus.data`), so keys the
engine passes, or types while it is down, follow it. What the XKB layout
cannot express is reported per layout (`ldml.linux.classify`). Keyboard
semantics are those of `ldml.engine.*`; these rules cover only what is
specific to Linux. How the model becomes keysyms, key types and on-screen
keys is `docs/spec/ldml/linux.md`.

X11 sessions are out of scope: GNOME 50 and Plasma 6.8 run on Wayland only,
and an X server reads XKB data only from its compiled-in root
(doc/custom-configuration.md). XWayland clients get the compositor's
keymap, so they type the XKB layout. A Fcitx5 addon, which Fcitx5 needs
because it cannot host IBus engines, and a Stevia completer hosting
`kbd-engine` are later work; the engine's host logic is kept free of D-Bus
for them (`linux.ibus.planner`).

**What the marks mean.** "(Source)" marks behaviour read in the upstream
source at the revisions below; nothing in this file has yet been run on a
Linux desktop. "Unverified" marks inferences, each to be settled by
`linux.osk.gnome.spike` or `linux.test.acceptance`. Rules naming the
installer bind the keyboard packaging (divvun-actions), not kbdgen.

Sources:

- libxkbcommon `15b2b4e` (1.14.0-beta2): `doc/packaging-keyboard-layouts.md`,
  `doc/custom-configuration.md`, `doc/rules-format.md`, `NEWS.md`,
  `src/xkbcomp/include.c` (`ProcessIncludeFile`), `src/registry.c`
  (`validate`, `rxkb_context_include_path_append_default`), `src/state.c`
  (caps transformation)
- xkeyboard-config `f4d3ed8` (2.49): `rules/xkb.dtd`, `rules/base.xml`,
  `types/basic`, `types/extra`, `symbols/{pc,level3,kpdl}`, `keycodes/evdev`
- gnome-shell `7fcbec8` (51.1): `data/README.osk-layouts`,
  `data/osk-layouts/*.json`, `data/gnome-shell-osk-layouts.gresource.xml`,
  `js/ui/main.js` (`_loadOskLayouts`), `js/ui/keyboard.js` (`KeyboardModel`,
  `_updateLayout`, `_addRowKeys`, `KeyboardController.commit`,
  `toggleDelete`), `js/misc/inputMethod.js`, `js/misc/ibusManager.js`,
  `js/misc/keyboardManager.js`, `js/ui/status/keyboard.js`,
  `js/misc/extensionUtils.js`, `js/ui/extensionSystem.js`
- IBus `7d228ea` (1.5.35): `src/ibusengine.c` (introspection, surrounding
  text), `src/ibusfactory.c`, `src/ibuscomponent.c`, `src/ibusobservedpath.c`,
  `src/ibusregistry.c`, `src/ibusenginedesc.c`, `src/ibustypes.h`,
  `src/ibustext.c`, `src/ibusattrlist.c`, `src/ibusattribute.c`,
  `src/ibusshare.c`, `src/ibusenginesimple.c`, `src/ibuscomposetable.c`,
  `bus/ibusimpl.c`, `bus/engineproxy.c`, `bus/component.c`,
  `client/gtk2/ibusimcontext.c`
- ibus-keyman (keymanapp/keyman `master`,
  `linux/ibus-keyman/src/keyman.xml.in`, `engine.cpp`) and ibus-m17n
  (`src/m17n.xml.in`)
- Stevia `810aba7` (0.59.0): `src/pos-osk-widget.c`,
  `src/pos-input-surface.c`, `src/phosh-osk-stevia.gresources.xml`,
  `src/layouts/*.json`, `doc/testing-layouts.md`
- plasma-keyboard `1e20db4` (6.8.80, the 6.9 development line):
  `README.md`, `layouts/*/metadata.json`, `layouts/nb_NO/`,
  `layouts/defaultplugin/`, `virtualkeyboard/components/`,
  `virtualkeyboard/keyboardlayoutmetadata.cpp`,
  `virtualkeyboard/textcomposers/directtextcomposer.cpp`,
  `kcm/plasmakeyboardkcm.cpp`, `src/plasmakeyboardsettings.kcfg`; tag
  `v6.7.90` `src/layoutpathhelper.h`
- crates.io `ibus-engine` 0.3.1, `gvdb` 0.10.1, `xkbcommon` 0.9.0
- `docs/spec/tsf.md`, `docs/spec/chromeos.md`, `docs/spec/ldml/chromeos.md`
  and `docs/spec/ldml/linux.md`

## Identifiers and output

> [spec:kbdgen:def:linux.ids]
> `targets/linux.yaml` (`bundle.structure.targets`) names the bundle with
> `packageId`, a reverse-DNS name of lowercase ASCII letters, digits, `-`
> and `.`, and `layoutPrefix`, a lowercase ASCII letter followed by at most
> 15 lowercase letters and digits; another shape is an error naming the
> file and the field. A layout's *layout id* L is `layoutPrefix`, `-`, and
> its normalised tag in lowercase (`divvun` and `se-FI` give
> `divvun-se-fi`). L names the XKB layout, its symbols file and registry
> entry, the IBus engine and that engine's base keymap, and the GNOME
> Shell and Stevia layout files, so each desktop matches the pieces by
> one name (`linux.xkb.naming`). Further:
>
> - the GNOME Shell extension's UUID is `osk@<packageId>`
> - a layout's KPackage id is `<packageId>.<tag_>`, where `tag_` is the
>   tag with `-` replaced by `_`
> - the XKB package directory and the IBus data directory of the bundle
>   are named `packageId` (`linux.install`)

> [spec:kbdgen:def:linux.output]
> `target linux` runs one step, which writes these directories in this
> order, once every layout has compiled and passed its checks, so that a
> failure writes nothing:
>
> - `xkb/`: `symbols/<L>` per layout (`linux.xkb.symbols`) and
>   `rules/evdev.xml` (`linux.xkb.registry`)
> - `ibus/`: `<L>.xml` and `<L>.dvkb` per layout (`linux.ibus.data`)
> - `gnome-shell/<uuid>/` (`linux.osk.gnome`)
> - `plasma-keyboard/<id>/` per layout (`linux.osk.plasma`)
> - `stevia/layouts/<L>.json` per layout (`linux.osk.stevia`)
>
> Each is emptied first, so nothing of an earlier run remains, and each
> is laid out as it is installed (`linux.install`). The step MUST fail,
> before writing, with `targets/linux.yaml is missing` if that file is
> absent, then with an error of `linux.ids`, then with `project.yaml:
> locales has no en entry`, then with any error of `ldml.linux.input`,
> then with `no layout has a linux keyboard` when there are none. There
> are no subcommands (`cli.commands`): nothing needs a tool beyond kbdgen.

> [spec:kbdgen:req:linux.install]
> kbdgen installs nothing. A keyboard package or installer MUST install
> one bundle's output in one scope:
>
> | Output | System-wide | One user |
> |---|---|---|
> | `xkb/` | `/usr/share/xkeyboard-config.d/<packageId>/`, or merged into `/etc/xkb/` (`linux.xkb.install`) | merged into `$XDG_CONFIG_HOME/xkb/` |
> | `ibus/` | `<datadir>/divvun/keyboards/<packageId>/` | `~/.local/share/divvun/keyboards/<packageId>/` |
> | `gnome-shell/<uuid>/` | `<datadir>/gnome-shell/extensions/<uuid>/` | `$XDG_DATA_HOME/gnome-shell/extensions/<uuid>/` |
> | `plasma-keyboard/<id>/` | `<datadir>/plasma/keyboard/keyboardpackages/<id>/` | `$XDG_DATA_HOME/plasma/keyboard/keyboardpackages/<id>/` |
>
> `stevia/` is not installed (`linux.osk.stevia`). The installer enables
> the GNOME extension for the user (`gnome-extensions enable <uuid>`); a
> distribution may enable it by a dconf default of `org.gnome.shell
> enabled-extensions`. The IBus engine is a system package of its own,
> which keyboard packages depend on (`linux.ibus.component`); a per-user
> install types through it only where it is installed. Uninstalling
> removes exactly what installing added.

## XKB layout

> [spec:kbdgen:sem:linux.xkb.naming]
> Each layout is an XKB layout of its own, named L (`linux.ids`), not a
> variant of an existing country layout. A variant such as `no(…)` would
> let GNOME Shell's on-screen keyboard fall back to the country's layout
> where the layout's own is missing (`linux.osk.gnome.selection`), but:
>
> - the package would ship `symbols/no`. libxkbcommon before 1.9.0 took
>   the first file of a name on its include path, and that file's first
>   section as its default, so a `symbols/no` in `/etc/xkb` or
>   `~/.config/xkb` broke Norwegian; since 1.9.0 it searches on for an
>   exact or explicit default section (Source: `ProcessIncludeFile`;
>   doc/custom-configuration.md, "Using system file names")
> - two packages, or a user's own customisation, adding variants to one
>   country collide on one file in `/etc/xkb` and `~/.config/xkb`, where
>   nothing composes them
> - every layout would need a country, which tags such as `sma` and `vro`
>   do not give
>
> Bare tags are no better: `se` is Northern Sami and Sweden's
> `symbols/se`, and `fi`, `et` and `lv` are country files too. No
> xkeyboard-config symbols file has a `-` in its name, and L always does,
> so L never shadows one. Stevia has no fallback, so it needs the
> layout's own file either way (`linux.osk.stevia`).

> [spec:kbdgen:syn:linux.xkb.symbols]
> `xkb/symbols/<L>` is UTF-8 text in libxkbcommon's keymap text format v1
> holding one section: a comment line `// <L>: generated by kbdgen from
> layouts/<file>. Do not edit.`, then `default partial alphanumeric_keys`,
> then `xkb_symbols "basic" {`, then `name[Group1] = "<description>";`,
> then one line `key <NAME> { type[Group1] = "<type>", symbols[Group1] = [
> <keysym>, … ] };` per key of `ldml.linux.levels` in the order of
> `ldml.linux.keycodes`, with as many keysyms as the type has levels
> (`ldml.linux.types`, `ldml.linux.values`, `ldml.linux.decimal`), then
> `include "level3(ralt_switch)"` when the model uses Level3, then `};`.
> Lines inside the section are indented by four spaces; the file ends
> with a newline. The description is the registry entry's
> (`linux.xkb.registry`) with `\` and `"` escaped by `\`. The section is
> the default, so the layout name alone selects it: xkeyboard-config's
> evdev rules map a layout L to `symbols/L` without a rules file of the
> package's own, and add `pc` and `inet(evdev)`, which keep every key the
> file does not define (Source: doc/custom-configuration.md, "Adding a
> layout").

> [spec:kbdgen:syn:linux.xkb.registry]
> `xkb/rules/evdev.xml` is an XKB registry: an `<?xml version="1.0"
> encoding="UTF-8"?>` declaration, no doctype, and a root
> `<xkbConfigRegistry version="1.1">` holding only a `<layoutList>` with
> one `<layout><configItem>` per layout, in bundle layout order, whose
> children are, in order:
>
> - `name`: L
> - `shortDescription`: the tag's primary language subtag, which GNOME
>   shows as the indicator
> - `description`: the layout's display name under its primary language
>   subtag (`bundle.layouts.autonym`)
> - `countryList` with one `iso3166Id`, when the tag has a region subtag
>   of two letters
> - `languageList` with one `iso639Id`, the ISO 639-3 code of the primary
>   language subtag (`sme` for `se`), omitted when it has none
>
> Elements are indented by two spaces, text escapes `&`, `<` and `>`, and
> the file ends with a newline. libxkbregistry validates against a DTD of
> its own that, unlike xkeyboard-config's `rules/xkb.dtd`, needs no
> `modelList` or `optionList` (Source: `src/registry.c` `validate`). The
> entry is mandatory: GNOME Shell drops an input source that
> libxkbregistry does not list (`js/misc/keyboardManager.js`
> `setUserLayouts`), Stevia does too (`src/pos-input-surface.c`), and the
> GNOME and KDE settings list layouts from it.

> [spec:kbdgen:req:linux.xkb.install]
> The `xkb/` tree MUST be installed in one of three places (`linux.install`):
>
> 1. **Extension directory**, for system packages where libxkbcommon ≥
>    1.13 and xkeyboard-config ≥ 2.45: the tree as
>    `<ext>/<packageId>/`, where `<ext>` is pkg-config `xkbcommon`'s
>    `xkb_unversioned_extensions_path`, `/usr/share/xkeyboard-config.d` by
>    default. libxkbcommon appends each subdirectory to its default include
>    path after `/etc/xkb` and before the xkeyboard-config root, and
>    libxkbregistry reads each one's `rules/evdev.xml`, so each package owns
>    its directory and nothing is merged (Source:
>    doc/packaging-keyboard-layouts.md, `src/registry.c`).
> 2. **System configuration**, for libxkbcommon from 1.0 to 1.12: each
>    `symbols/<L>` into `$XKB_CONFIG_EXTRA_PATH`, `/etc/xkb` by default,
>    and the `<layout>` entries merged into its `rules/evdev.xml`.
> 3. **User configuration**, without root (libxkbcommon ≥ 0.10): likewise
>    into `$XDG_CONFIG_HOME/xkb`, `~/.config/xkb` by default.
>
> Merging MUST add or replace the `<layout>` elements whose `name` is one
> of the bundle's layout ids and keep every other element, creating the
> file when it is missing; uninstalling removes them, and removes the file
> when it then holds no `layout`, `model` or `option`. Desktops read new
> layouts at their next session.

> [spec:kbdgen:sem:linux.xkb.desktops]
> On Wayland the compositor compiles the keymap with libxkbcommon's
> default include path and sends it to every client, XWayland included:
>
> - GNOME: the input-source list and mutter's keymap come from
>   libxkbregistry and the include path. With the XKB source L active,
>   GNOME Shell asks IBus for the engine `xkb:L::<language>`, which does
>   not exist, so IBus runs `xkb:us::eng`, its simple engine, while L stays
>   the keymap (Source: `js/ui/status/keyboard.js` `activateInputSource`,
>   `js/misc/ibusManager.js`). That engine composes dead keys from
>   `$XDG_CONFIG_HOME/ibus/Compose`, the GTK Compose files or
>   `~/.XCompose`, else its built-in en_US table; there is no system-wide
>   Compose location a package could install to (Source:
>   `src/ibusenginesimple.c`). With the IBus engine's source active, GNOME
>   applies the engine's `layout`, L, as the keymap.
> - KDE Plasma: the keyboard settings list libxkbregistry's layouts, and
>   KWin compiles with the default include path. The input method,
>   plasma-keyboard or IBus, composes.
> - Phosh: the keymap follows `org.gnome.desktop.input-sources` as on
>   GNOME; there is no IBus.
>
> (Unverified on the desktops themselves: `linux.test.acceptance`.)

## IBus engine

> [spec:kbdgen:def:linux.ibus.component]
> The *IBus engine* is one program, `ibus-engine-divvun`, built from the
> workspace crate `crates/kbd-ibus` (`linux.ibus.crate`). Every keyboard
> kbdgen builds shares it, as the text service is shared (`tsf.component`): it
> is released, versioned by the `kbd-ibus` package version, and installed
> separately from keyboards, which supply only data (`linux.ibus.data`).
> IBus reads components only from `<datadir>/ibus/component`, or the
> directories of `IBUS_COMPONENT_PATH` instead; there is no per-user
> component directory (Source: `src/ibusregistry.c`). The engine is
> therefore installed system-wide as `<libexecdir>/ibus-engine-divvun`
> with the component file `<datadir>/ibus/component/divvun.xml`. That file
> is `crates/kbd-ibus/data/divvun.xml.in` with `@LIBEXECDIR@`,
> `@DATADIR@` and `@VERSION@` substituted by packaging: a `<component>`
> with `name` `org.freedesktop.IBus.Divvun`, `description`, `exec`
> `@LIBEXECDIR@/ibus-engine-divvun --ibus --data-dir
> @DATADIR@/divvun/keyboards`, `version`, `author`, `license` `MIT OR
> Apache-2.0`, `homepage`, `observed-paths` with the paths
> `@DATADIR@/divvun/keyboards/` and `~/.local/share/divvun/keyboards/`,
> and `<engines exec="@LIBEXECDIR@/ibus-engine-divvun --xml --data-dir
> @DATADIR@/divvun/keyboards"/>`. The engine list is dynamic, as
> ibus-keyman's and ibus-m17n's are: ibus-daemon runs the `engines` command
> (`linux.ibus.lister`) and runs it again when an observed directory
> changes, so keyboards installed for one user appear without root
> (Source: `src/ibuscomponent.c`, `src/ibusobservedpath.c`). IBus expands
> only a leading `~` in observed paths, so the per-user directory does not
> follow `XDG_DATA_HOME`.

> [spec:kbdgen:req:linux.ibus.crate]
> `crates/kbd-ibus` MUST be an edition 2024 binary crate whose binary is
> `ibus-engine-divvun`. It depends on `kbd-engine` with its default
> `normalization` feature, `kbd-model`, and `zbus` 5, and implements the
> D-Bus protocol of `linux.ibus.factory` itself, with no GLib, GObject or
> libibus. It is excluded from `default-members`; releases are built for
> `x86_64-unknown-linux-gnu` and `aarch64-unknown-linux-gnu`. It keeps
> `kbd-engine`'s lints against panics, `unwrap`, `expect`, indexing and
> `unreachable!`. It MUST NOT depend on the `ibus-engine` crate: 0.3.1,
> first published on 2026-11-09, ignores `SetSurroundingText` and
> `SetCapabilities`, emits neither `DeleteSurroundingText` nor
> `RequireSurroundingText`, and is licensed LGPL-2.1-or-later or
> GPL-3.0-or-later, which a statically linked MIT or Apache-2.0 program
> would inherit.

> [spec:kbdgen:syn:linux.ibus.data]
> For each layout, `ibus/<L>.dvkb` is the `ldml.model.encoding` of its
> `linux` keyboard (`ldml.linux.input`), and `ibus/<L>.xml` its engine
> description: an `<?xml version="1.0" encoding="UTF-8"?>` declaration and
> an `<engine>` (Source: `src/ibusenginedesc.c`) whose children are, in
> order, indented by two spaces:
>
> - `name`: L
> - `longname` and `description`: the display name of `linux.xkb.registry`;
>   GNOME Shell lists the engine as `<language name> (<longname>)`
> - `language`: the tag's language subtag, joined by `_` to its region
>   subtag when it has one (`se_FI`), which IBus names through iso-codes
> - `author`: `project.yaml` `author`
> - `layout`: L, the layout's own XKB layout. GNOME Shell applies it as
>   the keymap while the engine is active and picks the on-screen layout by
>   it (`linux.osk.gnome.selection`), so it needs the registry entry
>   (`linux.xkb.registry`); keys the engine passes type it, and so does
>   everything while the engine is down
> - `symbol`: the registry entry's `shortDescription`
> - `version`: `targets/linux.yaml` `version`
>
> There is no `icon`, `setup` or `rank`: IBus shows the symbol, and the
> bundle has no Linux icons yet (Decisions, Later). Text escapes `&`, `<` and
> `>`; the file ends with a newline.

> [spec:kbdgen:req:linux.ibus.lister]
> `ibus-engine-divvun --xml [--data-dir DIR]…` MUST print one `<engines>`
> element holding the `<engine>` of each usable description in
> `~/.local/share/divvun/keyboards/*/`, then in each `DIR/*/` in argument
> order, directories and files in byte order of their names. A
> description is usable when it parses, has every element of
> `linux.ibus.data`, its `name` equals its file stem, and the model beside
> it decodes with `Model::from_bytes`. Of two descriptions with one name
> the first wins, so a user's copy overrides the system's. Unusable ones
> are left out silently: ibus-daemon logs whatever the command writes to
> standard error as its failure (Source: `src/ibuscomponent.c`).
> `--ibus` serves exactly the engines `--xml` lists.

> [spec:kbdgen:req:linux.ibus.factory]
> Run with `--ibus`, the program MUST connect to ibus-daemon's bus at the
> address libibus's `ibus_get_address` finds (`IBUS_ADDRESS`, else the
> address file that `src/ibusshare.c` names), export
> `org.freedesktop.IBus.Factory` at `/org/freedesktop/IBus/Factory`, and
> request the bus name `org.freedesktop.IBus.Divvun`; ibus-daemon treats
> the component as started once that name has an owner, and calls the
> factory on it (Source: `bus/ibusimpl.c`, `bus/component.c`).
> `CreateEngine(s name) → o` loads the engine's model, exports an engine at
> `/org/freedesktop/IBus/Engine/<n>`, *n* counting from 1, and returns its
> path; a name it cannot serve fails (`linux.ibus.inert`). An engine
> object implements `org.freedesktop.IBus.Engine` as `src/ibusengine.c`
> declares it:
>
> - methods `ProcessKeyEvent(u keyval, u keycode, u state) → b`,
>   `FocusIn`, `FocusOut`, `FocusInId(ss)`, `FocusOutId(s)`, `Reset`,
>   `Enable`, `Disable`, `SetCapabilities(u)`, `SetSurroundingText(v text,
>   u cursor_pos, u anchor_pos)` and `SetCursorLocation(iiii)`; the others
>   succeed and do nothing
> - signals `CommitText(v)`, `UpdatePreeditText(v text, u cursor_pos, b
>   visible, u mode)`, `DeleteSurroundingText(i offset, u nchars)` and
>   `RequireSurroundingText`. The last two are missing from libibus's
>   introspection data, but libibus emits them and ibus-daemon handles them
>   by name (Source: `src/ibusengine.c`, `bus/engineproxy.c`)
> - properties `ContentType` `(uu)`, written by ibus-daemon; `FocusId`
>   `(b)`, false; and `ActiveSurroundingText` `(b)`, true, so ibus-daemon
>   asks clients for surrounding text on focus
>
> and `org.freedesktop.IBus.Service.Destroy`, which unexports it. The
> program exits when its bus connection closes.

> [spec:kbdgen:req:linux.ibus.inert]
> `CreateEngine` MUST fail with `org.freedesktop.DBus.Error.Failed`, the
> message being the reason, and log the reason with the file's name, when
> it finds: `no-description` (no usable description has the name),
> `model-unreadable`, `model-version` (`Model::from_bytes` refused the
> major version, `tsf.data.version`), `model-malformed` or `model-invalid`.
> ibus-daemon then leaves the input context without an engine, and the
> XKB layout types. A keyboard package MUST depend on an engine release
> whose `kbd-model` reads the major version kbdgen wrote.

> [spec:kbdgen:req:linux.ibus.planner]
> The host logic — classifying key events (`linux.ibus.keys`,
> `linux.ibus.modifiers`, `linux.ibus.virtual-keys`), choosing the context
> (`linux.ibus.context`, `linux.ibus.context.fallback`,
> `linux.ibus.security`) and turning an engine `Action` into the signals of
> `linux.ibus.edit` — MUST be a module with no D-Bus calls, as the text
> service's planner is (`tsf.test.host`). Later hosts reuse it: a Fcitx5
> addon and a Stevia completer (Decisions, Later).

### Keys

> [spec:kbdgen:req:linux.ibus.keys]
> IBus passes evdev key codes, the XKB key code minus 8 (Source:
> `js/misc/inputMethod.js` sends `event.get_key_code() - 8`, the GTK and
> Wayland clients `keycode - 8`). A press, a state without
> `IBUS_RELEASE_MASK` (1 << 30), with key code *e* > 0 MUST be classified
> by *e*, never by its keyval, which depends on the keymap
> (`tsf.keys.identity`):
>
> - a modifier, by keyval (the Shift, Control, Alt, Meta, Super, Hyper,
>   `ISO_Level3_Shift`, `ISO_Level5_Shift`, `Mode_switch`, `ISO_*Group*`,
>   `Caps_Lock`, `Shift_Lock` and `Num_Lock` keysyms) or by code (29, 42,
>   54, 56, 58, 69, 97, 100, 125, 126): passes and resets nothing
> - 14 is `Backspace`; 83 is `Decimal`, unless its keyval is `KP_Delete`
>   (Num Lock off), when it is a frame key
> - a code of `ldml.linux.keycodes` is `Scan` of its scan code, which the
>   engine passes unless a form names it; a `B00` (86) bound as an extra
>   modifier is sent too, and the engine consumes it (`ldml.engine.extra`)
> - any other code, such as the arrows, `Delete`, keypad Enter or the
>   function keys past F12, is a frame key
>
> A press with key code 0 is a virtual key (`linux.ibus.virtual-keys`). A
> release returns what its press returned, and false for a press not seen.
> An event with `IBUS_FORWARD_MASK` (1 << 25) passes.

> [spec:kbdgen:sem:linux.ibus.modifiers]
> The state's bits (Source: `src/ibustypes.h`) say which modifiers are
> down, and the presses and releases seen of the modifier keys say which
> side, Left when none was seen (`ldml.chromeos.modifiers`):
>
> - `IBUS_SHIFT_MASK` (1 << 0) gives `shift_l` or `shift_r` (42, 54);
>   `IBUS_CONTROL_MASK` (1 << 2) gives `ctrl_l` or `ctrl_r` (29, 97)
> - AltGr is `alt_r` with `altgr`: when `IBUS_MOD5_MASK` (1 << 7) is set,
>   which `ISO_Level3_Shift` sets, or when Right Alt (100) is held and the
>   model uses Level3 (`ldml.linux.levels`) whatever the keymap beneath, as
>   `tsf.keys.altgr` decides it
> - `IBUS_MOD1_MASK` (1 << 3) with Left Alt (56) held, or with no Right Alt
>   held, is `alt_l`. The XKB layout leaves Left Alt as Alt, for menus and
>   shortcuts, and the engine passes a key held with it for host `linux`,
>   as for Windows and ChromeOS (`ldml.engine.shortcuts`, changed under
>   Changes to shared rules), so the host sends it and lets the engine
>   decide, as the text service does (`tsf.engine.contract`)
> - `cmd` while `IBUS_MOD4_MASK` (1 << 6) or `IBUS_SUPER_MASK` (1 << 26) is
>   set
> - `caps` is `IBUS_LOCK_MASK` (1 << 1), or with `windows.shift_lock` the
>   host's own Shift Lock, which Caps Lock sets and Shift clears
> - `extra[n-1]` while the bound Caps Lock (58) or `B00` (86) is held; a
>   bound Right Ctrl stays `ctrl_r`, which the engine rebinds
>   (`ldml.engine.extra`)
>
> The keys seen held are forgotten on `FocusIn`, `FocusOut`, `Enable` and
> `Disable`, since their releases may go elsewhere.

> [spec:kbdgen:sem:linux.ibus.virtual-keys]
> While an IBus source is active, GNOME Shell's on-screen keyboard sends
> each scalar value of a key's string as `ProcessKeyEvent(keyval, 0, 0)`,
> the keyval being `Clutter.unicode_to_keysym` of the scalar, and presses
> the keyval on its virtual keyboard device itself when the engine returns
> false (Source: `js/ui/keyboard.js` `KeyboardController.commit`,
> `js/misc/inputMethod.js` `handleVirtualKey`). A press with key code 0
> and keyval *k* is:
>
> - `Backspace` for `BackSpace`
> - `Emit(c)` (`ldml.engine.event`) with no modifiers when *k* maps to a
>   scalar value *c* that is not a control character, as
>   `xkb_keysym_to_utf32` maps it (legacy keysyms by table, the Unicode
>   keysym `0x01000000` + *c* to *c*): the engine handles *c* as a key's
>   output, so transforms and pending dead keys apply to on-screen keys
> - otherwise a frame key, which passes, so GNOME Shell presses it on its
>   virtual device and it comes back as a hardware key
>
> A string of several scalar values therefore reaches the engine one at a
> time (`ldml.linux.classify.touch`). On-screen dead keys are keyvals
> instead (`ldml.linux.touch.gnome`), which GNOME Shell presses on its
> virtual device, so they reach the engine as hardware keys. (Unverified:
> that mutter's virtual device finds the key code and level modifiers of a
> dead keysym in the keymap, `linux.osk.gnome.spike`.)

### Edits and context

> [spec:kbdgen:sem:linux.ibus.semantics]
> A frame key, or a key the engine passes, Left Alt chords among them
> (`linux.ibus.modifiers`), *passes*: if a preedit is shown or markers are
> pending, the edit of a `Commit` is applied first, with any held text
> (`linux.ibus.context.fallback`); then the state resets and
> `ProcessKeyEvent` returns false, so the client types the key. An engine
> edit is applied (`linux.ibus.edit`) and it returns true. `FocusIn`,
> `FocusOut`, `Enable`, `Disable` and `Reset` reset the state and forget
> the context and the cache (`ldml.engine.state`). A shown preedit is left
> to the client, which commits it (`IBUS_ENGINE_PREEDIT_COMMIT`): the text
> `Commit` would have inserted (`ldml.engine.preedit`). Sessions use the
> engine's `Nfc`, `CancelOrPass` and host `linux`.

> [spec:kbdgen:req:linux.ibus.edit]
> An engine edit (*d*, *s*, *p*) (`tsf.edit.ops`) MUST be applied by
> emitting, in this order and before replying to `ProcessKeyEvent`, so
> that ibus-daemon handles them first:
>
> 1. when a preedit is shown and *d* > 0 or *s* is non-empty,
>    `UpdatePreeditText` with the empty text, cursor 0, not visible, mode
>    `IBUS_ENGINE_PREEDIT_CLEAR` (0)
> 2. when *d* > 0, `DeleteSurroundingText(−d, d)`; IBus counts scalar
>    values, as the engine does
> 3. when *s* is non-empty, `CommitText` of *s*
> 4. when the preedit to show differs from the one shown,
>    `UpdatePreeditText` with it, the cursor at its end, visible when
>    non-empty, mode `IBUS_ENGINE_PREEDIT_COMMIT` (1), except as
>    `linux.ibus.osk-backspace` says
>
> The preedit to show is *p*, after any held text
> (`linux.ibus.context.fallback`). A text is an `IBusText`, the GVariant
> `(sa{sv}sv)` of `"IBusText"`, no attachments, the text, and an
> `IBusAttrList` `(sa{sv}av)`; a preedit's list holds one `IBusAttribute`
> `(sa{sv}uuuu)` of type 1, value 1 (single underline) over *p*, in
> scalar positions (Source: `src/ibustext.c`, `src/ibusattrlist.c`,
> `src/ibusattribute.c`).

> [spec:kbdgen:sem:linux.ibus.context]
> With `IBUS_CAP_SURROUNDING_TEXT` (1 << 5) among the client's
> capabilities, the context comes from `SetSurroundingText(text, cursor,
> anchor)`, positions in scalar values, the *reading* being the text before
> min(`cursor`, `anchor`). The engine keeps the committed text before the
> caret, at most 64 scalar values, and reconciles readings with it as
> `ldml.chromeos.context` does: one that agrees with the state its latest
> edit led to completes the context, one that agrees with an earlier state
> lags and is dropped, and any other replaces the context while the engine
> state is kept. A reading with `cursor` ≠ `anchor` makes the next key
> reset the state and start from an empty context (`tsf.edit.session`).
> `at_start` is false: a reading is a window of the text, a GTK paragraph
> for instance, not known to start where the text starts. GTK's IBus
> module asks for surrounding text before each key (Source:
> `client/gtk2/ibusimcontext.c`); text-input-v3 clients reach IBus through
> GNOME Shell (`js/misc/inputMethod.js` `vfunc_set_surrounding`), whose
> readings may lag a key. The engine emits `RequireSurroundingText` on
> `Enable`.

> [spec:kbdgen:sem:linux.ibus.context.fallback]
> Without `IBUS_CAP_SURROUNDING_TEXT`, as in terminals and X11 clients, the
> context is the *cache*: the last `context_len` scalar values that the
> engine's own edits produced since the last reset, with `authoritative`
> false (`tsf.edit.cache`). The last *h* of them are held back from the
> client and shown ahead of the engine's preedit, where *h* is
> `context_len` for a model that rewrites committed text
> (`ldml.linux.rewrites`) and 0 otherwise. An edit drops *d* scalar values
> from the cache and appends *s*; what then precedes the last *h* is
> committed. Since the engine reads at most `context_len` scalar values
> (`ldml.engine.context`), *d* never reaches past the held text, and a
> model that does not rewrite gives *d* = 0, so its text is committed at
> once and the cache only keeps markers pending after committed text. A
> passing key commits the held text with the edit of `Commit`; a reset or
> focus change leaves it to the client as preedit
> (`IBUS_ENGINE_PREEDIT_COMMIT`). ibus-keyman instead forwards BackSpace
> key events and restores their order with a sentinel key that a system
> service injects (`engine.cpp`); forwarded keys and committed text take
> different paths through GTK and GNOME Shell, so this engine forwards
> no keys. Without `IBUS_CAP_PREEDIT_TEXT` (1 << 0) nothing can be held:
> *h* is 0, no preedit is shown, the context is empty, so the engine never
> deletes, and the edit of `Commit` is applied before passing a key
> (`tsf.edit.reset`).

> [spec:kbdgen:sem:linux.ibus.osk-backspace]
> GNOME Shell's on-screen Backspace deletes with `delete_surrounding`
> without the engine, unless a preedit is shown, nothing has input focus
> or the field is a terminal, when it presses BackSpace (Source:
> `js/ui/keyboard.js` `toggleDelete`). A pending dead key is cancelled by
> Backspace (`ldml.engine.backspace.default`), so it must be shown: while
> markers are pending and nothing else is shown, as after a dead key with
> `standalone: ""`, the engine MUST show the trailing marker's display
> (`ldml.model.displays`), else U+25CC, with mode
> `IBUS_ENGINE_PREEDIT_CLEAR`, so that a reset drops it instead of
> committing it. A deletion that bypasses the engine reaches it as a
> reading that disagrees with its context (`linux.ibus.context`); the
> layout's `backspace` transforms do not run for it
> (`ldml.linux.classify.touch`).

> [spec:kbdgen:req:linux.ibus.security]
> In a field whose `ContentType` purpose is `IBUS_INPUT_PURPOSE_PASSWORD`
> (8) or `IBUS_INPUT_PURPOSE_PIN` (9), the engine MUST still map keys, so
> a password types the same everywhere (`tsf.security.disabled`), but MUST
> NOT show a preedit, keep a cache or keep surrounding text: the context
> is empty, and the edit of `Commit` is applied before passing a key. A
> dead key pending there when focus leaves is dropped.

### Robustness and privacy

> [spec:kbdgen:req:linux.ibus.logging]
> The engine MUST NOT record typed text, surrounding text, preedit text,
> keyvals, key codes or engine states anywhere. It writes diagnostics only
> to standard error, which ibus-daemon's session journal receives, and only
> messages compiled into the program, the reasons of `linux.ibus.inert`
> with a file's name, and panic locations: file, line and column, never the
> message (`tsf.component.logging`). `--xml` writes nothing to standard
> error (`linux.ibus.lister`). The engine talks to no process but
> ibus-daemon, opens no network connection, starts no process and writes
> no file.

> [spec:kbdgen:req:linux.ibus.panic]
> One process serves every input context, and a panic that unwound out of
> it would end them all (`tsf.component.panic`). The crate MUST build with
> `panic = "unwind"` and handle every D-Bus method call and property
> access inside `catch_unwind`. On a panic it logs the location, replies
> false to `ProcessKeyEvent` and success to anything else, and marks the
> process *poisoned*: a poisoned process passes every key, emits nothing,
> and exits once ibus-daemon has destroyed its last engine object, so the
> next activation starts a fresh one. Meanwhile the engine's base keymap,
> the XKB layout, types.

## On-screen keyboards

For now the on-screen layouts show each layout's desktop layout, its
`linux` keyboard presented as touch (`ldml.linux.touch`); authored Linux
touch layouts are later work (Decisions).

> [spec:kbdgen:def:linux.osk.gnome]
> `gnome-shell/<uuid>/` (`linux.ids`) is a GNOME Shell extension that adds
> the bundle's on-screen layouts to GNOME Shell 50 and 51. It holds:
>
> - `metadata.json`, pretty JSON with `uuid`; `name` and `description`
>   from `project.yaml` `locales.en`; `shell-version` `["50", "51"]`;
>   `session-modes` `["user", "unlock-dialog"]`, so the lock screen's
>   keyboard has the layouts too; and `version-name` from
>   `targets/linux.yaml` `version`. GNOME Shell refuses an extension
>   without the first four, whose UUID differs from its directory's name,
>   or whose `shell-version` lacks its major version (Source:
>   `js/misc/extensionUtils.js`, `js/ui/extensionSystem.js`)
> - `extension.js`, the same for every bundle and embedded in kbdgen: an
>   ES module whose default export extends GNOME Shell's `Extension`;
>   `enable()` loads `osk-layouts.gresource` from the extension's `path`
>   with `Gio.Resource.load` and registers it, `disable()` unregisters it
> - `osk-layouts.gresource`, a GResource bundle with
>   `/org/gnome/shell/osk-layouts/<L>.json` for each layout
>   (`ldml.linux.touch.gnome`), the JSON without insignificant whitespace,
>   as `json-stripblanks` leaves it. kbdgen writes the bundle itself in
>   GLib's GVDB format, with the `gvdb` crate (MIT or Apache-2.0), so no
>   `glib-compile-resources` is needed on any host
>
> GNOME Shell registers its own layouts as a GResource under the same
> prefix (`js/ui/main.js` `_loadOskLayouts`) and reads one with
> `Gio.File.new_for_uri('resource:///org/gnome/shell/osk-layouts/<id>.json')`
> each time the layout or the field's purpose changes (`js/ui/keyboard.js`
> `KeyboardModel`, `_updateLayout`). GLib looks a path up in every
> registered bundle, so the extension's bundle adds files without
> replacing the shell's. (Unverified: `linux.osk.gnome.spike`.)

> [spec:kbdgen:req:linux.osk.gnome.spike]
> Before `linux.osk.gnome` is built, a spike MUST check on GNOME Shell 50
> and 51 under Wayland, with an extension of that shape and one layout:
>
> 1. once enabled, without restarting the shell, the on-screen keyboard
>    shows the layout for the XKB source L and for the IBus source whose
>    engine's `layout` is L, in an application and on the lock screen
> 2. once disabled, it falls back to `us`
> 3. a key whose `keyval` is a dead keysym of L reaches the IBus engine as
>    a hardware key whose key code and level modifiers type that keysym in
>    L (`linux.ibus.virtual-keys`), and a key whose string has several
>    scalar values reaches it as one `ProcessKeyEvent` each
>
> Each result is recorded here as Verified, with the versions. If (1)
> fails, `extension.js` instead replaces `Gio.File.new_for_uri` while
> enabled, mapping each of its own layouts'
> `resource:///org/gnome/shell/osk-layouts/<L>.json` to the file
> `osk-layouts/<L>.json` in the extension's directory, which kbdgen then
> writes instead of the bundle, and every other URI to the original,
> which `disable()` restores. If (3) fails, on-screen dead keys are
> written as their standalone (`ldml.linux.touch.gnome`). Either way the
> layouts can be contributed to gnome-shell's `data/osk-layouts`, as
> Stevia's are (`linux.osk.stevia`).

> [spec:kbdgen:sem:linux.osk.gnome.selection]
> GNOME Shell takes the on-screen layout from the active input source's
> *xkbId*: an XKB source's id, or for an IBus source its engine's `layout`
> joined by `+` to its `layout_variant`. It tries `<xkbId>`, then the part
> before `+`, then `us`, each with `-extended` appended in terminals
> (Source: `js/ui/status/keyboard.js` `InputSource._getXkbId`,
> `js/ui/keyboard.js` `_updateLayout`). Both of a layout's sources
> therefore show `<L>.json`, and terminals show `us-extended` (Decisions,
> Later). With the IBus source, every on-screen key goes through the
> engine (`linux.ibus.virtual-keys`). With the XKB source, GNOME Shell
> commits on-screen strings itself, past IBus, and presses keyvals on its
> virtual device, so a dead key pressed on screen composes only with a
> hardware key (`ldml.linux.classify.touch`).

> [spec:kbdgen:def:linux.osk.plasma]
> `plasma-keyboard/<id>/` (`linux.ids`) is a plasma-keyboard layout
> package for Plasma 6.9 and later, one per layout. plasma-keyboard reads
> such packages from `plasma/keyboard/keyboardpackages` under every
> `GenericDataLocation` (Source: `keyboardlayoutmetadata.cpp`). It holds:
>
> - `metadata.json`, pretty JSON: `KPlugin` with `Id` (the package id),
>   `Name` (the display name of `linux.xkb.registry`), a `Name[<k>]` for
>   every other display name, `<k>` its tag with `-` replaced by `_`, and
>   `Version` (`targets/linux.yaml` `version`); and `X-Plasma-Keyboard`
>   with `KeyboardLayouts`, the one entry {`File`: `desktop.qml`,
>   `FormFactors`: `["phone", "desktop", "mediacenter"]`, `Id`:
>   `desktop`, `Name`: the display name}, and `Locales`, the tag with `-`
>   replaced by `_`
> - `contents/layouts/desktop.qml` (`ldml.linux.touch.plasma`)
>
> The entry lists every form factor plasma-keyboard's own packages use, so
> its settings offer the layout whatever the device (Source:
> `kcm/plasmakeyboardkcm.cpp`). Users enable layouts there
> (`enabledKeyboardLayoutIds`), not by XKB layout, so these ids need not
> match L. plasma-keyboard 6.5 to 6.8 compiled Qt Virtual Keyboard
> layouts in and read at most one layouts directory (Source: `v6.7.90`
> `src/layoutpathhelper.h`); kbdgen does not target them. KWin runs one
> input method, so where plasma-keyboard runs IBus does not, and its
> layouts type without the engine (`ldml.linux.classify.touch`).

> [spec:kbdgen:def:linux.osk.stevia]
> Stevia compiles its layouts in, as `/mobi/phosh/stevia/layouts/<layout>`
> or `<layout>+<variant>` `.json`, and loads the one the active XKB
> source's registry entry names, with no fallback (Source:
> `src/pos-osk-widget.c` `pos_osk_widget_set_layout`,
> `src/pos-input-surface.c`). kbdgen therefore writes
> `stevia/layouts/<L>.json` per layout (`ldml.linux.touch.stevia`) for
> contributing upstream: each file goes to Stevia's `src/layouts/` with a
> line `<file preprocess="json-stripblanks">layouts/<L>.json</file>` in
> `src/phosh-osk-stevia.gresources.xml`. Until Stevia ships it,
> `G_RESOURCE_OVERLAYS=/mobi/phosh/stevia/layouts=<dir>` in Stevia's
> environment adds the files without rebuilding, the mechanism Stevia
> documents for testing layouts (`doc/testing-layouts.md`); it serves
> `linux.test.acceptance`, not users. Stevia has no IBus; its engine
> hooks are compiled-in completers that it matches to `ibus` input sources
> by engine name and language, which is where a later completer hosting
> `kbd-engine` would go (`linux.ibus.planner`).

## Testing

> [spec:kbdgen:req:linux.test.golden]
> For a bundle with a v3 layout with a `windows` section, a v3 layout with
> a `chromeOS` section only, a v4 layout with the `abnt2` form, long-press
> candidates, gaps and dead keys on AltGr, and a v3 layout with neither
> `windows` nor `chromeOS`, every file of `linux.output` other than
> the models and the GResource bundle, the bundle's file list and
> contents read back, and the messages of `ldml.linux.classify` and
> `ldml.linux.classify.touch` MUST equal checked-in golden files byte for
> byte. Each model MUST decode to the keyboard of `ldml.linux.input`. Every
> GNOME Shell and Stevia layout MUST hold only the fields of its format,
> and every `levelSwitch` MUST name a level of its layout.

> [spec:kbdgen:req:linux.test.xkbcommon]
> Where libxkbcommon is installed, a test MUST, for each golden layout:
>
> - compile the keymap of layout L with the output's `xkb/` before the
>   default include paths, as `xkbcli compile-keymap --include <xkb>
>   --include-defaults --layout L` does, with no error or warning logged
> - find L in libxkbregistry with the same paths, with its description,
>   short description, languages and countries
> - press every key of `ldml.linux.levels` in all eight states through
>   `xkb_state_key_get_one_sym` and `xkb_state_key_get_utf8`, which MUST
>   give what `ldml.linux.semantics` predicts
>
> It uses the `xkbcommon` crate, a dev-dependency on Linux only. Without
> libxkbcommon it says so and passes.

> [spec:kbdgen:req:linux.test.planner]
> The planner (`linux.ibus.planner`) MUST be tested on every host against
> a fake client: a field that applies the signals of `linux.ibus.edit`,
> and reports its surrounding text on time, one key late, or not at all,
> with or without preedit. The cases are those of
> `ldml.chromeos.test.typing`, sent as IBus key codes and state bits, plus
> GNOME Shell's virtual keys, the on-screen Backspace that bypasses the
> engine, Left Alt, a password field, and for a model that rewrites
> committed text every pair of positions without surrounding text.
> Committed text and preedit MUST agree with the engine run natively.
> Fault injection MUST panic in each D-Bus entry point and check the reply
> and the poisoned process (`linux.ibus.panic`).

> [spec:kbdgen:req:linux.test.ibus]
> Where `ibus-daemon` is installed, a test MUST run the built
> `ibus-engine-divvun` under a private ibus-daemon: its own address, its
> `HOME` and `XDG_*` directories in a temporary directory,
> `IBUS_COMPONENT_PATH` naming a directory with only the engine's
> component, whose commands pass `--data-dir` with the golden output's
> `ibus/` tree, and no panel or config service. Acting as the client over
> D-Bus, it creates an input context (`org.freedesktop.IBus` method
> `CreateInputContext`), sets its capabilities, focus, engine and
> surrounding text, sends `ProcessKeyEvent`, and applies the
> `CommitText`, `UpdatePreeditText` and `DeleteSurroundingText` signals to
> a fake field, which MUST then hold what `linux.test.planner` expects for
> the same cases. It also checks ibus-daemon's engine list against
> `--xml`, and `CreateEngine` of a corrupt model (`linux.ibus.inert`).
> Without ibus-daemon it says so and passes.

> [spec:kbdgen:req:linux.test.acceptance]
> An acceptance run MUST install a fixture keyboard as users get it, both
> system-wide and per user, on a GNOME 50 or later VM, a Plasma 6.9 or
> later VM and a Phosh VM, and type:
>
> - on GNOME, with the XKB source and with the IBus source, into a GTK 4
>   field, a Qt 6 field, Chromium, a terminal and an XWayland client, by
>   hardware keys and by the on-screen keyboard, on the lock screen too
> - on Plasma, with the XKB layout and plasma-keyboard's package, and with
>   the IBus engine as KWin's input method
> - on Phosh, with the XKB layout and Stevia running the fixture's layout
>   through `G_RESOURCE_OVERLAYS`
>
> The XKB layout MUST type what `ldml.linux.semantics` predicts, and the
> IBus source what the engine types, except as the reports explain. The
> run is manual, like `tsf.test.vm`; its record names the versions, and
> settles each Unverified claim of this file.

> [spec:kbdgen:sem:linux.test.ci]
> CI runs `linux.test.golden`, `linux.test.planner` and
> `ldml.linux.test.typing` on every host, and `linux.test.xkbcommon` and
> `linux.test.ibus` on its Linux jobs, which MUST install libxkbcommon
> and ibus for them. No CI job can run GNOME Shell, KWin,
> plasma-keyboard, Phosh or Stevia, or a Wayland client: the GNOME spike
> (`linux.osk.gnome.spike`) and `linux.test.acceptance` are manual, on
> VMs.


## Changes to shared rules

These rules describe code that exists, so they change in the same commit as
the Linux code that implements the change, not before:

- `bundle.structure.targets`: `targets/linux.yaml` is recognised, with
  required `packageId`, `layoutPrefix` and `version`, read as strings
  (`linux.ids`); applied with `targets.linux.target-file`.
- `cli.commands` and `pipeline.steps`: `linux` is a `<PLATFORM>` with no
  subcommands, a full build of one step (`linux.output`); applied with
  `targets.linux.target-file`.
- `ldml.yaml.coexistence`: `linux` builds both formats from the model, v3
  layouts through in-memory migration from their Windows section, else
  their ChromeOS section (`ldml.linux.input`); applied with
  `targets.linux.input`.
- `ldml.scope.deferred` and `docs/spec/ldml/overview.md`: XKB output is no
  longer deferred, the IBus engine is a workspace host that calls the
  engine directly, and the overview lists `ldml/linux.md`; applied with
  `targets.linux.xkb.symbols` and `targets.linux.ibus.component`.
- `ldml.engine.shortcuts`: `alt_l` passes for host `linux` too, as for
  `windows` and `chromeOS`, since Linux desktops keep Left Alt for menus
  and shortcuts and the XKB layout leaves it Alt (`linux.ibus.modifiers`,
  `ldml.linux.levels`); applied with `targets.linux.ibus.keys`.
- `ldml.engine.event`: `Emit` is also the key-output event of hosts that
  receive text rather than keys, as the IBus engine does from GNOME
  Shell's on-screen keyboard (`linux.ibus.virtual-keys`), not only of
  tests; applied with `targets.linux.ibus.keys`.
- `ldml.crate.layout`: the workspace gains `crates/kbd-ibus`, a binary
  crate outside `default-members` (`linux.ibus.crate`); applied with
  `targets.linux.ibus.component`.

## Decisions

The operator settled these on review:

- **On-screen layouts show the desktop layout.** The OSKs present the
  `linux` keyboard's hardware set as touch (`ldml.linux.touch`). The
  `android` keyboard and authored touch sets are not used, and there is no
  `touch.linux` variant or touch chain for the `linux` host.
- **Left Alt** passes in the engine for host `linux`
  (`ldml.engine.shortcuts`, under Changes to shared rules); the IBus host
  sends `alt_l` and lets the engine decide (`linux.ibus.modifiers`).
- **v3 layouts** take their Windows section, else their ChromeOS one
  (`ldml.linux.input`).
- **Divvun's name** is on the engine, its component and its data
  directories, as on the text service; bundles from other organisations
  share the one engine.
- **No Compose file** for the XKB layout: it would be per user, replace the
  user's own file, and only GNOME's IBus would read it. The IBus engine
  types the layout's compositions.
- **GNOME versions**: `shell-version` is a kbdgen constant, extended once a
  GNOME release passes `linux.test.acceptance`; there is no bundle
  setting.
- **Held text** without surrounding text is accepted
  (`linux.ibus.context.fallback`): forwarding BackSpace cannot keep its
  order without a system service.
- **Workspace rules**: `ldml.crate.layout` gains `crates/kbd-ibus` (Changes
  to shared rules).

Later:

- authored Linux touch layouts, with a `touch.linux` variant and a touch
  chain for the `linux` host
- icons: a `resources/linux` directory (`bundle.resources`) for the
  engine's `<icon>` and the KPackage's `Icon`
- GNOME Shell's terminal layouts, `<L>-extended.json`, from the `linux`
  keyboard with Tab, Ctrl, Alt and arrows
- a per-layout XKB id (`targets.linux.xkbLayout`, like
  `targets.chromeOS.xkbLayout`) for layouts accepted into xkeyboard-config
  or Stevia under a country variant
- a Fcitx5 addon, which Fcitx5 needs because it cannot host IBus engines,
  and a Stevia completer hosting `kbd-engine`, both reusing the planner
  (`linux.ibus.planner`)
