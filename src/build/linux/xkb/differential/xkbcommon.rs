//! `linux.test.xkbcommon`: each fixture's XKB layout compiled, listed and
//! typed through the installed libxkbcommon and libxkbregistry. The build
//! script sets `kbdgen_xkbcommon` when pkg-config finds both for a Linux
//! target; elsewhere the test says so and passes.

// [spec:kbdgen:req:linux.test.xkbcommon+1/test]
#[test]
fn fixtures_compile_list_and_type_in_libxkbcommon() {
    #[cfg(all(target_os = "linux", kbdgen_xkbcommon))]
    real::run();
    #[cfg(not(all(target_os = "linux", kbdgen_xkbcommon)))]
    eprintln!(
        "linux.test.xkbcommon skipped: pkg-config found no xkbcommon and xkbregistry for this target"
    );
}

#[cfg(all(target_os = "linux", kbdgen_xkbcommon))]
mod real {
    use std::ffi::{CStr, CString, c_char, c_int, c_void};
    use std::path::Path;
    use std::sync::Mutex;

    use xkbcommon::xkb;

    use super::super::compare::{Press, keysym, presses};
    use super::super::{Subject, fixture_layouts, subject};
    use crate::build::linux::xkb::keycodes::{DECIMAL, KEYS};
    use crate::build::linux::xkb::tests::ids;
    use crate::build::linux::xkb::{RegistryEntry, Tree};

    const XKB_LOG_LEVEL_WARNING: c_int = 30;
    const RXKB_CONTEXT_NO_DEFAULT_INCLUDES: c_int = 1;

    type LogFn = unsafe extern "C" fn(*mut c_void, c_int, *const c_char, *mut c_void);

    #[link(name = "xkbcommon")]
    unsafe extern "C" {
        fn xkb_context_set_log_fn(context: *mut c_void, log_fn: LogFn);
        fn xkb_context_set_log_level(context: *mut c_void, level: c_int);
    }

    unsafe extern "C" {
        fn vsnprintf(
            buf: *mut c_char,
            size: usize,
            format: *const c_char,
            args: *mut c_void,
        ) -> c_int;
    }

    #[link(name = "xkbregistry")]
    unsafe extern "C" {
        fn rxkb_context_new(flags: c_int) -> *mut c_void;
        fn rxkb_context_include_path_append(context: *mut c_void, path: *const c_char) -> bool;
        fn rxkb_context_include_path_append_default(context: *mut c_void) -> bool;
        fn rxkb_context_parse(context: *mut c_void, ruleset: *const c_char) -> bool;
        fn rxkb_context_unref(context: *mut c_void) -> *mut c_void;
        fn rxkb_layout_first(context: *mut c_void) -> *mut c_void;
        fn rxkb_layout_next(layout: *mut c_void) -> *mut c_void;
        fn rxkb_layout_get_name(layout: *mut c_void) -> *const c_char;
        fn rxkb_layout_get_variant(layout: *mut c_void) -> *const c_char;
        fn rxkb_layout_get_brief(layout: *mut c_void) -> *const c_char;
        fn rxkb_layout_get_description(layout: *mut c_void) -> *const c_char;
        fn rxkb_layout_get_iso639_first(layout: *mut c_void) -> *mut c_void;
        fn rxkb_iso639_code_next(code: *mut c_void) -> *mut c_void;
        fn rxkb_iso639_code_get_code(code: *mut c_void) -> *const c_char;
        fn rxkb_layout_get_iso3166_first(layout: *mut c_void) -> *mut c_void;
        fn rxkb_iso3166_code_next(code: *mut c_void) -> *mut c_void;
        fn rxkb_iso3166_code_get_code(code: *mut c_void) -> *const c_char;
    }

    /// What libxkbcommon logged since the last [`take_log`].
    static LOG: Mutex<Vec<String>> = Mutex::new(Vec::new());

    /// Formats a message into [`LOG`]. On x86-64 and AArch64 Linux a
    /// `va_list` parameter arrives as a pointer, which `vsnprintf` takes.
    unsafe extern "C" fn log(
        _context: *mut c_void,
        _level: c_int,
        format: *const c_char,
        args: *mut c_void,
    ) {
        let mut buffer = [0 as c_char; 1024];
        // SAFETY: `format` and `args` are the pair libxkbcommon passes, and
        // the buffer's length bounds the write.
        let message = unsafe {
            vsnprintf(buffer.as_mut_ptr(), buffer.len(), format, args);
            CStr::from_ptr(buffer.as_ptr())
        };
        LOG.lock()
            .unwrap()
            .push(message.to_string_lossy().trim_end().to_owned());
    }

    fn take_log() -> Vec<String> {
        std::mem::take(&mut *LOG.lock().unwrap())
    }

    /// A context with `xkb` before the default include paths, as
    /// `xkbcli compile-keymap --include <xkb> --include-defaults` makes
    /// it, logging warnings and errors into [`LOG`].
    fn context(xkb: &Path) -> xkb::Context {
        let mut context = xkb::Context::new(xkb::CONTEXT_NO_DEFAULT_INCLUDES);
        assert!(context.include_path_append(xkb), "{}", xkb.display());
        assert!(context.include_path_append_default());
        // SAFETY: the context is live, and `log` has the signature
        // libxkbcommon calls.
        unsafe {
            let raw = context.get_raw_ptr().cast();
            xkb_context_set_log_fn(raw, log);
            xkb_context_set_log_level(raw, XKB_LOG_LEVEL_WARNING);
        }
        context
    }

    fn compile(context: &xkb::Context, layout: &str) -> Option<xkb::Keymap> {
        xkb::Keymap::new_from_names(
            context,
            "evdev",
            "pc105",
            layout,
            "",
            None,
            xkb::KEYMAP_COMPILE_NO_FLAGS,
        )
    }

    fn string(text: *const c_char) -> Option<String> {
        // SAFETY: libxkbregistry returns NUL-terminated strings, or null.
        (!text.is_null()).then(|| {
            unsafe { CStr::from_ptr(text) }
                .to_string_lossy()
                .into_owned()
        })
    }

    /// The registry entry libxkbregistry lists for `name` with `xkb` before
    /// the default paths: brief, description, languages, countries.
    fn listed(xkb: &Path, name: &str) -> Option<(String, String, Vec<String>, Vec<String>)> {
        let path = CString::new(xkb.to_str()?).ok()?;
        let evdev = CString::new("evdev").ok()?;
        // SAFETY: each pointer comes from libxkbregistry and is used while
        // the context lives; the context is released at the end.
        unsafe {
            let context = rxkb_context_new(RXKB_CONTEXT_NO_DEFAULT_INCLUDES);
            assert!(rxkb_context_include_path_append(context, path.as_ptr()));
            assert!(rxkb_context_include_path_append_default(context));
            assert!(
                rxkb_context_parse(context, evdev.as_ptr()),
                "rxkb_context_parse"
            );
            let mut found = None;
            let mut layout = rxkb_layout_first(context);
            while !layout.is_null() && found.is_none() {
                let variant = string(rxkb_layout_get_variant(layout)).unwrap_or_default();
                if string(rxkb_layout_get_name(layout)).as_deref() == Some(name)
                    && variant.is_empty()
                {
                    let mut languages = Vec::new();
                    let mut code = rxkb_layout_get_iso639_first(layout);
                    while !code.is_null() {
                        languages.extend(string(rxkb_iso639_code_get_code(code)));
                        code = rxkb_iso639_code_next(code);
                    }
                    let mut countries = Vec::new();
                    let mut code = rxkb_layout_get_iso3166_first(layout);
                    while !code.is_null() {
                        countries.extend(string(rxkb_iso3166_code_get_code(code)));
                        code = rxkb_iso3166_code_next(code);
                    }
                    found = Some((
                        string(rxkb_layout_get_brief(layout)).unwrap_or_default(),
                        string(rxkb_layout_get_description(layout)).unwrap_or_default(),
                        languages,
                        countries,
                    ));
                }
                layout = rxkb_layout_next(layout);
            }
            rxkb_context_unref(context);
            found
        }
    }

    /// The XKB key code of a press's key.
    fn keycode(press: Press) -> u32 {
        KEYS.iter()
            .chain([&DECIMAL])
            .find(|key| key.name == press.name)
            .map(|key| key.keycode())
            .unwrap_or_default()
    }

    const SHIFT_L: u32 = 50;
    const CAPS_LOCK: u32 = 66;
    const NUM_LOCK: u32 = 77;
    const RALT: u32 = 108;

    fn tap(state: &mut xkb::State, code: u32) {
        state.update_key(xkb::Keycode::new(code), xkb::KeyDirection::Down);
        state.update_key(xkb::Keycode::new(code), xkb::KeyDirection::Up);
    }

    /// The keysym and text libxkbcommon gives `press`, its modifier keys
    /// pressed as a user does.
    fn typed(keymap: &xkb::Keymap, press: Press) -> (u32, String) {
        let mut state = xkb::State::new(keymap);
        if press.state.lock() {
            tap(&mut state, CAPS_LOCK);
        }
        if press.is_decimal() {
            tap(&mut state, NUM_LOCK);
        }
        if press.state.shift() {
            state.update_key(xkb::Keycode::new(SHIFT_L), xkb::KeyDirection::Down);
        }
        if press.state.level3() {
            state.update_key(xkb::Keycode::new(RALT), xkb::KeyDirection::Down);
        }
        let key = xkb::Keycode::new(keycode(press));
        (state.key_get_one_sym(key).raw(), state.key_get_utf8(key))
    }

    fn entry_matches(subject: &Subject, xkb: &Path) -> Result<(), String> {
        let RegistryEntry {
            name,
            short_description,
            description,
            country,
            language,
        } = &subject.adapted.entry;
        let expected = (
            short_description.clone(),
            description.clone(),
            language.iter().cloned().collect::<Vec<_>>(),
            country.iter().cloned().collect::<Vec<_>>(),
        );
        match listed(xkb, name) {
            Some(found) if found == expected => Ok(()),
            found => Err(format!(
                "{name}: libxkbregistry lists {found:?}, not {expected:?}"
            )),
        }
    }

    /// Checks one fixture, returning what libxkbcommon does otherwise.
    fn check(subject: &Subject, dir: &Path) -> Vec<String> {
        let mut problems = Vec::new();
        let tree = Tree::new(&ids(), std::slice::from_ref(&subject.layout)).unwrap();
        let xkb = dir.join("xkb");
        for (path, text) in &tree.files {
            let path = xkb.join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, text).unwrap();
        }
        let label = &subject.label;
        let id = &subject.adapted.entry.name;
        let context = context(&xkb);
        take_log();
        let baseline = compile(&context, "us");
        assert!(baseline.is_some(), "the us layout does not compile");
        let expected_log = take_log();
        let Some(keymap) = compile(&context, id) else {
            return vec![format!("{label}: {id} does not compile: {:?}", take_log())];
        };
        for message in take_log() {
            if !expected_log.contains(&message) {
                problems.push(format!("{label}: compiling {id} logged {message:?}"));
            }
        }
        if let Err(problem) = entry_matches(subject, &xkb) {
            problems.push(format!("{label}: {problem}"));
        }
        for press in presses(subject) {
            let Some(predicted) = keysym(subject, press) else {
                continue;
            };
            let text: String = predicted
                .to_char()
                .filter(|c| !c.is_control())
                .map(String::from)
                .unwrap_or_default();
            let (sym, utf8) = typed(&keymap, press);
            if sym != predicted.0 || utf8 != text {
                problems.push(format!(
                    "{label}: {press}: libxkbcommon gives {} {utf8:?}, the semantics {} {text:?}",
                    crate::build::linux::xkb::keysym::Keysym(sym),
                    predicted
                ));
            }
            if let Some(c) = predicted.to_char()
                && !press.is_decimal()
                && predicted.0 < 0x0100_0000
                && xkb::utf32_to_keysym(u32::from(c)).raw() != predicted.0
            {
                problems.push(format!(
                    "{label}: {press}: xkb_utf32_to_keysym({c:?}) is not {predicted}"
                ));
            }
        }
        problems
    }

    /// Whether the log callback sees a warning a symbols file causes, so
    /// that a clean log means something.
    fn log_is_captured() -> bool {
        let dir = tempfile::tempdir().unwrap();
        let symbols = dir.path().join("symbols");
        std::fs::create_dir_all(&symbols).unwrap();
        std::fs::write(
            symbols.join("kbdgen-broken"),
            "default partial alphanumeric_keys\nxkb_symbols \"basic\" {\n    key <AC01> { [ kbdgen_no_such_keysym ] };\n};\n",
        )
        .unwrap();
        let context = context(dir.path());
        take_log();
        compile(&context, "us");
        let baseline = take_log();
        compile(&context, "kbdgen-broken");
        take_log().iter().any(|message| !baseline.contains(message))
    }

    // [spec:kbdgen:req:linux.test.xkbcommon+1]
    pub fn run() {
        assert!(
            log_is_captured(),
            "libxkbcommon's warnings do not reach the test"
        );
        let mut problems = Vec::new();
        for (label, tag, yaml) in fixture_layouts() {
            let subject = subject(&label, &tag, &yaml);
            let dir = tempfile::tempdir().unwrap();
            let found = check(&subject, dir.path());
            eprintln!("{label}: {} problem(s)", found.len());
            problems.extend(found);
        }
        assert!(problems.is_empty(), "{}", problems.join("\n"));
    }
}
