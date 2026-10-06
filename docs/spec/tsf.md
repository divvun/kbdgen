# Windows text service (TSF)

On Windows a Divvun keyboard is a Text Services Framework *text input
processor* (TIP), the *text service*, backed by the `KBDTABLES` layout DLL
of `docs/spec/kbdl.md`. Users see one entry per keyboard: the text service's
profile (`tsf.register.enable`). The text service runs kbdgen's keyboard
engine, `kbd-engine` (`ldml.engine.*`), on the layout's model, so it does
what tables cannot: dead-key and transform output of any length, deep
chains, rules on visible text, and a visible pending state.
The layout DLL is the fallback wherever Windows reads keyboard layouts
without TSF: the sign-in screen, the secure desktop and games, and any
machine without the text service. Windows 11's console host is not such a
place: it runs the text service (`tsf.test.acceptance`). It also carries the text service's
data (`tsf.data.resource`). Keyboard semantics are those of
`ldml.engine.*`; these rules cover only what is specific to TSF.

These rules cover:

- the component shape
- keyboard data
- how the text service relates to the layout DLL
- key handling and the edit operations
- security contexts
- architectures and registration
- installer and `kbdi` responsibilities
- the engine boundary
- testing

The text service's source is the kbdgen workspace crate `crates/kbd-tsf`
(`ldml.crate.layout`, `ldml.crate.tsf`). Rules naming `kbdi` or the
keyboard installer bind those projects (github.com/divvun/kbdi,
divvun-actions `actions/keyboard/build`), not kbdgen.

**Pairing with the layout DLL is not what was first planned.** The original
plan paired the text service with the layout DLL as its TSF *substitute
HKL*. Windows does not apply a substitute HKL that is a plain layout
(`tsf.pairing.substitute`). Instead:

- The text service maps keys itself, from scan codes and its own copy of the
  layout (`tsf.pairing.self-sufficient`).
- The layout DLL stays registered as the fallback and as the carrier of the
  text service's data (`tsf.data.resource`).

**What "Verified" means.** "Verified" marks behaviour observed on Windows 11
25H2 (build 26200) x64. The test was a throwaway Rust TIP built with `cargo`
against `windows` 0.62.2 (128 KB x64, 110 KB x86, release, with `std`). It was
registered in both registry views, paired with kbdgen's `kbdvro` layout DLL
under test KLIDs, and driven by `SendInput` scan codes in an interactive
session. The test targets were:

- a Win32 `EDIT` control and a WinForms `RichTextBox`, both of which TSF
  presents as *transitory* contexts
- a WPF `TextBox`, which has a full text store
- the same controls from a 32-bit process

Claims about `crates/kbd-tsf` itself were verified on the same machine by
its VM test (`tsf.test.vm`), which types into those controls from 64- and
32-bit processes.

"Unverified" marks claims taken from documentation or other projects only.
Nothing was tested on Windows on Arm.

Sources:

- TSF reference:
  <https://learn.microsoft.com/en-us/windows/win32/tsf/text-services-framework>
  - `ITfInputProcessorProfileMgr::RegisterProfile`:
    <https://learn.microsoft.com/en-us/windows/win32/api/msctf/nf-msctf-itfinputprocessorprofilemgr-registerprofile>
  - category values:
    <https://learn.microsoft.com/en-us/windows/win32/tsf/predefined-category-values>
  - `ITfContext::RequestEditSession`, `ITfThreadMgrEx::ActivateEx`
  - `InstallLayoutOrTip`:
    <https://learn.microsoft.com/en-us/windows/win32/tsf/installlayoutortip>
- IME requirements (Store apps, AppContainer, UI):
  <https://learn.microsoft.com/en-us/windows/apps/develop/input/input-method-editor-requirements>
  and the Windows 8 cookbook
  <https://learn.microsoft.com/en-us/windows/win32/w8cookbook/third-party-input-method-editors>
- Shared and redirected registry keys:
  <https://learn.microsoft.com/en-us/windows/win32/winprog64/shared-registry-keys>
- Microsoft Windows-classic-samples `Samples/IME/cpp/SampleIME`
  (`Register.cpp`, `Server.cpp`, `KeyEventSink.cpp`, `Composition.cpp`,
  `CandidateListUIPresenter.cpp`)
- Keyman for Windows `windows/src/engine/kmtip` (`register.cpp`, `keys.cpp`,
  `kmkey.cpp`, `inserttext.cpp`, `README.md`) and `keyman32`
  (`serialkeyeventserver.cpp`). Keyman's
  `KPInstallKeyboardLanguage.pas` registers profiles with a null substitute.
- Marc Durdin, "Substituted layouts in Text Services Framework" (2017),
  quoting Microsoft: substitution "doesn't support substituting an arbitrary
  plain keyboard layout":
  <https://marc.durdin.net/2017/07/substituted-layouts-in-text-services-framework/>
- Arm64X documentation:
  <https://learn.microsoft.com/en-us/windows/arm/arm64x-pe>,
  <https://learn.microsoft.com/en-us/windows/arm/arm64x-build> (pure
  forwarders) and <https://learn.microsoft.com/en-us/windows/arm/arm64ec>
- LLVM 21 `lld/test/COFF/Inputs/loadconfig-arm64.s` and
  `loadconfig-arm64ec.s`, the load configuration and CHPE metadata an
  Arm64X image needs from its objects
- rustc platform support for `arm64ec-pc-windows-msvc` (Tier 2 since 1.81),
  and rust-lang/rust#145154, the broken merged Arm64X Rust DLLs
- windows-rs 0.62.2 (`Win32_UI_TextServices`, `windows_core::implement`)
- chewing/windows-chewing-tsf, a Rust TIP that uses an Arm64X pure forwarder
- The Rust 1.81 release notes on aborting at non-unwind ABIs:
  <https://blog.rust-lang.org/2024/09/05/Rust-1.81.0/>
- divvun/kbdi `main` (`src/keyboard.rs`, `src/keyboard_win8.rs`) and
  divvun-actions `actions/keyboard/build/{mod,iss,outto,wind,layouts}.ts`,
  including branch `keyboard-rust-layout-dlls`
- `docs/spec/ldml/engine.md` (`ldml.engine.*`), `docs/spec/ldml/crate.md`
  (`ldml.crate.*`), `docs/spec/ldml/model.md` (`ldml.model.encoding`,
  `ldml.model.windows`) and `docs/spec/ldml/kbdl.md`
  (`ldml.kbdl.*`); `src/build/windows/kbdl/resources.rs`

`divvun-wind` is unrelated to this layer. It is a per-session daemon that
injects a DLL into Explorer to relabel keyboards, and it runs only on
Windows 11 x64. It is not a TSF component, so the text service does not build
on it. The keyboard installer bundles the text service the same way it
bundles divvun-wind (`tsf.installer.bundle`).

## Component

> [spec:kbdgen:def:tsf.component]
> The *text service* is one in-process COM server built from the kbdgen
> workspace crate `crates/kbd-tsf` (`ldml.crate.tsf`). Every Divvun keyboard
> shares it: there is one CLSID, `{5E668C8A-2FB8-41D2-90B1-9C132653FA9D}`,
> and one DLL per architecture (`tsf.arch.builds`). Each installed layout
> adds a TSF language profile to it (`tsf.register.profile`) and supplies
> its keyboard data (`tsf.data.resource`). It is released, versioned (by the
> `kbd-tsf` package version) and installed separately from keyboards, so one
> installation serves them all; kbdgen's keyboard builds produce only the
> per-keyboard data. A shared binary keeps one copy loaded per process and
> one place to fix bugs. The cost is that keyboard data and text service
> versions must stay compatible (`tsf.data.version`).

> [spec:kbdgen:req:tsf.component.crate]
> `kbd-tsf` MUST be a Rust `cdylib` that uses `windows` and `windows-core`
> 0.62.x, implements COM interfaces with `windows_core::implement`, and runs
> `kbd-engine` with its default `normalization` feature (`ldml.crate.tsf`).
> It MUST export exactly `DllGetClassObject`, `DllCanUnloadNow`,
> `DllRegisterServer` and `DllUnregisterServer`. `DllGetClassObject` MUST
> return `CLASS_E_CLASSNOTAVAILABLE` for any other CLSID. `DllCanUnloadNow`
> MUST return `S_OK` only when no object and no server lock is alive.
> (Verified: a crate of this shape loads in x64 and x86 processes, activates
> and edits text.)

> [spec:kbdgen:req:tsf.component.interfaces+2]
> The TIP object MUST implement:
>
> - `ITfTextInputProcessorEx`
> - `ITfThreadMgrEventSink`, to reset on focus changes
> - `ITfKeyEventSink`
> - `ITfTextEditSink`, to detect edits it did not make
> - `ITfCompositionSink`
> - `ITfCompartmentEventSink`, to follow the keyboard-disabled and
>   empty-context compartments
> - `ITfDisplayAttributeProvider`, with an enumerator over one preedit
>   attribute
> - `ITfActiveLanguageProfileNotifySink`, to follow switches between its
>   profiles, which share one CLSID and so do not reactivate it
>
> `ActivateEx` MUST record the activation flags from
> `ITfThreadMgrEx::GetActiveFlags`, including `TF_TMF_SECUREMODE` and
> `TF_TMF_IMMERSIVEMODE`. `Deactivate` MUST unadvise every sink and end any
> composition it owns.

> [spec:kbdgen:req:tsf.component.panic]
> TSF loads the text service into every process the user types in, so a
> crash kills the host application. Since Rust 1.81 an unwind that reaches
> an `extern "system"` boundary aborts the process, and windows-rs vtable
> shims do not catch panics. The crate MUST therefore build with
> `panic = "unwind"` and run the body of every export and every COM method
> inside `catch_unwind`. On a panic it MUST return `E_UNEXPECTED`.
> It MUST also mark the process's text service *poisoned*. A poisoned text
> service MUST NOT eat keys or request edit sessions until it is unloaded.

> [spec:kbdgen:req:tsf.component.self-contained]
> The text service MUST NOT:
>
> - talk to other processes (no pipes, shared memory or window messages to
>   another process)
> - access the network
> - start processes
> - write files or the registry
>
> It needs only its install directory, the layout DLLs it reads
> (`tsf.data.locate`) and the HKLM registration. With no per-user settings it
> MUST behave fully. Its key path MUST NOT block on I/O after the keyboard
> data has loaded.

> [spec:kbdgen:req:tsf.component.logging]
> The text service MUST NOT record typed text, context text or key
> identities anywhere. Diagnostics MAY record:
>
> - HRESULTs
> - interface names
> - context flags
> - process names
> - panic locations
>
> It MAY write them only when a machine-wide diagnostic switch is on. It
> MUST NOT write them in secure or immersive mode.

## Keyboard data

> [spec:kbdgen:req:tsf.data.resource]
> For each layout, kbdgen MUST put the model (`tsf.engine.model`) in the
> layout DLL's `<name>.res`, written as in
> `[spec:kbdgen:syn:kbdl.resources.format]`: an `RT_RCDATA` (10) entry with
> name 1, language 0 and memory flags `0x0030`, placed after the
> `RT_VERSION` entry and before the string tables
> (`ldml.kbdl.model-resource`). Every variant built by `kbdl.build`
> therefore carries identical data. This keeps the text service's data in
> step with the table fallback and readable wherever the layout DLL is,
> because System32 and SysWOW64 are readable from AppContainers. A layout
> whose model cannot be built gets no resource, and its profile is inert.
> (Verified: `LoadKeyboardLayout` accepts layout DLLs carrying the resource,
> Windows 11 x64, 2026-10-05.)

> [spec:kbdgen:req:tsf.data.locate+1]
> On activation, and on each switch to another of its profiles
> (`tsf.component.interfaces`), the text service MUST find the layout under
> `HKLM\SYSTEM\CurrentControlSet\Control\Keyboard Layouts` whose
> `Layout Product Code` equals the profile GUID. The comparison ignores
> case and surrounding braces, because the legacy installer writes the code
> without a closing brace. The service MUST then load that key's
> `Layout File`, which must be a bare file name, from the system directory with `LoadLibraryExW(…,
> LOAD_LIBRARY_AS_DATAFILE | LOAD_LIBRARY_AS_IMAGE_RESOURCE)` and read the
> resource of `tsf.data.resource`. Results are cached per process. If any
> step fails, the profile is inert (it eats no keys). (Unverified: that this
> key is readable from AppContainers.)

> [spec:kbdgen:req:tsf.data.version]
> The model's header is that of `ldml.model.encoding`: the ASCII magic
> `DVKB`, then a little-endian u16 major and u16 minor version, currently
> 1.0. The text service decodes it with `Model::from_bytes`, which refuses
> an unknown major version and accepts a higher minor one
> (`ldml.model.decode`); on refusal the profile is inert. The keyboard
> installer installs the text service it bundles before registering layouts
> (`tsf.installer.bundle`), and that bundled release MUST be built from a
> kbdgen revision whose `kbd-model` reads the major version kbdgen wrote.
> A keyboard therefore never runs against a text service older than its
> model.

## Relation to the layout DLL

> [spec:kbdgen:sem:tsf.pairing.substitute]
> `RegisterProfile` stores `hklSubstitute` and `GetActiveProfile` reports
> it, but activating the profile does not activate that layout. While a TIP
> profile is active, the thread's HKL is the language's hidden dummy layout,
> which is US English under the profile's LANGID (`0x04090425` for an
> Estonian profile). Activating the paired HKL explicitly deactivates the
> TIP. (Verified with the substitute given as the HKL `0xF0E70409` and as the
> KLID `0xA0470409`, after `LoadKeyboardLayout`, with the layout in the
> user's list, and after restarting `ctfmon`. Microsoft states that
> substitution exists only to replace IMM32 IMEs.)

> [spec:kbdgen:req:tsf.pairing.self-sufficient]
> Profiles MUST be registered with `hklSubstitute` 0. The text service MUST
> produce every output of the layout itself, from scan codes and its model
> (`tsf.keys.identity`). It MUST NOT rely on the HKL beneath it for any key
> it claims. The layout DLL remains the input method wherever the text
> service is not consulted:
>
> - when the text service is not installed (`tsf.register.enable`)
> - at the welcome screen (`tsf.register.welcome`)
> - when the text service is poisoned or inert
>
> Both derive from one model, and the DLL's output equals the engine's on
> every table-expressible key, except as `tsf.test.differential` lists.

> [spec:kbdgen:sem:tsf.pairing.alternatives]
> Two other pairings were tried. Neither is adopted, even as a fallback.
>
> - **Per-user substitute of the dummy layout.** Setting
>   `HKCU\Keyboard Layout\Substitutes` for the language's dummy KLID (from
>   `HKCU\Software\Microsoft\CTF\HiddenDummyLayouts`) to the layout's KLID
>   puts the layout beneath the TIP after `ctfmon` restarts. Dead keys
>   passed through then work. But Windows rewrites the value to `00000409`
>   whenever it regenerates the layout registry, for example on
>   `Set-WinUserLanguageList`. (Verified.)
> - **Non-keyboard-category TIP beside the active layout.** A TIP registered
>   under a non-keyboard category, such as handwriting, receives keys beside
>   the active layout only after `ActivateProfile` inside that process.
>   `TF_IPPMF_FORSESSION` does not reach new processes. (Verified.)

## Key handling

> [spec:kbdgen:req:tsf.keys.identity+1]
> The text service MUST identify a key by its scan code and extended flag
> (`lParam` bits 16–23 and 24, completed as in `tsf.keys.recover`), never
> by its virtual key, which depends on the dummy HKL beneath
> (`tsf.pairing.substitute`). A key that is not extended becomes
> `Backspace` for `0e`, `Decimal` for `53`, and otherwise `Scan(code)`
> (`ldml.engine.event`). Extended keys, `VK_PACKET`, `VK_PROCESSKEY` and the
> service's own injected input (`tsf.edit.inject`) are never sent to the
> engine and pass. Modifiers are read with `GetKeyState` while handling the
> event: left and right Shift, Ctrl and Alt, Caps Lock's toggle as `caps`,
> either Win key as `cmd`, and held extra-modifier keys as `extra`
> (`ldml.engine.extra`). `repeat` is `lParam` bit 30.

> [spec:kbdgen:req:tsf.keys.recover]
> WPF reports key events whose `lParam` lacks the scan code or the
> extended flag. The text service MUST map the virtual key through the
> thread's dummy layout with `MapVirtualKeyExW(…, MAPVK_VK_TO_VSC_EX)` and
> complete the key's identity from the result. An `lParam` with scan code 0
> takes the mapped scan code and extended flag. One whose scan code equals
> the mapped one takes the mapped extended flag. Any other keeps its own.
> This also tells the navigation keys from the keypad's. The keypad's `53`
> with Num Lock off, which the dummy layout reports as `VK_DELETE`, is
> therefore the extended Delete and passes; it is not `Decimal`.
> (Verified: every case of `tsf.test.vm` types the same in a WPF `TextBox`
> as in `EDIT`. Unverified: the keypad's `53` with Num Lock off.)

> [spec:kbdgen:req:tsf.keys.claim+1]
> A key down is eaten exactly when the engine's action for it is not `Pass`
> (`ldml.engine.api`), and `OnTestKeyDown` and `OnKeyDown` agree on it
> (`tsf.keys.phases`). A key up MUST be eaten exactly when its key down was
> eaten. While a context is read-only or the service is poisoned or inert,
> every key passes. A passed key resets the context (`tsf.edit.reset`),
> unless the service is poisoned or inert. Shift, Ctrl, Alt, Win and Caps
> Lock alone are not sent to the engine and never reset it; a `B00` bound as
> an extra modifier is sent, and the engine consumes it
> (`ldml.engine.extra`). A form key with no output in the selected layer is
> eaten with no output (`ldml.engine.hardware`), not passed to the dummy
> layout.

> [spec:kbdgen:req:tsf.keys.phases]
> TSF calls `OnKeyDown` only for keys that `OnTestKeyDown` ate. (Verified.)
> `OnTestKeyDown` MUST therefore decide every key down. It asks the engine
> in a `TF_ES_SYNC | TF_ES_READ` edit session, changing no engine state,
> and keeps the decision with the key's virtual key, physical key and
> message time. A key that passes is handled at once: in a read-write
> session, the text service commits and resets as `tsf.edit.reset`
> requires. `OnKeyDown` MUST apply the kept decision when its key matches,
> and otherwise decide and apply in one read-write session. A key whose
> kept decision was to pass is not handled again.

> [spec:kbdgen:req:tsf.keys.altgr+1]
> The text service MUST treat Right Alt as AltGr, sending `alt_r` with
> `altgr` set (`ldml.engine.tsf`), when the layout DLL built from the same
> model sets `KLLF_ALTGR` (`kbdl.locale`, `ldml.kbdl.layers`). AltGr chords
> without Ctrl reach it only as preserved keys (`tsf.keys.preserved`), and
> the application sees Right Alt itself. Where Right Alt's own key events
> reach the key sink, the text service MUST eat them. A Left Ctrl that the
> system synthesises with Right Alt (same message time) MUST NOT be sent as
> `ctrl_l`. A Ctrl the user really holds with AltGr is sent, and reaches
> layers that name both ctrl and alt (`ldml.engine.altgr`). (Verified: an
> AltGr chord and an AltGr dead key type in `EDIT`, RichEdit and WPF.
> Unverified: how to tell synthesised from held Left Ctrl.)

> [spec:kbdgen:req:tsf.keys.preserved]
> TSF calls no key event sink method while Alt is held without Ctrl,
> because the window then gets `WM_SYSKEYDOWN`. (Verified.) Where Right Alt
> is AltGr (`tsf.keys.altgr`), the text service MUST register with
> `ITfKeystrokeMgr::PreserveKey` each AltGr and AltGr+Shift chord of an ISO
> position (`kbdl.scancodes.iso`) that the engine does not pass from
> `State::default()` with an empty context. Each is `TF_MOD_RALT`, with
> `TF_MOD_SHIFT` if shifted, on the dummy layout's virtual key for the scan
> code. `OnPreservedKey` MUST decide and apply the chord in one edit
> session. After an eaten chord it MUST send, signed as in
> `tsf.edit.inject`, a press and release of the unassigned virtual key
> `0xFF`, so the application, which saw Right Alt, does not open its menu
> bar. (Unverified: the menu bar. Other chords never reach the engine, even
> after a dead key.)

> [spec:kbdgen:req:tsf.keys.locale-flags]
> LRM/RLM on Shift+Backspace is the engine's (`ldml.engine.backspace`), so
> the text service sends Backspace with the Shift side held. When the
> model's `windows.shift_lock` is set (`ldml.model.windows`), the text
> service SHOULD send `caps` from its own Shift Lock state, which Caps Lock
> sets and Shift releases, instead of the system toggle. It cannot clear the
> system toggle, so where the two diverge the indicator light may disagree.
> (Unverified.)

## Edit operations

> [spec:kbdgen:def:tsf.edit.ops]
> An *edit* is a triple (*d*, *s*, *p*), the engine's `Edit { delete,
> insert, preedit }` (`ldml.engine.action`):
>
> - *d*: the number of Unicode scalar values to delete before the caret
> - *s*: a string to insert at the caret as committed text
> - *p*: the new preedit, shown after the caret as an uncommitted
>   composition (empty for none)
>
> They are applied in that order. *d* never exceeds the scalar values of the
> context passed to the engine. The text service ignores `Edit.layer`.

> [spec:kbdgen:thm:tsf.edit.units]
> The number of UTF-16 units to delete is well defined. It is the UTF-16
> length of the last *d* scalar values of the context string the text
> service passed to the engine. By `ldml.engine.action`, *d* is at most that
> context's scalar count, counted on the text as given even when the engine
> normalizes internally (`ldml.engine.output.segment`). The context ends at
> the caret, so those units are exactly the units before the caret. The text
> service therefore never splits a surrogate pair while the context is
> authoritative.

> [spec:kbdgen:req:tsf.edit.session+1]
> The text service MUST do its document work in `TF_ES_SYNC` edit
> sessions: read-only to decide a key, read-write to apply or pass it
> (`tsf.keys.phases`). It reads the context back from the caret, which is
> the start of the default selection, or of the composition while a preedit
> is shown. It shifts a clone's start back by up to 2 × `context_len`
> UTF-16 units, calls `GetText`, drops a low surrogate cut from its pair,
> and keeps the last `context_len` scalar values. The context is
> *authoritative* when the read succeeds and the context's static flags
> lack `TS_SS_TRANSITORY`. Otherwise, and when TSF grants no session, the
> cache of `tsf.edit.cache` is used. `at_start` is set only when an
> authoritative read stopped short of the request. A non-empty selection
> resets the engine first. (Verified: transitory contexts return no text
> before the caret; a WPF `TextBox` returns it. Unverified: `at_start`.)

> [spec:kbdgen:req:tsf.edit.apply+1]
> In an authoritative context, the edit MUST be applied in the edit session:
>
> 1. Clone the selection range, or the composition's start while a preedit
>    is shown, and shift its start back by the units of `tsf.edit.units`.
> 2. `SetText` it to *s*, which also replaces any selection.
> 3. Collapse the range to its end and, if no preedit is shown, make it the
>    selection.
> 4. Update the composition to *p* (`tsf.edit.preedit`).
>
> (Verified in a WPF `TextBox`: `a` then a key mapped to "delete 1, insert
> `a` U+0308 U+0303" leaves exactly those three units.)

> [spec:kbdgen:req:tsf.edit.inject+1]
> In a context that is not authoritative, an edit with *d* = 0 MUST be
> applied with `SetText` at the selection, or with `SendInput` when TSF
> grants no edit session. An edit with *d* > 0 MUST be sent with
> `SendInput`, all in one call:
>
> 1. *d* Backspace presses (`VK_BACK`, scan `0e`)
> 2. one `KEYEVENTF_UNICODE` press per UTF-16 unit of *s*
>
> Every injected event carries a fixed `dwExtraInfo` signature, which
> `GetMessageExtraInfo` reveals, so that the text service passes it through.
> Mixing the two paths would apply the insertion before the queued
> deletions. If `SendInput` reports fewer events than it was given, the
> context MUST be reset. (Verified in Win32 `EDIT` and RichEdit controls,
> 64- and 32-bit; unverified in AppContainers and consoles.)

> [spec:kbdgen:req:tsf.edit.cache]
> For contexts that are not authoritative, the text service MUST keep a
> per-context cache of at most `context_len` scalar values. The cache holds
> the scalar values its own edits left before the caret. Keys the text
> service eats are the only source of character input it does not see as
> text, so the cache is complete until a reset (`tsf.edit.reset`). It is
> discarded with the context. The cache MUST be passed to the engine with
> `authoritative` and `at_start` false.

> [spec:kbdgen:req:tsf.edit.reset+1]
> The text service MUST reset the engine state to `State::default()`, and
> clear the cache, at each event of `ldml.engine.state`:
>
> - the focused document or context changes (`OnSetFocus` of either sink,
>   context push or pop)
> - `ITfTextEditSink::OnEndEdit` reports a text or selection change that is
>   not its own (`tsf.edit.own`), as after a mouse click or another input
>   method
> - the keyboard-disabled or empty-context compartment changes, or TSF
>   switches profile
> - a key passes other than a lone modifier
> - the application terminates its composition
>
> A reset ends any composition by leaving its text committed, which equals
> `Commit`'s output (`ldml.engine.preedit`). Where no preedit was shown
> (`tsf.edit.preedit`), the text service MUST first apply the edit of a
> `Commit` event (`ldml.engine.commit`) before passing a key other than a
> lone modifier.

> [spec:kbdgen:req:tsf.edit.own]
> A change that `OnEndEdit` reports is the text service's *own*, and does
> not reset, when:
>
> - it is reported while the text service requests its own edit session
> - the text service injected input (`tsf.edit.inject`) since the last key
>   down it did not inject: the application's handling of that input
>   reaches TSF as the application's edit
> - it echoes a `SetText` at the selection in a context that is not
>   authoritative. Win32 `EDIT` and RichEdit report one further change as
>   the application's after each such edit session. The text service counts
>   these edits and ignores as many reports, until the next key down it did
>   not inject.
>
> (Verified in `EDIT` and RichEdit.)

> [spec:kbdgen:req:tsf.edit.preedit+1]
> A non-empty preedit MUST be shown as a composition, started with
> `ITfContextComposition::StartComposition` after the edit's committed
> text, or at the caret. It is decorated with the provider's single display
> attribute, a dotted underline, and replaced in place on each edit. An
> empty preedit ends the composition and removes its text. Committed text of
> an edit goes before the composition range. TSF may add text set at the
> composition's start to the composition, so before updating or ending it
> the text service MUST move the composition's start (`ShiftStart`) past
> the text it just committed. If `OnCompositionTerminated` arrives, the
> preedit text stays as committed, undecorated, and the engine is reset. In
> a context that is not authoritative (`tsf.edit.session`), the preedit is
> not shown. (Verified in a WPF `TextBox`.)

## Security contexts

> [spec:kbdgen:req:tsf.security.disabled+1]
> A context with `GUID_COMPARTMENT_KEYBOARD_DISABLED` or
> `GUID_COMPARTMENT_EMPTYCONTEXT` set (for example a password field) MUST
> still get the engine's mapping. A password must type the same in every
> field, and passing keys would hand them to the dummy US layout. In such a
> context the text service MUST NOT:
>
> - show a preedit (*p* is dropped)
> - keep a cache
>
> If edit sessions fail there, `tsf.edit.inject` applies. (Unverified:
> Chromium's password-field behaviour.)

> [spec:kbdgen:req:tsf.security.secure-mode+1]
> The text service MUST register `GUID_TFCAT_TIPCAP_SECUREMODE`. Under
> `TF_TMF_SECUREMODE`, as on the welcome screen, UAC prompts and
> credential UI, it MUST NOT:
>
> - create windows
> - read anything but its own image and the layout DLL resource
> - emit diagnostics
>
> Key mapping still runs. (Unverified: whether LogonUI on Windows 11 loads
> third-party TIPs. The installer keeps the layout DLL as the welcome-screen
> input method in any case: `tsf.register.welcome`.)

> [spec:kbdgen:req:tsf.security.appcontainer+2]
> The text service MUST register `GUID_TFCAT_TIPCAP_IMMERSIVESUPPORT`. Store
> apps and AppContainer processes load it, so every file it reads (its DLLs)
> MUST lie under the 64-bit `%ProgramFiles%`, also for the x86 DLL. Each
> such file MUST grant read and execute to `ALL APPLICATION PACKAGES`
> (`S-1-15-2-1`) and `ALL RESTRICTED APPLICATION PACKAGES` (`S-1-15-2-2`).
> `DllRegisterServer` MUST verify both, from the file's DACL, for every
> file the registration loads (`tsf.arch.registration`), and otherwise fail
> and register nothing. The installer's check after copying is that
> registration succeeds. (Verified on x64: files inheriting the
> `%ProgramFiles%` ACL pass, a file without the restricted packages' entry
> fails. Unverified: loading in an AppContainer process.)

> [spec:kbdgen:sem:tsf.security.integrity]
> The text service runs inside each host process at that process's
> integrity level, elevated processes included. It sends input only to its
> own process (`tsf.edit.inject`) and talks to no other process
> (`tsf.component.self-contained`). User Interface Privilege Isolation
> therefore never blocks it, and it gives a lower-integrity process no
> channel into a higher one.

> [spec:kbdgen:req:tsf.security.signing+1]
> Every PE file of the text service MUST carry an Authenticode signature
> with Divvun's code-signing certificate before release. That covers the
> three DLLs and the Arm64X forwarder. Microsoft requires third-party IMEs to
> be signed for Store apps. kbdgen holds no certificate: the release
> pipeline (divvun-actions) signs. `kbdgen tsf` MUST write exactly these
> four files to its output directory, unsigned, and only once all four pass
> their checks (`tsf.arch.builds`, `tsf.arch.arm64x`). It MUST print each
> one's absolute path on its own line to stdout: the list of files to sign.
> (Unverified: whether Windows 11 refuses to load an unsigned TIP.)

## Architectures

> [spec:kbdgen:req:tsf.arch.builds+1]
> The text service MUST be built as three DLLs, each the `kbd_tsf.dll` that
> cargo builds from `crates/kbd-tsf` for one target, renamed:
>
> | DLL | Target | Process |
> |---|---|---|
> | `divvun_tip_x86.dll` | `i686-pc-windows-msvc` | 32-bit, on any Windows |
> | `divvun_tip_x64.dll` | `x86_64-pc-windows-msvc` | x64, including emulated x64 on Arm |
> | `divvun_tip_arm64.dll` | `aarch64-pc-windows-msvc` | native Arm64 |
>
> `kbdgen tsf` builds them (`cli.commands`). After the checks of
> `kbdl.build.toolchain`, it runs for each target, in the workspace and with
> the environment of `kbdl.build.environment`, `cargo rustc --package
> kbd-tsf --release --lib --target <triple> --target-dir <workspace>/target
> -- -C target-feature=+crt-static`. The C runtime is linked statically
> because the text service loads into every process, also on machines
> without the Visual C++ runtime. kbdgen MUST check each DLL: its machine,
> the DLL flag, exactly the four exports of `tsf.component.crate` with none
> forwarded, and no C runtime DLL (`vcruntime*`, `msvcp*`, `ucrtbase*`,
> `api-ms-win-crt-*`) among its imports. Unlike a layout DLL, the DLL links
> `std`, which needs the MSVC and Windows SDK libraries. It is therefore
> built on Windows. (Verified on Windows 11 x64 with rustc 1.98.1 and MSVC:
> all three build and pass the checks. Without `+crt-static` the x64 DLL
> imports `VCRUNTIME140.dll` and `api-ms-win-crt-*`; with it, only system
> DLLs. The x64 and x86 DLLs load in 64- and 32-bit processes, where
> `DllCanUnloadNow` returns `S_OK`. Unverified: loading the arm64 DLL.)
>
> On Windows on Arm, x64 processes MAY load plain x64 DLLs. An
> `arm64ec-pc-windows-msvc` build is therefore not needed. A merged Arm64X
> image from Rust is not used: such images crash in x64 processes
> (rust-lang/rust#145154).

> [spec:kbdgen:req:tsf.arch.arm64x+1]
> For Windows on Arm, the text service MUST also ship `divvun_tip.dll`, an
> Arm64X pure forwarder in the same directory as the three DLLs. Its native
> exports forward to `divvun_tip_arm64` and its EC exports to
> `divvun_tip_x64`, each export to its own name. `kbdgen tsf` links it with
> the toolchain's `rust-lld` (`kbdl.build.toolchain`): `rust-lld -flavor
> link -dll -noentry -machine:arm64x -defarm64native:<native.def>
> -def:<ec.def> <native.obj> <ec.obj> -out:divvun_tip.dll -brepro`. The
> objects are a native and an Arm64EC `_load_config_used`; the Arm64EC one
> also defines `__chpe_metadata`. kbdgen writes both objects itself, with
> the data, symbols and relocations of LLD's test inputs
> `lld/test/COFF/Inputs/loadconfig-arm64.s` and `loadconfig-arm64ec.s`, so
> no C compiler or assembler is needed. Without those objects the output is
> plain ARM64. kbdgen MUST check the forwarder:
>
> - an ARM64 DLL with no entry point and no imports
> - its native view forwards the four exports to `divvun_tip_arm64`
> - its ARM64X dynamic relocations give an x64 view with CHPE metadata
>   whose exports forward to `divvun_tip_x64`
>
> (Verified on macOS: the forwarder is byte-identical to one linked from
> the LLD inputs assembled by `llvm-mc`, and `llvm-readobj` shows
> `COFF-ARM64X`, CHPE metadata, Arm64X relocations and both forward tables.
> `kbdgen tsf` on Windows x64 links the same bytes. Unverified: loading on
> Windows on Arm.)

> [spec:kbdgen:req:tsf.arch.registration+1]
> `InprocServer32` of the CLSID MUST name:
>
> | Windows | 64-bit registry view | `WOW6432Node` view |
> |---|---|---|
> | x64 | `divvun_tip_x64.dll` | `divvun_tip_x86.dll` |
> | Arm64 | `divvun_tip.dll` (`tsf.arch.arm64x`) | `divvun_tip_x86.dll` |
> | x86 | `divvun_tip_x86.dll` | — |
>
> `ThreadingModel` is `Apartment`. On Arm64, native and x64 processes share
> one 64-bit `Classes` view, so one path must load in both. `CLSID` keys are
> redirected for 32-bit processes, but `SOFTWARE\Microsoft\CTF\TIP` is
> shared, so profiles and categories are registered once.
>
> `DllRegisterServer` of each DLL writes the row of its process's view: the
> x86 DLL its own path, the x64 DLL on x64 its own path, and the Arm64 or
> x64 DLL on Arm64 `divvun_tip.dll` in its own directory. Only the DLL that
> writes the 64-bit view, or the x86 DLL on x86 Windows, registers the
> categories (`tsf.register.server`). It MUST fail, registering nothing, if
> its file is not named as in `tsf.arch.builds` or a file that path loads is
> missing. (Verified on x64, including the shared `CTF\TIP` key: the x86
> DLL registers no category.)

## Registration

> [spec:kbdgen:def:tsf.register.ids]
> A layout's *profile GUID* is its product code: the UUIDv5 of the layout's
> `kbdId` (`kbdl.metadata.bundle` keyboard name) in the namespace
> UUIDv5(DNS, `divvun.no`). divvun-actions already derives it for `kbdi -g`.
> Its *LANGID* is the low word of the LCID that `kbdi` uses for the
> layout's KLID. For tags without a Windows locale, it is the transient
> LANGID that the user's language list assigns (`tsf.register.langid`). Its
> *TIP string* is `LLLL:{CLSID}{GUID}`, with `LLLL` the LANGID in four
> hexadecimal digits.

> [spec:kbdgen:req:tsf.register.server]
> The text service's installer, not `kbdi`, MUST:
>
> - register the CLSID (`tsf.arch.registration`)
> - with `ITfCategoryMgr::RegisterCategory`, register
>   `GUID_TFCAT_TIP_KEYBOARD`, `GUID_TFCAT_TIPCAP_IMMERSIVESUPPORT`,
>   `GUID_TFCAT_TIPCAP_SYSTRAYSUPPORT`, `GUID_TFCAT_TIPCAP_UIELEMENTENABLED`,
>   `GUID_TFCAT_TIPCAP_SECUREMODE` and `GUID_TFCAT_DISPLAYATTRIBUTEPROVIDER`
>
> It MUST NOT register `GUID_TFCAT_TIPCAP_COMLESS`, which is unverified. It
> registers no profiles. `DllRegisterServer` and `DllUnregisterServer` MUST
> perform exactly this registration and its removal. Its install is
> machine-wide and needs elevation.

> [spec:kbdgen:req:tsf.register.profile]
> `kbdi keyboard_install` MUST, after creating the KLID of a layout whose
> tag has a Windows locale, call
> `ITfInputProcessorProfileMgr::RegisterProfile` with these arguments:
>
> - the CLSID, the LANGID and the profile GUID
> - the display name (`-n`)
> - the system path of the layout DLL as icon file, icon index 0
> - `hklSubstitute` 0 and `dwPreferredLayout` 0
> - enabled by default, flags 0
>
> It MUST NOT write `CTF\TIP` keys directly. If the text service's CLSID is
> not registered, it MUST register no profile and fall back to enabling the
> layout (`tsf.register.enable`). A tag without a Windows locale has no
> LANGID until a user's language list assigns one, so its profile is
> registered by `kbdi keyboard_enable` (`tsf.register.langid`). (Verified
> with `hklSubstitute` 0 and the layout DLL as the icon file: the profile
> registers, activates and types in 64- and 32-bit processes.)

> [spec:kbdgen:req:tsf.register.langid]
> Profiles are stored machine-wide under a LANGID. For tags without a
> Windows locale, the LANGID is a transient value (`0x2000`, `0x2400`, …)
> assigned per user. `kbdi` MUST therefore register the profile when it
> enables the keyboard for a user. It uses the LANGID that `LcidFromBcp47`
> returns after the tag is in that user's language list, and registers the
> profile again if a later user's LANGID differs. It MUST NOT unregister
> profiles other users enabled. (Unverified: the cost of one profile under
> several transient LANGIDs. Two users mapping different languages to one
> transient LANGID is an open problem.)

> [spec:kbdgen:req:tsf.register.enable]
> `kbdi keyboard_enable` MUST add the layout's TIP string with
> `InstallLayoutOrTip(…, 0)`, not `LLLL:<KLID>`, so the user sees one entry
> per keyboard: the text service's profile. The layout stays registered
> machine-wide as the fallback, and is enabled for the user instead of the
> TIP string only when the text service is absent; it MUST NOT be enabled
> beside it. Afterwards the existing `ctfmon` refresh applies. `kbdi` MUST
> accept TIP strings wherever it lists or verifies inputs. (Verified:
> `InstallLayoutOrTip` with a TIP string adds it to the language's
> `InputMethodTips`, and the profile activates.)

> [spec:kbdgen:req:tsf.register.welcome]
> `kbdi` SHOULD offer to install the layout, not the TIP string, for the
> welcome screen (`ILOT_DEFUSER4`, which changes `.DEFAULT`, not the
> profile that new users copy). Sign-in passwords are then typed through the
> layout DLL, which `tsf.test.differential` keeps equal to the text service
> on every table-expressible key. `kbdi keyboard_uninstall` MUST remove it
> from the welcome screen again, with `ILOT_DEFUSER4 | ILOT_UNINSTALL`,
> before it removes the KLID. (Verified: `.DEFAULT`'s `Preload` gains the
> layout through a substitute and, for a LANGID it lacked, an entry for that
> LANGID; the removal restores both exactly. `C:\Users\Default\NTUSER.DAT`
> is unchanged. Unverified: typing at the welcome screen.)

> [spec:kbdgen:req:tsf.register.uninstall+1]
> `kbdi keyboard_uninstall` MUST, in this order:
>
> 1. remove the TIP string for the current user with `ILOT_UNINSTALL`
> 2. remove what Windows keeps of the profile for the current user: its key
>    under `HKCU\Software\Microsoft\CTF\TIP\{CLSID}\LanguageProfile`, which
>    `ILOT_UNINSTALL` leaves with `Enable` 0, and a language's keyboard
>    entries under `HKCU\Software\Microsoft\CTF\SortOrder\AssemblyItem`,
>    which activating the profile writes, when they name only the profile;
>    keys left empty go too
> 3. call `UnregisterProfile` for every LANGID under which it registered the
>    profile GUID
> 4. remove the KLID as today
>
> The text service's own uninstaller MUST run only when no language profile
> remains under its CLSID in `CTF\TIP`. Removing one keyboard therefore never
> breaks another. (Verified: without step 2 both keys outlive the keyboard;
> with it an install and uninstall restore the user's registry exactly.)

> [spec:kbdgen:req:tsf.register.upgrade+2]
> Every running process that has typed text keeps the text service loaded,
> Explorer and consoles included. Its installer MUST therefore:
>
> 1. install each version into its own directory
>    `%ProgramFiles%\Divvun\Text Service\<version>\`, where `<version>` is
>    the `kbd-tsf` package version (`tsf.component`)
> 2. point `InprocServer32` at the new version
> 3. delete older directories
>
> A file in use cannot be deleted but can be renamed. The installer and the
> uninstaller MUST move such a file out of its directory, on the same
> volume, and delete it from there at the next reboot. They MUST NOT
> schedule a version directory's own paths for deletion at reboot: had that
> version been installed again before then, the reboot would delete the
> DLLs it registered.
>
> It MUST NOT overwrite a DLL in place. `DllRegisterServer` MUST fail,
> registering nothing, unless its DLL lies in a directory named for its own
> `kbd-tsf` version, so registering a version's DLLs points
> `InprocServer32` at that version. `DllUnregisterServer` MUST remove
> nothing while `InprocServer32` names another path than its
> `DllRegisterServer` would write, so an older version's removal leaves a
> newer registration intact. (Verified: a loaded TIP DLL cannot be
> overwritten but can be renamed. Its `ActivateEx` ran in `explorer.exe` and
> `conhost.exe`. On x64, a DLL outside a version directory is refused, and
> another directory's `DllUnregisterServer` leaves the registration.)

## Installer

> [spec:kbdgen:req:tsf.installer.bundle+1]
> Every keyboard installer MUST embed one pinned, BLAKE3-checked release of
> the text service's installer, as divvun-actions embeds divvun-wind. It MUST
> run that installer silently before any `kbdi keyboard_install`. The text
> service installer upgrades but never downgrades an installed text service.
> It MUST run on every Windows the keyboard installer accepts
> (`tsf.installer.layout-dlls`). It is not gated like divvun-wind. Failure to
> install it MUST NOT fail the keyboard install, and the keyboard then falls
> back to its layout (`tsf.register.enable`). After its last
> `kbdi keyboard_uninstall`, the keyboard uninstaller MUST run the text
> service's uninstaller. That uninstaller refuses while any language profile
> remains under the CLSID (`tsf.register.uninstall`), so removing the last
> keyboard removes the text service.
>
> divvun-actions builds the text service installer in the kbdgen pipeline,
> with a version of its own for every development build
> (`tsf.register.upgrade`). Builds of main replace the `kbd-tsf-dev-latest`
> prerelease of divvun/kbdgen; a `kbd-tsf-v<version>` tag gets its own
> release. Keyboard builds embed the installer of `kbd-tsf-dev-latest`, as
> they do divvun-wind's `dev-latest`. (Verified on Windows 11 x64, with a
> test CLSID and two keyboard installers: the first installs and registers
> the text service, which types in 64- and 32-bit processes; the second,
> embedding an older text service, leaves the newer one registered and
> types too; removing the first leaves the text service for the second;
> removing the second removes it and restores the language list. An upgrade
> while the old DLL is loaded deletes it at restart. Unverified: Arm64, x86
> Windows, outto packages, signed builds.)

> [spec:kbdgen:req:tsf.installer.layout-dlls]
> The keyboard installer MUST place kbdgen's layout DLL variants unchanged:
>
> | Windows | System32 | SysWOW64 |
> |---|---|---|
> | x64 | `x64` | `wow64` |
> | Arm64 | `arm64` | `wow64` |
> | x86 | `x86` | — |
>
> A plain `x86` DLL in SysWOW64 does not load
> (`[spec:kbdgen:req:kbdl.wow64]`). The installer MUST run on Arm64 Windows
> 11. divvun-actions branch `keyboard-rust-layout-dlls` does this for the
> Inno installer, which refuses Arm64 Windows 10 because `kbdi` there needs
> x64 emulation. outto has no Arm64 architecture selector, so its package
> refuses Arm64 until outto gains one.

## Emoji keyboard

> [spec:kbdgen:req:tsf.test.emoji]
> The integration tests (`tsf.test.vm`) MUST type, through the text
> service, a fixture layout whose keys output emoji, and check the committed
> text in a Win32 `EDIT` and a WPF `TextBox`. It proves output is not limited
> to UCS-2 or to the 16 UTF-16 units of a layout DLL ligature. Its keys
> include at least:
>
> - `q` → 😀 (U+1F600), outside the BMP, so a surrogate pair
> - 👩🏽‍💻, an emoji ZWJ sequence with a skin-tone modifier
> - 👨‍👩‍👧‍👦 followed by 🏳️‍🌈, one output longer than 16 UTF-16 units
>
> Backspace after each MUST be checked against `ldml.engine.backspace`.

## Engine boundary

> [spec:kbdgen:def:tsf.engine.api]
> The text service's engine is `kbd-engine`, through the API of
> `ldml.engine.api` as bounded for TSF by `ldml.engine.tsf`. Per profile it
> decodes the model with `Model::from_bytes`, which uses the default
> options (`CancelOrPass`, `Nfc`; `ldml.engine.backspace.default`,
> `ldml.engine.output.form`). Per context it owns one `State` and calls
> `Model::key(&State, &Context, &KeyEvent)`. It sends only `Scan`,
> `Decimal`, `Backspace` (`tsf.keys.identity`) and `Commit`
> (`tsf.edit.reset`) keys, builds `Context` per `tsf.edit.session` and
> `tsf.edit.cache`, and applies `Edit` per `tsf.edit.ops`. Purity makes
> `OnTestKeyDown` exact (`tsf.keys.claim`).

> [spec:kbdgen:req:tsf.engine.contract]
> The text service MUST rely on `ldml.engine.contract` and MUST NOT
> second-guess the engine's choice to pass or consume a key. In particular:
>
> - Keyboards with normalization `Enabled` normalize
>   (`ldml.engine.output.form`); `Disabled` ones, every v4 default, change
>   no text.
> - Ctrl without Alt, Left Alt and Win chords pass
>   (`ldml.engine.shortcuts`); Ctrl held with AltGr reaches layers that
>   name both ctrl and alt.
> - A form key with no output is consumed; scan codes in no form row pass
>   (`ldml.engine.hardware`).
>
> `kbd-engine` MUST build for every target of `tsf.arch.builds`
> (`ldml.crate.targets`).

> [spec:kbdgen:def:tsf.engine.model+1]
> The *model* of a layout is the `ldml.model.encoding` (`DVKB` major 1) of
> the layout's `windows` keyboard, the same keyboard the `kbdl` adapter
> reads (`ldml.kbdl.model-resource`). A v3 layout gets it by in-memory
> migration. Of it, the text service uses the hardware set and its layers,
> the transforms, the flush outputs that form the preedit, and
> `windows` (`ldml.model.windows`). It does not use touch sets or displays.

## Testing

> [spec:kbdgen:req:tsf.test.engine]
> Engine behaviour MUST be tested on every host operating system without
> Windows, through `Model::key` alone (`ldml.test.harness`). Test cases are
> sequences of key events with the expected text before the caret and the
> expected preedit. kbdgen MUST also test, for every fixture bundle, that
> the resource of `tsf.data.resource` in each built `<name>.res` decodes
> with `Model::from_bytes` to the model of `ldml.kbdl.model-resource`.

> [spec:kbdgen:req:tsf.test.differential+1]
> For every fixture layout, a test MUST compare `kbd-engine` on the layout's
> model with the layout DLL built from it. For each table-expressible
> position, `kbdl.layers` layer and dead-key path, the engine's committed
> output MUST equal what `ToUnicodeEx` returns for the built DLL on Windows.
> The exceptions are the documented ones:
>
> - Caps+Shift on `CAPLOK` keys (`ldml.kbdl.caps`)
> - the `ctrl` layer, which the engine passes (`ldml.engine.shortcuts`)
> - what `ldml.kbdl.classify` reports: multi-unit dead-key leaves,
>   transforms that do not start with a marker, `reorder` and extra
>   `backspace` rules, mixed outputs, enabled normalization
> - positions `ldml.kbdl.positions` drops or warns about, and the space
>   bar outside the columns its fixed row fills
> - caps states of a dead key that `kbdl.caps.sgcaps` cannot express
> - a v3 layout's caps states at positions M09 lists for Windows
>   (`ldml.kbdl.model-resource`, `ldml.migrate.caps-diff`)
> - a dead key with no entry after a pending one (`ldml.kbdl.dead-tree`)
> - a ligature whose first unit composes with a pending dead key
>   (`kbdl.ligatures`)
>
> Each exception MUST be listed in the test.

> [spec:kbdgen:req:tsf.test.host]
> The planner, which turns an `Action` plus context flags into
> `tsf.edit.apply`, `tsf.edit.inject` or composition steps, MUST be a module
> with no TSF calls. It MUST be tested against a fake context on every host.
> Fault-injection tests MUST force a panic in each COM entry point and check
> that the call returns `E_UNEXPECTED` and that the service is then poisoned
> (`tsf.component.panic`).

> [spec:kbdgen:req:tsf.test.vm+1]
> Integration tests MUST run on a Windows 11 machine with an interactive
> session, launched through a scheduled task in the signed-in session. TSF
> needs a desktop, and the shared Server Core CI queue has none. A run:
>
> 1. registers the build under a test CLSID, with a test KLID, test layout
>    DLL and test LANGID
> 2. sends scan codes with `SendInput` into a Win32 `EDIT`, a RichEdit, a
>    WPF `TextBox` pumped by its dispatcher, a console, and the same from a
>    32-bit process
> 3. checks the resulting text
> 4. restores the user's language list and removes every registration
>
> `ActivateProfile` with `TF_IPPMF_FORPROCESS` switches the input method of
> the whole session, not only of the calling process. (Verified.) A run
> that activates the test profile MUST therefore activate the previously
> active profile again afterwards. Arm64 and Arm64X need a Windows on Arm
> machine.

> [spec:kbdgen:req:tsf.test.acceptance]
> An acceptance test MUST install a keyboard on a Windows 11 machine with an
> interactive session as a user gets it, and type through the installed text
> service. A run:
>
> 1. builds the text service (`kbdgen tsf`) and `kbdi` under a test CLSID,
>    the layout DLLs of a fixture bundle, and the text service and keyboard
>    installers that divvun-actions generates, under test AppIds
> 2. records the user's languages and inputs, the user's and the welcome
>    screen's keyboard registry, the text service's registration, the
>    installed KLIDs, both uninstall entries and the installed files
> 3. runs the keyboard installer silently, which runs the text service's
>    installer and `kbdi keyboard_install -e`
> 4. types every case with `SendInput` through the profile `kbdi` enabled,
>    under the LANGID the user's input list gives it, into the controls of
>    `tsf.test.vm` from 64- and 32-bit processes; into a console switched to
>    the keyboard with Win+Space, as a user switches; and into an `EDIT`
>    with the layout DLL alone
> 5. uninstalls, installs again and uninstalls while a process holds the
>    text service's DLL loaded, as Explorer does, and records the state
>    again
>
> The fixture is a v4 layout derived from the golden Võro layout. Its cases
> MUST cover dead-key outputs of several UTF-16 units (combining sequences,
> a surrogate pair), a chain of three dead keys, two transform groups that
> rewrite text before the key and dead-key output, a dead key followed by an
> AltGr ligature, Backspace cancelling a pending dead key and a pending
> chain, and Backspace passing after transform and dead-key output. Each
> control MUST hold what `kbd-engine` types for the case on the model
> embedded in the built layout DLL (`tsf.data.resource`), following
> `tsf.keys.identity` and `tsf.edit.reset`; after a passed Backspace that
> erases part of a grapheme, what the control's own Backspace leaves. The
> layout DLL alone MUST type the engine's text where the tables can express
> the case, and other text where the case needs the text service. After the
> last uninstall every recorded item MUST equal its state before, and
> neither install may find the DLLs it registers scheduled for deletion at
> the next reboot (`tsf.register.upgrade`).
>
> The sign-in screen and the secure desktop, where the layout DLL is the
> input method (`tsf.pairing.self-sufficient`), take no `SendInput` from the
> session, so the layout DLL alone is typed in an ordinary `EDIT`.
> (Verified: Windows 11's console host runs the text service once the
> keyboard is selected in it; a console started while another process
> activated the profile keeps the session's previous input.)
