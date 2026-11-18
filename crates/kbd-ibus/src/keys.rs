//! Key identity (`linux.ibus.keys`): what an IBus key event is to the
//! engine, the engine's modifiers while it is held
//! (`linux.ibus.modifiers`), and the on-screen keyboard's text keys
//! (`linux.ibus.virtual-keys`).
//!
//! A hardware key is identified by its evdev key code, never by its
//! keyval, which comes from whatever keymap the compositor applied.

use std::collections::{BTreeMap, BTreeSet};

use kbd_engine::{Action, Context, Key, KeyEvent, Model, ModifierState, State};
use kbd_model::{ExtraModifierKey, Windows, scan_code};

use crate::keysym;

/// `IBusModifierType` bits (`src/ibustypes.h`).
pub const SHIFT_MASK: u32 = 1 << 0;
pub const LOCK_MASK: u32 = 1 << 1;
pub const CONTROL_MASK: u32 = 1 << 2;
pub const MOD1_MASK: u32 = 1 << 3;
pub const MOD4_MASK: u32 = 1 << 6;
pub const MOD5_MASK: u32 = 1 << 7;
pub const FORWARD_MASK: u32 = 1 << 25;
pub const SUPER_MASK: u32 = 1 << 26;
pub const RELEASE_MASK: u32 = 1 << 30;

const BACKSPACE: u32 = 14;
const LEFT_CTRL: u32 = 29;
const LEFT_SHIFT: u32 = 42;
const RIGHT_SHIFT: u32 = 54;
const LEFT_ALT: u32 = 56;
const CAPS_LOCK: u32 = 58;
const NUM_LOCK: u32 = 69;
const DECIMAL: u32 = 83;
const B00: u32 = 86;
const RIGHT_CTRL: u32 = 97;
const RIGHT_ALT: u32 = 100;
const LEFT_SUPER: u32 = 125;
const RIGHT_SUPER: u32 = 126;

/// The modifier keys by code.
const MODIFIER_CODES: [u32; 10] = [
    LEFT_CTRL,
    LEFT_SHIFT,
    RIGHT_SHIFT,
    LEFT_ALT,
    CAPS_LOCK,
    NUM_LOCK,
    RIGHT_CTRL,
    RIGHT_ALT,
    LEFT_SUPER,
    RIGHT_SUPER,
];

/// What a key press is to the engine object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Role {
    /// A modifier: it passes and resets nothing.
    Modifier,
    /// A key the engine has no part in, such as an arrow: it passes.
    Frame,
    Engine(Key),
}

/// Classifies a press of `keycode`, an evdev key code, whose keyval is
/// `keyval`. Key code 0 is an on-screen keyboard's text, which GNOME Shell
/// sends a scalar value at a time: `Emit` of a keyval that maps to a
/// character other than a control character, so transforms and pending
/// dead keys apply to it, and otherwise a frame key, which GNOME Shell
/// then presses on its virtual keyboard.
// [spec:kbdgen:req:linux.ibus.keys]
// [spec:kbdgen:sem:linux.ibus.virtual-keys]
pub fn classify(keyval: u32, keycode: u32) -> Role {
    if keycode == 0 {
        return match keysym::to_char(keyval) {
            _ if keyval == keysym::BACKSPACE => Role::Engine(Key::Backspace),
            Some(c) if !c.is_control() => Role::Engine(Key::Emit(c.to_string())),
            _ => Role::Frame,
        };
    }
    if keysym::is_modifier(keyval) || MODIFIER_CODES.contains(&keycode) {
        return Role::Modifier;
    }
    match keycode {
        BACKSPACE => Role::Engine(Key::Backspace),
        DECIMAL if keyval == keysym::KP_DELETE => Role::Frame,
        DECIMAL => Role::Engine(Key::Decimal),
        _ => u16::try_from(keycode)
            .ok()
            .and_then(scan_code)
            .map_or(Role::Frame, |scan| Role::Engine(Key::Scan(scan))),
    }
}

/// What the host needs to know of a model to send its keys.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Facts {
    /// A state with Level3 (AltGr) selects a layer (`ldml.linux.levels`),
    /// so Right Alt is AltGr whatever the keymap beneath says.
    pub uses_level3: bool,
    pub windows: Windows,
}

impl Facts {
    /// Asks the engine whether AltGr, with or without Shift and Caps
    /// Lock, selects a layer: it passes a form key only when none does.
    // [spec:kbdgen:sem:linux.ibus.modifiers]
    pub fn of(model: &Model) -> Facts {
        let keyboard = model.keyboard();
        let probe = keyboard.hardware.as_ref().and_then(|hardware| {
            hardware
                .form
                .rows
                .iter()
                .flatten()
                .copied()
                .find(|scan| *scan != 0x56)
        });
        let uses_level3 = probe.is_some_and(|scan| {
            [(false, false), (true, false), (false, true), (true, true)]
                .into_iter()
                .any(|(shift_l, caps)| {
                    let modifiers = ModifierState {
                        shift_l,
                        caps,
                        ..ModifierState::altgr()
                    };
                    let event = KeyEvent::with(Key::Scan(scan), modifiers);
                    let (action, _) = model.key(&State::default(), &Context::default(), &event);
                    action != Action::Pass
                })
        });
        Facts {
            uses_level3,
            windows: keyboard.windows.clone(),
        }
    }
}

/// The modifier keys seen held, the host's own Shift Lock, and what each
/// key press returned, for its release.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Keys {
    held: BTreeSet<u32>,
    shift_lock: bool,
    /// (key code, keyval of a key code 0) → the press's reply.
    pressed: BTreeMap<(u32, u32), bool>,
}

fn identity(keyval: u32, keycode: u32) -> (u32, u32) {
    (keycode, if keycode == 0 { keyval } else { 0 })
}

impl Keys {
    /// Follows a press or release: modifier keys and `B00` held, and Shift
    /// Lock, which Caps Lock sets and Shift clears.
    // [spec:kbdgen:sem:linux.ibus.modifiers]
    pub fn observe(&mut self, keycode: u32, press: bool) {
        if press {
            match keycode {
                CAPS_LOCK => self.shift_lock = true,
                LEFT_SHIFT | RIGHT_SHIFT => self.shift_lock = false,
                _ => {}
            }
            if MODIFIER_CODES.contains(&keycode) || keycode == B00 {
                self.held.insert(keycode);
            }
        } else {
            self.held.remove(&keycode);
        }
    }

    pub fn pressed(&mut self, keyval: u32, keycode: u32, handled: bool) {
        self.pressed.insert(identity(keyval, keycode), handled);
    }

    /// What a release returns: what its press did, false for a press not
    /// seen.
    // [spec:kbdgen:req:linux.ibus.keys]
    pub fn released(&mut self, keyval: u32, keycode: u32) -> bool {
        self.pressed
            .remove(&identity(keyval, keycode))
            .unwrap_or(false)
    }

    /// Forgets the keys seen held, whose releases may go elsewhere.
    // [spec:kbdgen:sem:linux.ibus.modifiers]
    pub fn forget(&mut self) {
        self.held.clear();
        self.pressed.clear();
    }

    /// Which of two keys is down when `down` says one is: the left one
    /// when the right one was not seen.
    fn sides(&self, down: bool, left: u32, right: u32) -> (bool, bool) {
        if !down {
            return (false, false);
        }
        let right = self.held.contains(&right);
        (self.held.contains(&left) || !right, right)
    }

    /// The engine's modifiers for a key pressed in `state`. AltGr is Right
    /// Alt as `alt_r` with `altgr`, when `ISO_Level3_Shift` set MOD5 or
    /// when Right Alt is held and the model uses Level3. Left Alt, and an
    /// Alt with no Right Alt held, is `alt_l`, which the engine passes on
    /// Linux. A bound Caps Lock or `B00` held is its `extra`; a bound Right
    /// Ctrl stays `ctrl_r`, which the engine rebinds.
    // [spec:kbdgen:sem:linux.ibus.modifiers]
    pub fn modifiers(&self, state: u32, facts: &Facts) -> ModifierState {
        let (shift_l, shift_r) = self.sides(state & SHIFT_MASK != 0, LEFT_SHIFT, RIGHT_SHIFT);
        let (ctrl_l, ctrl_r) = self.sides(state & CONTROL_MASK != 0, LEFT_CTRL, RIGHT_CTRL);
        let right_alt = self.held.contains(&RIGHT_ALT);
        let alt = state & MOD1_MASK != 0;
        let altgr = state & MOD5_MASK != 0 || (right_alt && facts.uses_level3);
        let mut extra = [false; 3];
        for (slot, key) in extra.iter_mut().zip(&facts.windows.extra_modifiers) {
            *slot = match key {
                ExtraModifierKey::CapsLock => self.held.contains(&CAPS_LOCK),
                ExtraModifierKey::B00 => self.held.contains(&B00),
                ExtraModifierKey::RightCtrl => false,
            };
        }
        ModifierState {
            shift_l,
            shift_r,
            caps: if facts.windows.shift_lock {
                self.shift_lock
            } else {
                state & LOCK_MASK != 0
            },
            ctrl_l,
            ctrl_r,
            alt_l: alt && (self.held.contains(&LEFT_ALT) || !right_alt),
            alt_r: altgr || (alt && right_alt),
            altgr,
            cmd: state & (MOD4_MASK | SUPER_MASK) != 0,
            extra,
        }
    }
}
