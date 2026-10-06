# Keyboard model, engine and LDML: scope and decisions

kbdgen gets its own keyboard model and engine. The model is a *superset* of
CLDR LDML Keyboard 3.0 (UTS #35 Part 7, stable since CLDR 45). Everything an
LDML keyboard can say, the model can say with LDML's meaning. On top of
that, the model treats as first-class the things LDML leaves out but
kbdgen's engine or native compilers need:

- per-host layouts
- the Windows extras: extra modifiers bound to Right Ctrl, Caps Lock or
  `B00`; Shift Lock; LRM/RLM; dead-key names; key-name overrides; and the
  49th key
- native-only shortcut layers (`ctrl`, `cmd`)
- touch key roles and labels
- the numpad decimal key
- the text committed when a dead key is interrupted

The parts are:

- **the model** (`ldml.model.*`): one normalized form, read by the engine
  and by the native compilers
- **the engine** (`ldml.engine.*`): deterministic, turning key events into
  edit operations. It is hosted on every operating system: Windows TSF
  (`docs/spec/tsf.md`, the same workspace), macOS IMKit, the iOS and Android
  keyboard apps, ChromeOS and the web through wasm, and Linux IBus and
  Fcitx. There is no other engine.
- **LDML XML** (`ldml.xml.*`): import and export through `xmlem`. It maps
  the LDML subset losslessly and carries the superset in a kbdgen namespace,
  which LDML-only consumers ignore.
- **YAML v4** (`ldml.yaml.*`): the authoring format, compiled to the model.

The `ldml.*` prefix names this spec tree; it does not mean the engine is
LDML-only.

| File | Prefix | Concern |
|---|---|---|
| `overview.md` | `ldml.scope.*` | v1 scope, deferrals, where the macOS rules live, operator questions |
| `crate.md` | `ldml.crate.*` | workspace members, `no_std`, wasm, FFI, the TSF crate |
| `model.md` | `ldml.model.*` | the superset model and its binary encoding |
| `xml.md` | `ldml.xml.*` | keyboard3 XML through `xmlem`: read, write, imports, syntax, resolution, extensions |
| `engine.md` | `ldml.engine.*` | the runtime API (`tsf.engine.api`), state and semantics |
| `test.md` | `ldml.test.*` | CLDR vectors, golden vectors, round trips |
| `yaml.md` | `ldml.yaml.*` | the v4 authoring format and its lowering |
| `migrate.md` | `ldml.migrate.*` | v3 to v4 conversion and the defect report |
| `kbdl.md` | `ldml.kbdl.*` | the model to `kbdl.input` adapter |
| `android.md` | `ldml.android.*` | the model to Android layout resources adapter |
| `cli.md` | `ldml.cli.*` | `kbdgen ldml` subcommands |

Rule bodies flag LDML gaps and kbdgen additions with fixed phrases:

- "LDML is silent" or "LDML is ambiguous" marks a gap in LDML.
- "Extension" marks a superset feature.
- "Divergence" marks a deliberate difference from LDML or from
  `docs/spec/tsf.md`.

Sources:

- UTS #35 Part 7, Keyboards, version 48.2 (tr35-78); the CLDR 49 draft has
  the same keyboard text:
  <https://www.unicode.org/reports/tr35/tr35-keyboards.html>
- CLDR `keyboards/dtd/ldmlKeyboard3.dtd` and `ldmlKeyboardTest3.dtd`,
  `keyboards/3.0/*.xml`, `keyboards/import/*.xml`, `keyboards/test/*.xml`
  and the test `README.md`:
  <https://github.com/unicode-org/cldr/tree/main/keyboards>
- Keyman `kmc-ldml` (the CLDR validator) and the KMX+ format description,
  as a reference implementation
- `xmlem` 0.3.3 source (`document.rs`, `display.rs`, `element.rs`)
- the kbdgen LDML gap analysis of 2026-10-05 (concept mapping, bundle
  survey, migration table, semantic divergences); a working document, not
  in the repository
- `docs/spec/{layout,bundle,kbdl,macos,tsf}.md`

## Scope

> [spec:kbdgen:def:ldml.scope.v1+1]
> Version 1 implements these features end to end: through XML, the model
> and the engine. The last column says what v4 YAML offers for each.
>
> | Feature | v4 YAML |
> |---|---|
> | LDML `keyboard3` `locale`, `conformsTo`, `draft` (kept, no effect); `locales`, `version`, `info`, `settings` | yes |
> | LDML `import`, both local and `base="cldr"` | XML only (`ldml.scope.deferred`) |
> | LDML `displays`, `display` (by output and by `keyId`), `displayOptions` | yes |
> | LDML `keys` with every attribute; implied keys | yes |
> | LDML `flicks`; `forms` (implied `us` `iso` `jis` `ks` `abnt2`, and custom) | yes |
> | LDML `layers` (one hardware set, several touch sets), every modifier component including `other` | yes |
> | LDML `variables`, `transforms` (`simple` and `backspace`), the full regex-like syntax, `reorder`, markers | verbatim, plus dead-key sugar |
> | LDML normalization, on and `disabled`, with marker gluing | yes |
> | Extension: per-host documents; the modifier components `cmd` and `extra1`–`extra3`; native-only layers; numpad decimal; dead-key flush output; touch roles, labels and bottom-row policy; dead-key names; Windows key-name overrides, Shift Lock, LRM/RLM, and extra-modifier bindings | yes |
> | LDML keyboardTest3 `startContext`, `keystroke` (`flick`, `longPress`, `tapCount`), `emit`, `backspace`, `check` | n/a |

> [spec:kbdgen:req:ldml.scope.deferred]
> The following are out of scope for v1. A keyboard that uses one still
> loads.
>
> - keyboardTest3 `repertoire` tests are parsed and not run
>   (`ldml.test.repertoire`).
> - v4 has no syntax for LDML `import` or a YAML include. Layouts that share
>   tables use `ldml:` XML files with local imports.
> - The engine produces no "error indication" output; LDML defines no
>   element for one.
> - Emoji-cluster-aware default backspace is not done. The `codePoint`
>   policy deletes one scalar value (`ldml.engine.backspace.default`).
> - The model is not compiled to macOS `.keylayout` or XKB, and the iOS,
>   Android and ChromeOS generators are not ported to v4
>   (`ldml.yaml.coexistence`).
> - There are no C ABI, JNI or wasm-bindgen host bindings (`ldml.crate.ffi`).
> - There is no differential testing against Keyman Core (`ldml.test.oracle`).
> - No filler base is inserted for a lone prebase character in `reorder`
>   (`ldml.engine.reorder`); LDML is ambiguous about it.

> [spec:kbdgen:sem:ldml.scope.macos-rules]
> The operator's macOS rules are realised as follows, without changing what
> an LDML keyboard means.
>
> | Rule | Realised by |
> |---|---|
> | An unmatched dead key emits its standalone output, then the key | the v4 lowering (`ldml.yaml.dead-keys.fallback`). Exported XML carries the generated rules, so LDML-only engines agree. |
> | Caps+Shift+letter gives uppercase | v4 implied layers (`ldml.yaml.implied-layers`); layer matching itself stays exact (`ldml.engine.modifiers`) |
> | No output normalization | v4 defaults to LDML `settings normalization="disabled"` (`ldml.yaml.normalization`). Keyboards with normalization on are processed as LDML says (`ldml.engine.normalization`). |
> | Backspace with a pending dead key cancels only it | the engine's default backspace policy (`ldml.engine.backspace.default`) and a generated backspace rule (`ldml.yaml.dead-keys.backspace`) |
> | A pending dead key is shown, and stays as its standalone output on interruption | the flush outputs (`ldml.engine.preedit`, `ldml.engine.commit`); Extension |

## Operator questions

The gap analysis listed 16 open questions. Each is decided here, or marked
open.

| # | Question | Decision |
|---|---|---|
| 1 | Keep per-OS desktop variants, or converge? | Decided: v4 has `hardware.default` and per-host variants (`ldml.yaml.hardware`). The migrator keeps today's variants. Converging them is optional linguistic work per language, not a migration step. |
| 2 | Policy for an unmatched dead key | Decided (macOS rule): emit the standalone output, then the key. A dead key with `standalone: ""` gives "drop" behaviour for that key (`ldml.yaml.dead-keys.fallback`). |
| 3 | Caps+Shift+letter | Decided (macOS rule): uppercase in the engine (`ldml.yaml.implied-layers`). The Windows layout DLL keeps Windows' convention (`ldml.kbdl.caps`). |
| 4 | Normalization | Decided: v4 defaults to `disabled`, so the engine emits exactly what is authored. `normalization: enabled` opts into LDML NFD matching with NFC output (`ldml.engine.output.form`). |
| 5 | Backspace | Decided: Backspace cancels trailing markers. Otherwise it passes unless a rule matches (`ldml.engine.backspace.default`). |
| 6 | Touch bottom row | Decided: the host draws it by default; `bottomRow: authored` hands it to the author (`ldml.yaml.touch`). |
| 7 | Touch convergence | Decided: `touch.default` plus per-host variants with `inherits`. The migrator keeps iOS and Android separate. |
| 8 | `minDeviceWidth` thresholds | Decided: the size names are `phone` (no width), `tablet` (95 mm, the same as Android's 600 dp) and `tablet-large` (190 mm), and authors may override them (`ldml.yaml.touch`). Hosts may still select by name (`ldml.engine.touch`). |
| 9 | Hardware keyboards on Android | The model and engine support it: the Android document carries a hardware set (`ldml.yaml.hosts`). **Open, needs the operator:** whether the Android app routes hardware events through the engine. That is a product decision for the Android host work. |
| 10 | Ctrl and Cmd | Decided: layers using them are native-only (`ldml.model.native`), and the engine passes shortcuts through (`ldml.engine.shortcuts`). |
| 11 | Reference LDML XML directly | Decided: yes, with `ldml:` (`ldml.yaml.ldml-ref`). CLDR keyboards are accepted, and the `base="cldr"` imports are embedded (`ldml.xml.cldr-data`). |
| 12 | Fields to drop or adopt | Decided for layout fields: Windows `languageName`, `legacyName` and `space` are dropped and reported (`ldml.migrate.fields`). The target-file fields (`sentryDsn`, `aboutDir`, `icon`, `provisioningProfileId`, `chrome.yaml`, `resources/chrome`) are packaging and stay with their target nodes. |
| 13 | Test format | Decided: run CLDR keyboardTest3 as it is (`ldml.test.cldr`). kbdgen's own vectors are YAML, a superset with hardware modifiers and preedit checks (`ldml.test.vectors`). |
| 14 | Shared transform tables | Deferred (`ldml.scope.deferred`): use `ldml:` XML with local `import`. **Open, needs the operator** only if sharing at the YAML level becomes a priority. |
| 15 | Plan nodes | Done: the `ldml.*` WBS exists. The `ldml.spec` report proposes additions. |
| 16 | Dependency repositories not surveyed | Assigned to `ldml.migrate.survey`: run the migrator in report mode over every dependency repository before v4 is declared frozen. |
