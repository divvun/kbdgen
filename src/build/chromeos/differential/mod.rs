//! `ldml.chromeos.test.typing`: the generated extension's service worker,
//! engine and models, driven in Deno through a fake `chrome.input.ime`,
//! against `kbd-engine` running natively on the same model.
//!
//! For each fixture layout the generator writes an extension. The cases
//! are sequences of key strokes: every position of the layout's form under
//! each modifier state, every dead-key path, special keys after a dead key
//! and, for the context fixture, every pair of keys. Each stroke becomes
//! the key downs and ups ChromeOS sends, held modifiers included. The
//! native side runs each case through [`Host`], the host behaviour of
//! `ldml.chromeos.semantics` written over the engine API, which gives the
//! committed text, the composition and which key downs are handled. The
//! harness `harness/typing_test.ts` loads the generated `background.js`
//! with a fake text field that applies the IME calls and reports the text
//! before the caret, with and without the composition and on time or one
//! key late, and requires the same results.
//!
//! The test needs `deno` (or `$DENO`); without it, it says so and passes.

use std::collections::{BTreeSet, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use kbd_engine::{
    Action, BackspacePolicy, Context, Key, KeyEvent, Model, ModifierState, Options, OutputForm,
    State,
};
use kbd_model::{ExtraModifierKey, Host as ModelHost};
use serde::Serialize;

use super::tests::{TARGET, WORKSPACE, fixture_text, generate};

/// Each ISO position's `KeyboardEvent.code` by its scan code, written out
/// independently of the service worker's table.
const POSITION_CODES: [(u8, &str); 51] = [
    (0x29, "Backquote"),
    (0x02, "Digit1"),
    (0x03, "Digit2"),
    (0x04, "Digit3"),
    (0x05, "Digit4"),
    (0x06, "Digit5"),
    (0x07, "Digit6"),
    (0x08, "Digit7"),
    (0x09, "Digit8"),
    (0x0A, "Digit9"),
    (0x0B, "Digit0"),
    (0x0C, "Minus"),
    (0x0D, "Equal"),
    (0x10, "KeyQ"),
    (0x11, "KeyW"),
    (0x12, "KeyE"),
    (0x13, "KeyR"),
    (0x14, "KeyT"),
    (0x15, "KeyY"),
    (0x16, "KeyU"),
    (0x17, "KeyI"),
    (0x18, "KeyO"),
    (0x19, "KeyP"),
    (0x1A, "BracketLeft"),
    (0x1B, "BracketRight"),
    (0x1E, "KeyA"),
    (0x1F, "KeyS"),
    (0x20, "KeyD"),
    (0x21, "KeyF"),
    (0x22, "KeyG"),
    (0x23, "KeyH"),
    (0x24, "KeyJ"),
    (0x25, "KeyK"),
    (0x26, "KeyL"),
    (0x27, "Semicolon"),
    (0x28, "Quote"),
    (0x2B, "Backslash"),
    (0x56, "IntlBackslash"),
    (0x2C, "KeyZ"),
    (0x2D, "KeyX"),
    (0x2E, "KeyC"),
    (0x2F, "KeyV"),
    (0x30, "KeyB"),
    (0x31, "KeyN"),
    (0x32, "KeyM"),
    (0x33, "Comma"),
    (0x34, "Period"),
    (0x35, "Slash"),
    (0x73, "IntlRo"),
    (0x7D, "IntlYen"),
    (0x39, "Space"),
];

const MODIFIER_CODES: [&str; 8] = [
    "ShiftLeft",
    "ShiftRight",
    "ControlLeft",
    "ControlRight",
    "AltLeft",
    "AltRight",
    "MetaLeft",
    "CapsLock",
];

/// A context fixture: text transforms that rewrite earlier text, an
/// astral character, and normalization that recomposes with the text
/// before the caret, so edits delete.
const CONTEXT: &str = "format: 4\ndisplayNames: {und: Context}\nnormalization: enabled\ntransforms: [[{from: qq, to: ʠ}, {from: '\\u{1D56B}z', to: ż}]]\nhardware:\n  chromeOS:\n    layers:\n      none: |\n        § 1 2 3 4 5 6 7 8 9 0 + \\u{301}\n        q w e r t y u i o p å \\u{1D56B}\n        a s d f g h j k l ö æ \\u{308}\n        < z x c v b n m , . -\n";

/// The fixture layouts: the label, the tag, and the layout text.
fn fixtures() -> Vec<(&'static str, &'static str, String)> {
    let chromeos = |text: String, variant: &str| {
        text.replace(
            &format!("hardware:\n  {variant}:"),
            "hardware:\n  chromeOS:",
        )
    };
    vec![
        (
            "layouts/vro",
            "vro",
            fixture_text("crates/kbd-engine/tests/golden/layouts/vro.yaml"),
        ),
        (
            "layouts/und-x-mods",
            "und-x-mods",
            fixture_text("crates/kbd-engine/tests/golden/layouts/und-x-mods.yaml"),
        ),
        (
            "v3/se-NO",
            "se-NO",
            fixture_text("crates/kbd-engine/tests/golden/v3/se-NO.yaml"),
        ),
        (
            "chromeos/vro-v3",
            "vro",
            fixture_text("src/build/chromeos/testdata/vro-v3.yaml"),
        ),
        (
            "macos/features",
            "sme",
            chromeos(
                fixture_text("src/build/macos/testdata/features.yaml"),
                "macOS",
            ),
        ),
        (
            "windows/features",
            "sme",
            chromeos(
                fixture_text("src/build/windows/kbdl/testdata/features.yaml"),
                "windows",
            ),
        ),
        ("context", "und", CONTEXT.to_owned()),
    ]
}

/// A key press: the key and the modifier keys held down around it, with
/// the Caps Lock state, and whether the XKB layout beneath reports Right
/// Alt as AltGr rather than as Alt.
#[derive(Debug, Clone)]
struct Stroke {
    code: &'static str,
    held: Vec<&'static str>,
    caps: bool,
    altgr_flag: bool,
}

impl Stroke {
    fn plain(code: &'static str) -> Self {
        Stroke {
            code,
            held: Vec::new(),
            caps: false,
            altgr_flag: false,
        }
    }
}

/// A key event as `chrome.input.ime.onKeyEvent` delivers it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct KeyData {
    r#type: &'static str,
    code: &'static str,
    key: &'static str,
    shift_key: bool,
    ctrl_key: bool,
    alt_key: bool,
    altgr_key: bool,
    caps_lock: bool,
}

fn key_data(kind: &'static str, code: &'static str, down: &[&str], stroke: &Stroke) -> KeyData {
    let has = |code: &str| down.contains(&code);
    KeyData {
        r#type: kind,
        code,
        key: if code == "NumpadDecimal" { "." } else { code },
        shift_key: has("ShiftLeft") || has("ShiftRight"),
        ctrl_key: has("ControlLeft") || has("ControlRight"),
        alt_key: has("AltLeft") || (has("AltRight") && !stroke.altgr_flag),
        altgr_key: has("AltRight") && stroke.altgr_flag,
        caps_lock: stroke.caps,
    }
}

/// The key downs and ups of a stroke, in order.
fn events(stroke: &Stroke) -> Vec<KeyData> {
    let mut down: Vec<&str> = Vec::new();
    let mut out = Vec::new();
    for &m in &stroke.held {
        down.push(m);
        out.push(key_data("keydown", m, &down, stroke));
    }
    down.push(stroke.code);
    out.push(key_data("keydown", stroke.code, &down, stroke));
    down.pop();
    out.push(key_data("keyup", stroke.code, &down, stroke));
    for &m in stroke.held.iter().rev() {
        down.retain(|d| *d != m);
        out.push(key_data("keyup", m, &down, stroke));
    }
    out
}

// [spec:kbdgen:req:ldml.chromeos.test.typing]
/// The ChromeOS host over the engine (`ldml.chromeos.semantics`): the
/// committed text before the caret, the composition, and the state.
struct Host<'m> {
    model: &'m Model,
    state: State,
    text: String,
    preedit: String,
    shift_lock: bool,
}

/// What a key down is to the host.
enum Role {
    Modifier,
    Other,
    Engine(Key),
}

fn role(code: &str) -> Role {
    if MODIFIER_CODES.contains(&code) {
        return Role::Modifier;
    }
    match code {
        "Backspace" => Role::Engine(Key::Backspace),
        "NumpadDecimal" => Role::Engine(Key::Decimal),
        _ => match POSITION_CODES.iter().find(|(_, c)| *c == code) {
            Some((scan, _)) => Role::Engine(Key::Scan(*scan)),
            None => Role::Other,
        },
    }
}

impl<'m> Host<'m> {
    fn new(model: &'m Model) -> Self {
        Host {
            model,
            state: State::default(),
            text: String::new(),
            preedit: String::new(),
            shift_lock: false,
        }
    }

    fn modifiers(&self, down: &[&str], caps: bool) -> ModifierState {
        let has = |code: &str| down.contains(&code);
        let windows = &self.model.keyboard().windows;
        let mut extra = [false; 3];
        for (slot, key) in extra.iter_mut().zip(&windows.extra_modifiers) {
            *slot = match key {
                ExtraModifierKey::CapsLock => has("CapsLock"),
                ExtraModifierKey::B00 => has("IntlBackslash"),
                ExtraModifierKey::RightCtrl => false,
            };
        }
        ModifierState {
            shift_l: has("ShiftLeft"),
            shift_r: has("ShiftRight"),
            caps: if windows.shift_lock {
                self.shift_lock
            } else {
                caps
            },
            ctrl_l: has("ControlLeft"),
            ctrl_r: has("ControlRight"),
            alt_l: has("AltLeft"),
            alt_r: has("AltRight"),
            altgr: has("AltRight"),
            cmd: has("MetaLeft"),
            extra,
        }
    }

    fn send(&mut self, key: Key, modifiers: ModifierState) -> Action {
        let context = Context {
            text: self.text.clone(),
            authoritative: true,
            at_start: true,
        };
        let (action, state) =
            self.model
                .key(&self.state, &context, &KeyEvent::with(key, modifiers));
        if let Action::Edit {
            delete,
            insert,
            preedit,
            ..
        } = &action
        {
            let keep = self.text.chars().count().saturating_sub(*delete);
            self.text = self.text.chars().take(keep).collect();
            self.text.push_str(insert);
            self.preedit.clone_from(preedit);
            self.state = state;
        }
        action
    }

    /// A key that passes: what is pending is committed, then the state
    /// resets.
    fn pass(&mut self) {
        if !self.preedit.is_empty() || !self.model.pending_markers(&self.state).is_empty() {
            self.send(Key::Commit, ModifierState::default());
        }
        self.state = State::default();
    }

    /// Handles a key down with `down` held, the key itself included;
    /// whether the key is handled.
    fn key_down(&mut self, code: &str, down: &[&str], caps: bool) -> bool {
        if code == "CapsLock" {
            self.shift_lock = true;
        } else if code == "ShiftLeft" || code == "ShiftRight" {
            self.shift_lock = false;
        }
        match role(code) {
            Role::Modifier => false,
            Role::Other => {
                self.pass();
                false
            }
            Role::Engine(key) => {
                let modifiers = self.modifiers(down, caps);
                match self.send(key, modifiers) {
                    Action::Pass => {
                        self.pass();
                        false
                    }
                    Action::Edit { .. } => true,
                }
            }
        }
    }

    /// Presses `stroke`; whether each of its key downs is handled.
    fn press(&mut self, stroke: &Stroke) -> Vec<bool> {
        let mut down: Vec<&str> = Vec::new();
        let mut handled = Vec::new();
        for &m in &stroke.held {
            down.push(m);
            handled.push(self.key_down(m, &down, stroke.caps));
        }
        down.push(stroke.code);
        handled.push(self.key_down(stroke.code, &down, stroke.caps));
        handled
    }
}

/// One case: the events the harness sends, and what the native host gives.
#[derive(Debug, Serialize)]
struct Case {
    label: String,
    events: Vec<KeyData>,
    handled: Vec<bool>,
    text: String,
    preedit: String,
}

#[derive(Debug, Serialize)]
struct Fixture {
    label: &'static str,
    extension: PathBuf,
    engine: &'static str,
    cases: Vec<Case>,
}

#[derive(Debug, Serialize)]
struct Cases {
    fixtures: Vec<Fixture>,
}

fn options() -> Options {
    Options {
        output_form: OutputForm::Nfc,
        backspace: BackspacePolicy::CancelOrPass,
        host: Some(ModelHost::ChromeOs),
    }
}

fn describe(strokes: &[Stroke]) -> String {
    strokes
        .iter()
        .map(|s| {
            let mut held: Vec<&str> = s.held.clone();
            if s.caps {
                held.push("caps");
            }
            if s.altgr_flag {
                held.push("altgr-flag");
            }
            if held.is_empty() {
                s.code.to_owned()
            } else {
                format!("{}+{}", held.join("+"), s.code)
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// The case of `strokes`, and, when it leaves a dead key pending, what it
/// leaves: the state and the text with its composition.
fn case(model: &Model, strokes: &[Stroke]) -> (Case, Option<(State, String, String)>) {
    let mut host = Host::new(model);
    let mut handled = Vec::new();
    let mut events_out = Vec::new();
    for stroke in strokes {
        handled.extend(host.press(stroke));
        events_out.extend(events(stroke));
    }
    let pending = (!model.pending_markers(&host.state).is_empty())
        .then(|| (host.state.clone(), host.text.clone(), host.preedit.clone()));
    (
        Case {
            label: describe(strokes),
            events: events_out,
            handled,
            text: host.text,
            preedit: host.preedit,
        },
        pending,
    )
}

/// The modifier states every position is pressed in.
fn states(model: &Model) -> Vec<(Vec<&'static str>, bool, bool)> {
    let mut states = vec![
        (vec![], false, false),
        (vec!["ShiftLeft"], false, false),
        (vec!["ShiftRight"], false, false),
        (vec![], true, false),
        (vec!["ShiftLeft"], true, false),
        (vec!["AltRight"], false, false),
        (vec!["AltRight"], false, true),
        (vec!["ShiftLeft", "AltRight"], false, false),
        (vec!["AltRight"], true, false),
        (vec!["AltLeft"], false, false),
        (vec!["ControlLeft"], false, false),
        (vec!["ControlRight"], false, false),
        (vec!["ControlLeft", "AltRight"], false, false),
        (vec!["MetaLeft"], false, false),
    ];
    for key in &model.keyboard().windows.extra_modifiers {
        match key {
            ExtraModifierKey::CapsLock => states.push((vec!["CapsLock"], false, false)),
            ExtraModifierKey::B00 => {
                states.push((vec!["IntlBackslash"], false, false));
                states.push((vec!["ShiftLeft", "IntlBackslash"], false, false));
            }
            ExtraModifierKey::RightCtrl => {}
        }
    }
    states
}

/// The codes of the form's positions.
fn positions(model: &Model) -> Vec<&'static str> {
    let hardware = model.keyboard().hardware.as_ref().unwrap();
    hardware
        .form
        .rows
        .iter()
        .flatten()
        .map(|&scan| {
            POSITION_CODES
                .iter()
                .find(|(s, _)| *s == scan)
                .unwrap_or_else(|| panic!("no code for scan code {scan:02X}"))
                .1
        })
        .collect()
}

// [spec:kbdgen:req:ldml.chromeos.test.typing]
/// Every case of one fixture. Dead-key paths continue from each distinct
/// pending outcome once, with the typing states, up to three keys.
fn cases(label: &str, model: &Model) -> Vec<Case> {
    let positions = positions(model);
    let stroke = |code, (held, caps, altgr_flag): &(Vec<&'static str>, bool, bool)| Stroke {
        code,
        held: held.clone(),
        caps: *caps,
        altgr_flag: *altgr_flag,
    };
    let states = states(model);
    let typing = [
        (vec![], false, false),
        (vec!["ShiftLeft"], false, false),
        (vec![], true, false),
        (vec!["AltRight"], false, false),
    ];
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    let mut frontier: Vec<Vec<Stroke>> = Vec::new();
    for code in &positions {
        for state in &states {
            let strokes = vec![stroke(*code, state)];
            let (case, pending) = case(model, &strokes);
            out.push(case);
            if pending.is_some_and(|p| seen.insert(p)) {
                frontier.push(strokes);
            }
        }
    }
    for code in ["NumpadDecimal", "Backspace"] {
        out.push(case(model, &[Stroke::plain(code)]).0);
    }
    let mut ends: Vec<Stroke> = positions
        .iter()
        .flat_map(|code| typing.iter().map(move |state| stroke(*code, state)))
        .collect();
    ends.extend(
        ["Backspace", "NumpadDecimal", "ArrowLeft", "Enter"]
            .into_iter()
            .map(Stroke::plain),
    );
    while !frontier.is_empty() {
        let mut next = Vec::new();
        for path in &frontier {
            for end in &ends {
                let mut strokes = path.clone();
                strokes.push(end.clone());
                let (case, pending) = case(model, &strokes);
                out.push(case);
                if strokes.len() < 3 && pending.is_some_and(|p| seen.insert(p)) {
                    next.push(strokes);
                }
            }
        }
        frontier = next;
    }
    if label == "context" {
        for first in &positions {
            for second in &positions {
                let strokes = [Stroke::plain(first), Stroke::plain(second)];
                out.push(case(model, &strokes).0);
            }
        }
        let triples = [["KeyQ", "KeyQ", "KeyQ"], ["KeyA", "Equal", "Equal"]];
        for triple in triples {
            let strokes: Vec<Stroke> = triple.into_iter().map(Stroke::plain).collect();
            out.push(case(model, &strokes).0);
        }
    }
    out
}

fn deno() -> Option<PathBuf> {
    let deno = std::env::var_os("DENO").map_or_else(|| PathBuf::from("deno"), PathBuf::from);
    Command::new(&deno)
        .arg("--version")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|_| deno)
}

// [spec:kbdgen:req:ldml.chromeos.test.typing/test]
// [spec:kbdgen:sem:ldml.chromeos.keys/test]
// [spec:kbdgen:sem:ldml.chromeos.modifiers/test]
// [spec:kbdgen:sem:ldml.chromeos.semantics/test]
// [spec:kbdgen:sem:ldml.chromeos.context/test]
// [spec:kbdgen:req:ldml.chromeos.wasm/test]
// [spec:kbdgen:req:chromeos.engine/test]
#[tokio::test]
async fn the_extension_types_what_the_engine_types() {
    let Some(deno) = deno() else {
        eprintln!(
            "skipping the ChromeOS typing test: deno is not on PATH and DENO is not set; \
             install Deno to run src/build/chromeos/differential/harness/typing_test.ts"
        );
        return;
    };
    let mut generated = Vec::new();
    let mut fixtures_out = Vec::new();
    let mut total = 0;
    for (label, tag, yaml) in fixtures() {
        let extension = generate(&[(tag, &yaml)], &[("chromeos", TARGET)], &[])
            .await
            .unwrap_or_else(|e| panic!("{label}: {e:#}"));
        let bytes = std::fs::read(extension.out.join(format!("models/{tag}.dvkb"))).unwrap();
        let model = Model::from_bytes(&bytes).unwrap().with_options(options());
        let cases = cases(label, &model);
        let handled: BTreeSet<bool> = cases.iter().flat_map(|c| c.handled.clone()).collect();
        assert_eq!(handled.len(), 2, "{label}: some keys pass and some type");
        eprintln!("{label}: {} cases", cases.len());
        total += cases.len();
        fixtures_out.push(Fixture {
            label,
            extension: extension.out.clone(),
            engine: tag,
            cases,
        });
        generated.push(extension);
    }
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cases.json");
    let json = serde_json::to_vec(&Cases {
        fixtures: fixtures_out,
    })
    .unwrap();
    std::fs::write(&path, json).unwrap();

    let harness = Path::new(WORKSPACE).join("src/build/chromeos/differential/harness");
    let output = Command::new(&deno)
        .current_dir(&harness)
        .args(["test", "--allow-read", "--allow-env"])
        .arg("typing_test.ts")
        .env("KBDGEN_CHROMEOS_CASES", &path)
        .env("DENO_NO_UPDATE_CHECK", "1")
        .env("NO_COLOR", "1")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    eprintln!("{total} cases\n{stdout}{stderr}");
    assert!(output.status.success(), "the Deno harness failed");
    assert!(
        stdout.contains("ok | 1 passed") || stderr.contains("ok | 1 passed"),
        "the harness ran no test"
    );
}
