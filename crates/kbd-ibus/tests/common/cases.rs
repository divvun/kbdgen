//! The typing cases of `linux.test.planner`: those of
//! `ldml.chromeos.test.typing`, sent as IBus key codes and state bits,
//! with the engine event each key is natively, and GNOME Shell's virtual
//! keys and on-screen Backspace.

use kbd_engine::{Action, Context, Key, KeyEvent, Model, ModifierState, State};
use kbd_ibus::keys::{
    CONTROL_MASK, Facts, LOCK_MASK, MOD1_MASK, MOD4_MASK, MOD5_MASK, SHIFT_MASK, SUPER_MASK,
};
use kbd_ibus::keysym;
use kbd_ibus::planner::OPTIONS;
use kbd_model::{ExtraModifierKey, evdev_code};

/// A key's keyval and evdev key code.
pub type Press = (u32, u32);

const SHIFT_L: Press = (0xffe1, 42);
const SHIFT_R: Press = (0xffe2, 54);
const CONTROL_L: Press = (0xffe3, 29);
const CONTROL_R: Press = (0xffe4, 97);
const CAPS_LOCK: Press = (0xffe5, 58);
const ALT_L: Press = (0xffe9, 56);
const ALT_R: Press = (0xffea, 100);
const LEVEL3: Press = (0xfe03, 100);
const SUPER_L: Press = (0xffeb, 125);
const B00: Press = (0x3c, 86);

pub const BACKSPACE: Press = (0xff08, 14);
pub const DECIMAL: Press = (0xffae, 83);
pub const LEFT: Press = (0xff51, 105);
pub const RETURN: Press = (0xff0d, 28);

/// What the client types when the engine passes a key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Typed {
    Text(char),
    Backspace,
    Nothing,
}

impl Typed {
    /// As a GTK field types a key event: its keyval's character, unless a
    /// control character; Backspace deletes.
    pub fn of(keyval: u32, code: u32, state: u32) -> Typed {
        if keyval == BACKSPACE.0 || code == BACKSPACE.1 {
            return Typed::Backspace;
        }
        if state & (CONTROL_MASK | MOD1_MASK | MOD4_MASK | SUPER_MASK) != 0 {
            return Typed::Nothing;
        }
        keysym::to_char(keyval)
            .filter(|c| !c.is_control())
            .map_or(Typed::Nothing, Typed::Text)
    }
}

/// One key as IBus sends it, with the modifier keys around it, and the
/// event the engine gets for it natively, `None` for a frame key.
#[derive(Debug, Clone)]
pub struct Stroke {
    pub name: String,
    /// Pressed and released before the key, as Caps Lock is.
    pub taps: Vec<Press>,
    /// Held around the key.
    pub held: Vec<Press>,
    pub keyval: u32,
    pub code: u32,
    pub state: u32,
    pub event: Option<KeyEvent>,
}

impl Stroke {
    pub fn typed(&self) -> Typed {
        Typed::of(self.keyval, self.code, self.state)
    }
}

#[derive(Debug, Clone)]
pub enum Step {
    Key(Stroke),
    /// GNOME Shell's on-screen Backspace: through the engine while a
    /// preedit is shown, else a deletion past it.
    OnScreenBackspace,
}

#[derive(Debug, Clone)]
pub struct Case {
    pub name: String,
    pub steps: Vec<Step>,
}

/// A modifier state: its name, the keys tapped and held for it, its state
/// bits, and the engine's modifiers.
struct Held {
    name: String,
    taps: Vec<Press>,
    held: Vec<Press>,
    state: u32,
    modifiers: ModifierState,
}

fn held(name: &str, taps: &[Press], held: &[Press], state: u32, m: ModifierState) -> Held {
    Held {
        name: name.to_owned(),
        taps: taps.to_vec(),
        held: held.to_vec(),
        state,
        modifiers: m,
    }
}

/// The modifier states of `ldml.chromeos.test.typing`, as Linux reports
/// them: either Shift, Caps Lock, Caps Lock and Shift, AltGr reported as
/// Alt and as `ISO_Level3_Shift`, AltGr with Shift or Caps Lock, Left Alt,
/// either Ctrl, Ctrl with AltGr, Super, and each bound extra-modifier key.
fn modifier_states(model: &Model) -> Vec<Held> {
    let facts = Facts::of(model);
    let none = ModifierState::default();
    let altgr = ModifierState::altgr();
    let shift_lock = facts.windows.shift_lock;
    let mut states = vec![
        held("none", &[], &[], 0, none),
        held(
            "Shift_L",
            &[],
            &[SHIFT_L],
            SHIFT_MASK,
            ModifierState {
                shift_l: true,
                ..none
            },
        ),
        held(
            "Shift_R",
            &[],
            &[SHIFT_R],
            SHIFT_MASK,
            ModifierState {
                shift_r: true,
                ..none
            },
        ),
        held(
            "Caps",
            &[CAPS_LOCK],
            &[],
            LOCK_MASK,
            ModifierState { caps: true, ..none },
        ),
        held(
            "Caps+Shift",
            &[CAPS_LOCK],
            &[SHIFT_L],
            LOCK_MASK | SHIFT_MASK,
            ModifierState {
                shift_l: true,
                caps: !shift_lock,
                ..none
            },
        ),
        held(
            "AltGr as Alt",
            &[],
            &[ALT_R],
            MOD1_MASK,
            ModifierState {
                alt_r: true,
                altgr: facts.uses_level3,
                ..none
            },
        ),
        held("AltGr", &[], &[LEVEL3], MOD5_MASK, altgr),
        held(
            "AltGr+Shift",
            &[],
            &[LEVEL3, SHIFT_L],
            MOD5_MASK | SHIFT_MASK,
            ModifierState {
                shift_l: true,
                ..altgr
            },
        ),
        held(
            "AltGr+Caps",
            &[CAPS_LOCK],
            &[LEVEL3],
            MOD5_MASK | LOCK_MASK,
            ModifierState {
                caps: true,
                ..altgr
            },
        ),
        held(
            "Alt_L",
            &[],
            &[ALT_L],
            MOD1_MASK,
            ModifierState {
                alt_l: true,
                ..none
            },
        ),
        held(
            "Ctrl_L",
            &[],
            &[CONTROL_L],
            CONTROL_MASK,
            ModifierState {
                ctrl_l: true,
                ..none
            },
        ),
        held(
            "Ctrl_R",
            &[],
            &[CONTROL_R],
            CONTROL_MASK,
            ModifierState {
                ctrl_r: true,
                ..none
            },
        ),
        held(
            "Ctrl+AltGr",
            &[],
            &[CONTROL_L, LEVEL3],
            CONTROL_MASK | MOD5_MASK,
            ModifierState {
                ctrl_l: true,
                ..altgr
            },
        ),
        held(
            "Super",
            &[],
            &[SUPER_L],
            MOD4_MASK | SUPER_MASK,
            ModifierState { cmd: true, ..none },
        ),
    ];
    for (slot, key) in facts.windows.extra_modifiers.iter().enumerate() {
        let mut extra = [false; 3];
        extra[slot] = true;
        match key {
            ExtraModifierKey::CapsLock => states.push(held(
                "extra Caps Lock",
                &[],
                &[CAPS_LOCK],
                LOCK_MASK,
                ModifierState {
                    caps: true,
                    extra,
                    ..none
                },
            )),
            ExtraModifierKey::B00 => states.push(held(
                "extra B00",
                &[],
                &[B00],
                0,
                ModifierState { extra, ..none },
            )),
            ExtraModifierKey::RightCtrl => {}
        }
    }
    states
}

/// The form's positions with an evdev key code, but a `B00` bound as an
/// extra modifier, which is held rather than typed.
pub fn positions(model: &Model) -> Vec<(u8, u32)> {
    let keyboard = model.keyboard();
    let bound_b00 = keyboard
        .windows
        .extra_modifiers
        .contains(&ExtraModifierKey::B00);
    keyboard
        .hardware
        .iter()
        .flat_map(|hardware| hardware.form.rows.iter().flatten().copied())
        .filter(|scan| !(bound_b00 && *scan == 0x56))
        .filter_map(|scan| Some((scan, u32::from(evdev_code(scan)?))))
        .collect()
}

fn hardware(position: (u8, u32), state: &Held) -> Stroke {
    let (scan, code) = position;
    Stroke {
        name: format!("{scan:02X} {}", state.name),
        taps: state.taps.clone(),
        held: state.held.clone(),
        keyval: 0,
        code,
        state: state.state,
        event: Some(KeyEvent::with(Key::Scan(scan), state.modifiers)),
    }
}

fn special(name: &str, press: Press, key: Option<Key>) -> Stroke {
    Stroke {
        name: name.to_owned(),
        taps: Vec::new(),
        held: Vec::new(),
        keyval: press.0,
        code: press.1,
        state: 0,
        event: key.map(KeyEvent::new),
    }
}

/// Backspace, the decimal key, a frame key and Enter.
pub fn endings() -> Vec<Stroke> {
    vec![
        special("Backspace", BACKSPACE, Some(Key::Backspace)),
        special("Decimal", DECIMAL, Some(Key::Decimal)),
        special("Left", LEFT, None),
        special("Return", RETURN, Some(Key::Scan(0x1C))),
    ]
}

/// GNOME Shell's on-screen key for `c`: its keyval, as
/// `Clutter.unicode_to_keysym` gives it, with key code 0.
pub fn onscreen(c: char) -> Stroke {
    let code = u32::from(c);
    let keyval = if (0x20..=0x7e).contains(&code) || (0xa0..=0xff).contains(&code) {
        code
    } else {
        (0x100..0xfe00)
            .find(|keysym| keysym::to_char(*keysym) == Some(c))
            .unwrap_or(0x0100_0000 + code)
    };
    Stroke {
        name: format!("on-screen {c}"),
        taps: Vec::new(),
        held: Vec::new(),
        keyval,
        code: 0,
        state: 0,
        event: Some(KeyEvent::new(Key::Emit(c.to_string()))),
    }
}

fn case(strokes: &[&Stroke]) -> Case {
    Case {
        name: strokes
            .iter()
            .map(|stroke| stroke.name.as_str())
            .collect::<Vec<_>>()
            .join(", "),
        steps: strokes
            .iter()
            .map(|stroke| Step::Key((*stroke).clone()))
            .collect(),
    }
}

/// What a stroke does from `state` with `text` before the caret: the
/// action, and the state it leads to.
fn natively(model: &Model, state: &State, text: &str, stroke: &Stroke) -> (Action, State) {
    let context = Context::new(text);
    match &stroke.event {
        Some(event) => model.key(state, &context, event),
        None => (Action::Pass, state.clone()),
    }
}

/// Every form position under every modifier state; the dead-key paths of
/// up to three keys from the reset state, a dead key typed bare, with
/// Shift or with AltGr followed by keys typed bare or with Shift, a third
/// key following a second that chains to a further dead key, each also
/// ended by Backspace,
/// the decimal key, a frame key and Enter; GNOME Shell's on-screen text,
/// alone and after a dead key; and its on-screen Backspace past the
/// engine.
pub fn cases(model: &Model) -> Vec<Case> {
    let model = model.clone().with_options(OPTIONS);
    let positions = positions(&model);
    let states = modifier_states(&model);
    let mut cases = Vec::new();
    let mut typing = Vec::new();
    let mut following = Vec::new();
    for state in &states {
        for position in &positions {
            let stroke = hardware(*position, state);
            cases.push(case(&[&stroke]));
            if matches!(state.name.as_str(), "none" | "Shift_L" | "AltGr") {
                if state.name != "AltGr" {
                    following.push(stroke.clone());
                }
                typing.push(stroke);
            }
        }
    }
    let start = State::default();
    let pending = |state: &State| !model.pending_markers(state).is_empty();
    let mut texts = Vec::new();
    for first in &typing {
        let (action, after) = natively(&model, &start, "", first);
        if let Action::Edit { insert, .. } = &action
            && !pending(&after)
            && insert.chars().count() == 1
        {
            texts.extend(insert.chars());
        }
        if !pending(&after) {
            continue;
        }
        for ending in endings() {
            cases.push(case(&[first, &ending]));
        }
        for second in &following {
            cases.push(case(&[first, second]));
            let (action, after_second) = natively(&model, &after, "", second);
            let chained = matches!(action, Action::Edit { ref insert, .. } if insert.is_empty());
            if chained && pending(&after_second) {
                for ending in endings() {
                    cases.push(case(&[first, second, &ending]));
                }
                for third in &following {
                    cases.push(case(&[first, second, third]));
                }
            }
        }
    }
    texts.sort_unstable();
    texts.dedup();
    let dead = typing
        .iter()
        .find(|stroke| pending(&natively(&model, &start, "", stroke).1));
    for c in texts {
        let text = onscreen(c);
        cases.push(case(&[&text]));
        if let Some(dead) = dead {
            cases.push(case(&[dead, &text]));
        }
    }
    let plain: Vec<&Stroke> = typing
        .iter()
        .filter(|stroke| {
            let (action, after) = natively(&model, &start, "", stroke);
            matches!(action, Action::Edit { ref insert, .. } if !insert.is_empty())
                && !pending(&after)
        })
        .take(3)
        .collect();
    if let [a, b, c, ..] = plain.as_slice() {
        let mut deletion = case(&[a, b]);
        deletion.name.push_str(", on-screen Backspace, ");
        deletion.name.push_str(&c.name);
        deletion.steps.push(Step::OnScreenBackspace);
        deletion.steps.push(Step::Key((*c).clone()));
        cases.push(deletion);
        if let Some(dead) = dead {
            let mut cancel = case(&[a, dead]);
            cancel.name.push_str(", on-screen Backspace");
            cancel.steps.push(Step::OnScreenBackspace);
            cancel.steps.push(Step::Key((*b).clone()));
            cases.push(cancel);
        }
    }
    cases
}

/// Every pair of form positions, unmodified, for a model whose edits
/// delete.
pub fn pairs(model: &Model) -> Vec<Case> {
    let none = held("none", &[], &[], 0, ModifierState::default());
    let strokes: Vec<Stroke> = positions(model)
        .into_iter()
        .map(|position| hardware(position, &none))
        .collect();
    strokes
        .iter()
        .flat_map(|a| strokes.iter().map(move |b| case(&[a, b])))
        .collect()
}
