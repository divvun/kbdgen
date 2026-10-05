# Windows text service (TSF)

On Windows a Divvun keyboard has two layers. The base is the `KBDTABLES`
layout DLL of `docs/spec/kbdl.md`. It works wherever Windows reads keyboard
layouts: the sign-in screen, the secure desktop, elevated processes, consoles
and games. On top of it sits a Text Services Framework *text input
processor* (TIP), the *text service*. It does what tables cannot: dead-key
and transform output of any length, deep chains, rules on visible text, a
visible pending state, and an emoji picker. The text service hosts a keyboard
*engine*. Its long-term engine is Divvun's own Rust implementation of CLDR
LDML Keyboard 3.0 (UTS #35 Part 7), shared across operating systems. Until
that exists, an interim engine built from the `kbdl.input` model fills the
same interface (`tsf.engine.interim`). Where targets disagree, keyboard
semantics follow macOS.

These rules cover:

- the component shape
- keyboard data
- how the text service relates to the layout DLL
- key handling and the edit operations
- security contexts
- architectures and registration
- installer and `kbdi` responsibilities
- the emoji picker
- the engine boundary
- testing

Rules naming `kbdi` or the keyboard installer bind those projects
(github.com/divvun/kbdi, divvun-actions `actions/keyboard/build`), not
kbdgen.

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
- rustc platform support for `arm64ec-pc-windows-msvc` (Tier 2 since 1.81),
  and rust-lang/rust#145154, the broken merged Arm64X Rust DLLs
- windows-rs 0.62.2 (`Win32_UI_TextServices`, `windows_core::implement`)
- chewing/windows-chewing-tsf, a Rust TIP that uses an Arm64X pure forwarder
- The Rust 1.81 release notes on aborting at non-unwind ABIs:
  <https://blog.rust-lang.org/2024/09/05/Rust-1.81.0/>
- Unicode CLDR `common/annotations` and `common/annotationsDerived`, and
  Unicode `emoji-test.txt`
- divvun/kbdi `main` (`src/keyboard.rs`, `src/keyboard_win8.rs`) and
  divvun-actions `actions/keyboard/build/{mod,iss,outto,wind}.ts`

`divvun-wind` is unrelated to this layer. It is a per-session daemon that
injects a DLL into Explorer to relabel keyboards, and it runs only on
Windows 11 x64. It is not a TSF component, so the text service does not build
on it. The keyboard installer bundles the text service the same way it
bundles divvun-wind (`tsf.installer.bundle`).

## Component

> [spec:kbdgen:def:tsf.component]
> The *text service* is one in-process COM server. Every Divvun keyboard
> shares it: there is one CLSID, `{5E668C8A-2FB8-41D2-90B1-9C132653FA9D}`,
> and one DLL per architecture (`tsf.arch.builds`). Each installed layout
> adds a TSF language profile to it (`tsf.register.profile`) and supplies
> its keyboard data (`tsf.data.resource`). The text service is a separate
> product, not part of kbdgen or divvun-wind. kbdgen produces only the
> per-keyboard data. A shared binary keeps one copy loaded per process and
> one place to fix bugs. The cost is that keyboard data and engine versions
> must stay compatible (`tsf.data.version`).

> [spec:kbdgen:req:tsf.component.crate]
> The text service MUST be a Rust `cdylib` that uses `windows` and
> `windows-core` 0.62.x and implements COM interfaces with
> `windows_core::implement`. It MUST export exactly `DllGetClassObject`,
> `DllCanUnloadNow`, `DllRegisterServer` and `DllUnregisterServer`.
> `DllGetClassObject` MUST return `CLASS_E_CLASSNOTAVAILABLE` for any other
> CLSID. `DllCanUnloadNow` MUST return `S_OK` only when no object and no
> server lock is alive. (Verified: a crate of this shape loads in x64 and
> x86 processes, activates and edits text.)

> [spec:kbdgen:req:tsf.component.interfaces]
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
>
> The emoji picker additionally implements `ITfCandidateListUIElement`
> (`tsf.emoji.ui`). `ActivateEx` MUST record the activation flags from
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
> For each layout, kbdgen MUST put the serialized engine model
> (`tsf.engine.model`) in the layout DLL. It goes in an `RT_RCDATA` (10)
> resource with id 1 and language 0 in `<name>.res`, written as in
> `[spec:kbdgen:syn:kbdl.resources.format]` before the string tables. Every
> variant built by `kbdl.build` therefore carries identical data. This keeps
> the text service's data in step with the table fallback and readable
> wherever the layout DLL is, because System32 and SysWOW64 are readable from
> AppContainers. (Unverified: that `LoadKeyboardLayout` still accepts a
> layout DLL carrying this resource.)

> [spec:kbdgen:req:tsf.data.locate]
> On activation for a profile, the text service MUST find the layout under
> `HKLM\SYSTEM\CurrentControlSet\Control\Keyboard Layouts` whose
> `Layout Product Code` equals the profile GUID. The comparison ignores
> case and surrounding braces, because the legacy installer writes the code
> without a closing brace. The service MUST then load that key's
> `Layout File` from the system directory with `LoadLibraryExW(…,
> LOAD_LIBRARY_AS_DATAFILE | LOAD_LIBRARY_AS_IMAGE_RESOURCE)` and read the
> resource of `tsf.data.resource`. Results are cached per process. If any
> step fails, the profile is inert (it eats no keys). (Unverified: that this
> key is readable from AppContainers.)

> [spec:kbdgen:req:tsf.data.version]
> The model MUST begin with the ASCII magic `DVKB`, then a u16 major and a
> u16 minor format version, little-endian. The text service MUST refuse a
> model whose major version it does not implement, and the profile is then
> inert. It MUST accept a higher minor version. A minor version only appends
> fields that older readers can ignore. The keyboard installer installs the
> text service it bundles before registering layouts
> (`tsf.installer.bundle`), so a keyboard never runs against an older
> service than the one it was built for.

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
> - when its profile is not selected
> - at the welcome screen (`tsf.register.welcome`)
> - when the text service is poisoned or inert
>
> Its `kbdl` output for table-expressible keys equals the engine's output
> (`tsf.test.differential`).

> [spec:kbdgen:sem:tsf.pairing.alternatives]
> Two other pairings were tried. Neither is used.
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

> [spec:kbdgen:req:tsf.keys.identity]
> The text service MUST identify a key by its scan code and extended flag
> (`lParam` bits 16–23 and 24), never by its virtual key. The virtual key
> depends on the HKL beneath, which is not the layout
> (`tsf.pairing.substitute`). Scan codes map to positions as in
> `[spec:kbdgen:req:kbdl.scancodes.iso]`. It also recognises space
> (`39`), numpad decimal (`53`, not extended), Backspace (`0e`) and the
> physical keys of extra modifiers (`kbdl.scancodes.extra-modifiers`). It
> reads modifier and Caps Lock state with `GetKeyState` while handling the
> event. `VK_PACKET`, `VK_PROCESSKEY` and the service's own injected input
> (`tsf.edit.inject`) are always passed through.

> [spec:kbdgen:req:tsf.keys.claim]
> A key down is eaten exactly when the engine's action for it is not *pass*
> (`tsf.engine.api`). `OnTestKeyDown` MUST compute that action without
> changing engine state, and `OnKeyDown` MUST commit the same action. A key
> up MUST be eaten exactly when its key down was eaten. Auto-repeat is
> processed like a fresh key down. While a context is read-only or the
> service is poisoned or inert, every key passes. Keys not in the model pass
> and reset the context (`tsf.edit.reset`). Shift, Ctrl, Alt, Win and Caps
> Lock alone never reset it.

> [spec:kbdgen:req:tsf.keys.altgr]
> If the model has any `alt` or `alt+shift` value, the text service MUST
> treat Right Alt as AltGr. It MUST eat Right Alt's own key down and key up,
> so the application never sees a lone Alt and does not open its menu bar.
> If the HKL beneath synthesises Left Ctrl with Right Alt, that Ctrl MUST NOT
> count as Ctrl. Left Alt chords always pass. (Unverified: the menu-bar
> behaviour beneath the US dummy layout, and the use of Caps Lock as an extra
> modifier while the system still toggles its lock state.)

> [spec:kbdgen:req:tsf.keys.locale-flags]
> When the model's `lrmRlm` flag is set (`kbdl.locale`), the text service
> MUST make left Shift+Backspace insert U+200E and right Shift+Backspace
> insert U+200F. When `shiftLock` is set, it SHOULD treat the Caps Lock
> toggle as Shift Lock, where Shift releases it. It cannot clear the system
> toggle, so where that diverges the indicator light may disagree.
> (Unverified.)

## Edit operations

> [spec:kbdgen:def:tsf.edit.ops]
> An *edit* is a triple (*d*, *s*, *p*):
>
> - *d*: the number of Unicode scalar values to delete before the caret
> - *s*: a string to insert at the caret as committed text
> - *p*: the new preedit, shown after the caret as an uncommitted
>   composition (empty for none)
>
> They are applied in that order. *d* never exceeds the scalar values of the
> context passed to the engine (`tsf.engine.contract`).

> [spec:kbdgen:thm:tsf.edit.units]
> The number of UTF-16 units to delete is well defined. It is the UTF-16
> length of the last *d* scalar values of the context string the text
> service passed to the engine. By `tsf.engine.contract`, *d* is at most that
> context's scalar count. The context ends at the caret, so those units are
> exactly the units before the caret. The text service therefore never
> splits a surrogate pair while the context is authoritative.

> [spec:kbdgen:req:tsf.edit.session]
> For each eaten key down, the text service MUST request one
> `TF_ES_SYNC | TF_ES_READWRITE` edit session. In it, it reads the context:
> it clones the default selection, collapses the clone to its start, shifts
> the start back by up to the engine's context length, and calls `GetText`.
> The context is *authoritative* when the read succeeds and the context's
> static flags lack `TS_SS_TRANSITORY`. Otherwise the cache of
> `tsf.edit.cache` is used. A non-empty selection resets the engine first.
> (Verified: transitory contexts return no text before the caret; a WPF
> `TextBox` returns it.)

> [spec:kbdgen:req:tsf.edit.apply]
> In an authoritative context, the edit MUST be applied in the edit session:
>
> 1. Clone the selection range and shift its start back by the units of
>    `tsf.edit.units`.
> 2. `SetText` it to *s*, which also replaces any selection.
> 3. Collapse the range to its end and make it the selection.
> 4. Update the composition to *p* (`tsf.edit.preedit`).
>
> (Verified in a WPF `TextBox`: `a` then a key mapped to "delete 1, insert
> `a` U+0308 U+0303" leaves exactly those three units.)

> [spec:kbdgen:req:tsf.edit.inject]
> In a transitory or unreadable context, an edit with *d* = 0 MUST be
> applied with `SetText` at the selection. An edit with *d* > 0 MUST instead
> be sent with `SendInput`, all in one call:
>
> 1. *d* Backspace presses (`VK_BACK`, scan `0e`)
> 2. one `KEYEVENTF_UNICODE` press per UTF-16 unit of *s*
>
> Every injected event carries a fixed `dwExtraInfo` signature, which
> `GetMessageExtraInfo` reveals, so that the text service passes it through.
> Mixing the two paths would apply the insertion before the queued
> deletions. If `SendInput` reports fewer events than it was given, the
> context MUST be reset. (Verified in a Win32 `EDIT` control; unverified in
> AppContainers and consoles.)

> [spec:kbdgen:req:tsf.edit.cache]
> For contexts that are not authoritative, the text service MUST keep a
> per-context cache of at most the engine's context length. The cache holds
> the scalar values its own edits left before the caret. Keys the text
> service eats are the only source of character input it does not see as
> text, so the cache is complete until a reset (`tsf.edit.reset`). It is
> discarded with the context. The cache MUST be passed to the engine marked
> not authoritative.

> [spec:kbdgen:req:tsf.edit.reset]
> The text service MUST reset the engine state and the cache when:
>
> - the focused document or context changes (`OnSetFocus`, context push
>   or pop)
> - `ITfTextEditSink::OnEndEdit` reports a text or selection change from an
>   edit session other than its own, as after a mouse click or another input
>   method
> - a key passes other than a lone modifier
> - the application terminates its composition
>
> A reset ends any composition by leaving its text committed.

> [spec:kbdgen:req:tsf.edit.preedit]
> A non-empty preedit MUST be shown as a composition, started with
> `ITfContextComposition::StartComposition` at the caret. It is decorated
> with the provider's single display attribute, a dotted underline, and
> replaced in place on each edit. An empty preedit ends the composition.
> Committed text of an edit goes before the composition range. If
> `OnCompositionTerminated` arrives, the preedit text stays as committed and
> the engine is reset. Where no edit session is possible
> (`tsf.edit.inject`), the preedit is not shown. (Unverified for transitory
> contexts.)

## Security contexts

> [spec:kbdgen:req:tsf.security.disabled]
> A context with `GUID_COMPARTMENT_KEYBOARD_DISABLED` or
> `GUID_COMPARTMENT_EMPTYCONTEXT` set (for example a password field) MUST
> still get the engine's mapping. A password must type the same in every
> field, and passing keys would hand them to the dummy US layout. In such a
> context the text service MUST NOT:
>
> - show a preedit (*p* is dropped)
> - open the emoji picker
> - keep a cache
>
> If edit sessions fail there, `tsf.edit.inject` applies. (Unverified:
> Chromium's password-field behaviour.)

> [spec:kbdgen:req:tsf.security.secure-mode]
> The text service MUST register `GUID_TFCAT_TIPCAP_SECUREMODE`. Under
> `TF_TMF_SECUREMODE`, as on the welcome screen, UAC prompts and
> credential UI, it MUST NOT:
>
> - create windows
> - open the emoji picker
> - read anything but its own image and the layout DLL resource
> - emit diagnostics
>
> Key mapping still runs. (Unverified: whether LogonUI on Windows 11 loads
> third-party TIPs. The installer keeps the layout DLL as the welcome-screen
> input method in any case: `tsf.register.welcome`.)

> [spec:kbdgen:req:tsf.security.appcontainer]
> The text service MUST register `GUID_TFCAT_TIPCAP_IMMERSIVESUPPORT`. Store
> apps and AppContainer processes load it, so every file it reads (its
> DLLs, emoji data) MUST lie under `%ProgramFiles%`. Each such file MUST
> grant read and execute to `ALL APPLICATION PACKAGES` (`S-1-15-2-1`) and
> `ALL RESTRICTED APPLICATION PACKAGES` (`S-1-15-2-2`), and its installer MUST
> verify that after copying. Under `TF_TMF_IMMERSIVEMODE` its windows follow
> `tsf.emoji.ui`. (Unverified: loading in an AppContainer process.)

> [spec:kbdgen:sem:tsf.security.integrity]
> The text service runs inside each host process at that process's
> integrity level, elevated processes included. It sends input only to its
> own process (`tsf.edit.inject`) and talks to no other process
> (`tsf.component.self-contained`). User Interface Privilege Isolation
> therefore never blocks it, and it gives a lower-integrity process no
> channel into a higher one.

> [spec:kbdgen:req:tsf.security.signing]
> Every PE file of the text service MUST carry an Authenticode signature
> with Divvun's code-signing certificate before release. That covers the
> three DLLs and the Arm64X forwarder. Microsoft requires third-party IMEs to
> be signed for Store apps. (Unverified: whether Windows 11 refuses to load
> an unsigned TIP.)

## Architectures

> [spec:kbdgen:req:tsf.arch.builds]
> The text service MUST be built as three DLLs, named after the cargo
> target they come from:
>
> | DLL | Target | Process |
> |---|---|---|
> | `divvun_tip_x86.dll` | `i686-pc-windows-msvc` | 32-bit, on any Windows |
> | `divvun_tip_x64.dll` | `x86_64-pc-windows-msvc` | x64, including emulated x64 on Arm |
> | `divvun_tip_arm64.dll` | `aarch64-pc-windows-msvc` | native Arm64 |
>
> On Windows on Arm, x64 processes MAY load plain x64 DLLs. An
> `arm64ec-pc-windows-msvc` build is therefore not needed. A merged Arm64X
> image from Rust is not used: such images crash in x64 processes
> (rust-lang/rust#145154).

> [spec:kbdgen:req:tsf.arch.arm64x]
> For Windows on Arm, the text service MUST also ship `divvun_tip.dll`, an
> Arm64X pure forwarder in the same directory as the three DLLs. Its native
> exports forward to `divvun_tip_arm64` and its EC exports to
> `divvun_tip_x64`. It is linked with `rust-lld -flavor link -dll -noentry
> -machine:arm64x -defarm64native:<native.def> -def:<ec.def>` plus a native
> and an Arm64EC `_load_config_used` object. Without those objects the
> output is plain ARM64. (Verified on macOS: `llvm-readobj` shows
> `COFF-ARM64X`, CHPE metadata, Arm64X relocations and both forward tables.
> Unverified: loading on Windows on Arm.)

> [spec:kbdgen:req:tsf.arch.registration]
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
> shared, so profiles and categories are registered once. (Verified on x64,
> including the shared `CTF\TIP` key.)

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
> `kbdi keyboard_install` MUST, after creating the layout's KLID, call
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
> layout (`tsf.register.enable`). (Verified with `hklSubstitute` 0 and the
> TIP DLL as the icon file: the profile registers and activates.)

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
> per keyboard. The layout stays registered machine-wide and is enabled for
> the user only when the text service is absent or the user asks for the
> basic layout. Afterwards the existing `ctfmon` refresh applies. `kbdi` MUST
> accept TIP strings wherever it lists or verifies inputs. (Verified:
> `InstallLayoutOrTip` with a TIP string adds it to the language's
> `InputMethodTips`, and the profile activates.)

> [spec:kbdgen:req:tsf.register.welcome]
> `kbdi` SHOULD offer to install the layout, not the TIP string, for the
> welcome screen and new users (`ILOT_DEFUSER4`). Sign-in passwords are then
> typed through the layout DLL, which `tsf.test.differential` keeps equal to
> the text service on every table-expressible key.

> [spec:kbdgen:req:tsf.register.uninstall]
> `kbdi keyboard_uninstall` MUST, in this order:
>
> 1. remove the TIP string for the current user with `ILOT_UNINSTALL`
> 2. call `UnregisterProfile` for every LANGID under which it registered the
>    profile GUID
> 3. remove the KLID as today
>
> The text service's own uninstaller MUST run only when no language profile
> remains under its CLSID in `CTF\TIP`. Removing one keyboard therefore never
> breaks another.

> [spec:kbdgen:req:tsf.register.upgrade]
> Every running process that has typed text keeps the text service loaded,
> Explorer and consoles included. Its installer MUST therefore:
>
> 1. install each version into its own directory
>    `%ProgramFiles%\Divvun\Text Service\<version>\`
> 2. point `InprocServer32` at the new version
> 3. delete older directories only when no file in them is in use, otherwise
>    at the next install or reboot
>
> It MUST NOT overwrite a DLL in place. (Verified: a loaded TIP DLL cannot be
> overwritten but can be renamed. Its `ActivateEx` ran in `explorer.exe` and
> `conhost.exe`.)

## Installer

> [spec:kbdgen:req:tsf.installer.bundle]
> Every keyboard installer MUST embed one pinned, BLAKE3-checked release of
> the text service's installer, as divvun-actions embeds divvun-wind. It MUST
> run that installer silently before any `kbdi keyboard_install`. The text
> service installer upgrades but never downgrades an installed text service.
> It MUST run on x86, x64 and Arm64 Windows 10 and 11. It is not gated like
> divvun-wind. Failure to install it MUST NOT fail the keyboard install, and
> the keyboard then falls back to its layout (`tsf.register.enable`).

> [spec:kbdgen:req:tsf.installer.layout-dlls]
> The keyboard installer MUST place kbdgen's layout DLL variants:
>
> | Windows | System32 | SysWOW64 |
> |---|---|---|
> | x64 | `x64` | `wow64` |
> | Arm64 | `arm64` | `wow64` |
> | x86 | `x86` | — |
>
> It MUST place them unchanged. A plain `x86` DLL in SysWOW64 does not load
> (`[spec:kbdgen:req:kbdl.wow64]`). The installer MUST run on Arm64 Windows.
> Today divvun-actions does neither:
>
> - `createArchitectureDirectories` copies `x86` over `wow64`
> - the `arm64` build is ignored
> - Inno refuses Arm64
> - outto skips files on Arm64 but still runs `kbdi`

## Emoji picker

> [spec:kbdgen:def:tsf.emoji.picker]
> The *emoji picker* is a window that the text service hosts. It lists
> emoji by Unicode `emoji-test.txt` group and subgroup, showing
> fully-qualified sequences only. It filters them by keyword search in the
> keyboard's language (`tsf.emoji.data`) and remembers recently used emoji
> per process. Choosing an emoji commits it (`tsf.emoji.commit`). Windows'
> own panel (Win+. and Win+;) has no documented extension point and is left
> untouched. The text service never eats Win chords, so that panel keeps
> working alongside it.

> [spec:kbdgen:req:tsf.emoji.trigger]
> The picker MUST open from a preserved key, registered with
> `ITfKeystrokeMgr::PreserveKey` while the profile is active. The model
> names it, and none is registered if the model names none. It MUST NOT
> open in secure mode or in disabled contexts
> (`tsf.security.disabled`). While the picker is open, the text service
> eats keys:
>
> - letters edit the search; it MAY type them through the engine
> - arrows move the selection
> - Enter commits
> - Escape closes the picker
>
> Opening the picker first resets the engine (`tsf.edit.reset`), which
> commits any preedit and drops a pending dead key.

> [spec:kbdgen:req:tsf.emoji.ui]
> The picker MUST be published as an `ITfCandidateListUIElement` through
> `ITfUIElementMgr::BeginUIElement`, `UpdateUIElement` and `EndUIElement`.
> It draws its own window only when the application leaves `pbShow` true,
> so UI-less applications such as games can render it themselves. Its window
> is:
>
> - a `WS_POPUP` with `WS_EX_TOOLWINDOW | WS_EX_TOPMOST`
> - owned by the window of `ITfContextView::GetWnd`, or `GetFocus` as a
>   fallback
> - placed from `GetTextExt` of the selection
> - given the UIA AutomationId `IME_Candidate_Window`
> - announced with `EVENT_OBJECT_IME_SHOW`, `EVENT_OBJECT_IME_HIDE` and
>   `EVENT_OBJECT_IME_CHANGE`
>
> It declares no DPI awareness of its own.

> [spec:kbdgen:req:tsf.emoji.data]
> Search keywords and names come from these sources, merged in this order:
>
> 1. annotations that the bundle supplies for the layout's language
> 2. CLDR `annotations` and `annotationsDerived` for that language and its
>    CLDR parents
> 3. the same for each Windows UI language of the user
> 4. English
>
> CLDR has no annotations for most Divvun languages, Sámi included (it has
> `fo`, `kl`, `fi`), so the first source matters. CLDR data MUST be compiled
> into per-locale files in the text service's install directory. Bundle
> annotations travel in the model (`tsf.engine.model`). No data is fetched
> at runtime.

> [spec:kbdgen:req:tsf.emoji.commit]
> Committing an emoji MUST insert it through `tsf.edit.apply` or
> `tsf.edit.inject` as an edit (0, emoji, empty), then reset the engine. In a
> UI-less or immersive host where the picker cannot be shown, the preserved
> key does nothing.

## Engine boundary

> [spec:kbdgen:def:tsf.engine.api]
> The engine crate exposes:
>
> - `Model::from_bytes(&[u8]) -> Result<Model, Error>`
> - `Model::context_len() -> usize`, at most 64
> - `Model::preserved_keys()`
> - a value type `State`, where `State::default()` is the reset state
> - the pure function `Model::key(&State, &Context, KeyEvent) -> (Action,
>   State)`
>
> The types are:
>
> - `KeyEvent` is (position or space, decimal or Backspace; Shift; Caps
>   toggle; Ctrl; Left Alt; AltGr; extra modifiers; repeat).
> - `Context` is (scalar values before the caret, authoritative flag).
> - `Action` is either *pass* or an edit (`tsf.edit.ops`).
>
> Hosts own the state. Purity makes `OnTestKeyDown` exact
> (`tsf.keys.claim`).

> [spec:kbdgen:req:tsf.engine.contract]
> The engine:
>
> - MUST be deterministic and MUST NOT panic, perform I/O or read the clock
> - MUST bound its work per key by the model size and `context_len`
> - MUST return a delete count no larger than the context's scalar count,
>   and no preedit unless the model asks for one
> - MUST NOT normalize text
> - MUST pass any event with Ctrl (but not AltGr), Left Alt, or a key the
>   model does not define, so shortcuts reach the application
> - MUST handle a non-authoritative context exactly like an authoritative
>   one; the flag only informs diagnostics
>
> The same crate MUST build for every target of `tsf.arch.builds` and for the
> other operating-system hosts.

> [spec:kbdgen:def:tsf.engine.model]
> The *model* is the engine's compiled input for one layout, preceded by the
> header of `tsf.data.version`. Major version 1 is the interim model. It
> holds `kbdl.input` resolved at build time:
>
> - for every layer of `kbdl.layers`, the 49 position values with dead
>   flags, space and the decimal separator
> - the dead-key tree, at any depth
> - the extra modifiers, `shiftLock` and `lrmRlm`
> - the preserved emoji key and the bundle's emoji annotations
>
> The LDML engine defines major version 2 or later.

> [spec:kbdgen:req:tsf.engine.interim]
> The interim engine is defined as follows.
>
> - A key's value for the current modifiers is inserted unless it is dead.
> - A dead value enters its tree state. Each further key follows the branch,
>   to any depth, and a leaf inserts its whole output.
> - A key with no entry inserts the state's standalone output followed by
>   the key's own value.
> - Backspace with a pending state cancels only that state. Otherwise
>   Backspace passes.
> - Caps+Shift on a letter selects `shift`, as on macOS; `caps+shift`
>   applies only when the layout defines it.
>
> A pending state SHOULD be shown as the preedit of its standalone output,
> as macOS shows marked text. Its context is unused.

## Testing

> [spec:kbdgen:req:tsf.test.engine]
> Engine behaviour MUST be tested on every host operating system without
> Windows, through `Model::key` alone. Test cases are sequences of key events
> with the expected text before the caret and the expected preedit. kbdgen
> MUST also test that the model it embeds (`tsf.data.resource`) round-trips
> through `Model::from_bytes` for every fixture bundle.

> [spec:kbdgen:req:tsf.test.differential]
> For every fixture layout, a test MUST compare the interim engine and the
> layout DLL. For each table-expressible position, layer and dead-key path,
> the engine's committed output MUST equal what `ToUnicodeEx` returns for the
> built layout DLL on Windows. The exceptions are the documented ones:
>
> - multi-unit dead-key leaves, which the table omits
> - Caps+Shift on `CAPLOK` keys (`tsf.engine.interim` against
>   `[spec:kbdgen:sem:kbdl.caps]`)
>
> Each exception MUST be listed in the test.

> [spec:kbdgen:req:tsf.test.host]
> The planner, which turns an `Action` plus context flags into
> `tsf.edit.apply`, `tsf.edit.inject` or composition steps, MUST be a module
> with no TSF calls. It MUST be tested against a fake context on every host.
> Fault-injection tests MUST force a panic in each COM entry point and check
> that the call returns `E_UNEXPECTED` and that the service is then poisoned
> (`tsf.component.panic`).

> [spec:kbdgen:req:tsf.test.vm]
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
> Arm64 and Arm64X need a Windows on Arm machine.
