//! Key identity (`tsf.keys.identity`): which physical key a key event is,
//! what role it plays, and the engine's modifier state while it is held.
//!
//! Keys are identified by scan code and extended flag, never by virtual
//! key: the virtual key comes from the hidden US dummy layout beneath the
//! text service (`tsf.pairing.substitute`), not from the keyboard.

use kbd_engine::{Action, Context, Key, KeyEvent, Model, ModifierState, State};
use kbd_model::{ExtraModifierKey, Windows};

/// The virtual key of `KEYEVENTF_UNICODE` input.
pub const VK_PACKET: u16 = 0xE7;
/// The virtual key of input another input method has processed.
pub const VK_PROCESSKEY: u16 = 0xE5;

const SCAN_BACKSPACE: u8 = 0x0E;
const SCAN_DECIMAL: u8 = 0x53;
const SCAN_LEFT_SHIFT: u8 = 0x2A;
const SCAN_RIGHT_SHIFT: u8 = 0x36;
const SCAN_CTRL: u8 = 0x1D;
const SCAN_ALT: u8 = 0x38;
const SCAN_CAPS_LOCK: u8 = 0x3A;
const SCAN_LEFT_WIN: u8 = 0x5B;
const SCAN_RIGHT_WIN: u8 = 0x5C;
const SCAN_B00: u8 = 0x56;

/// The scan codes of the 49 ISO positions of `kbdl.scancodes.iso`, the
/// positions the layout DLL's character table covers.
pub const ISO_SCAN_CODES: [u8; 49] = [
    0x29, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x10, 0x11, 0x12,
    0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x1B, 0x1E, 0x1F, 0x20, 0x21, 0x22, 0x23, 0x24,
    0x25, 0x26, 0x27, 0x28, 0x2B, 0x56, 0x2C, 0x2D, 0x2E, 0x2F, 0x30, 0x31, 0x32, 0x33, 0x34, 0x35,
    0x73,
];

/// A key event as TSF reports it: the virtual key (`wParam`) and the low
/// 32 bits of `lParam`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stroke {
    pub vk: u16,
    pub lparam: u32,
}

impl Stroke {
    /// `lParam` bits 16–23.
    pub fn scan(self) -> u8 {
        self.lparam.to_le_bytes()[2]
    }

    /// `lParam` bit 24.
    pub fn extended(self) -> bool {
        self.lparam & (1 << 24) != 0
    }

    /// `lParam` bit 30: the key was already down.
    pub fn repeat(self) -> bool {
        self.lparam & (1 << 30) != 0
    }

    /// The physical key: scan code and extended flag.
    pub fn key(self) -> u16 {
        u16::from(self.scan()) | (u16::from(self.extended()) << 8)
    }

    /// Completes the key's identity from `mapped`, the
    /// `MAPVK_VK_TO_VSC_EX` mapping of its virtual key through the dummy
    /// layout (`E0` in the high byte for an extended key). WPF reports key
    /// events with no scan code, or without the extended flag; for every
    /// key the US dummy layout maps, the mapping back gives the physical
    /// key, and it tells the navigation keys from the numeric keypad's.
    // [spec:kbdgen:req:tsf.keys.recover]
    pub fn restored(self, mapped: u32) -> Stroke {
        let [scan, prefix, ..] = mapped.to_le_bytes();
        let extended = u32::from(prefix == 0xE0);
        let lparam = if self.scan() == 0 {
            self.lparam | (u32::from(scan) << 16) | (extended << 24)
        } else if self.scan() == scan {
            self.lparam | (extended << 24)
        } else {
            self.lparam
        };
        Stroke {
            vk: self.vk,
            lparam,
        }
    }
}

/// The base of the GUIDs of the AltGr preserved keys; a [`Chord`] adds
/// Ctrl, its scan code and Shift to the low bits.
pub const ALTGR_CHORDS: u128 = 0x553A_5A99_9E37_4F9D_A956_F34A_5544_0000;

/// An AltGr chord: Right Alt, optionally Shift, and the key at `scan`.
///
/// TSF hands a text service no key event while Alt is held, with or
/// without Ctrl, so the US dummy layout's Right Alt never reaches the key
/// event sink, nor does a Ctrl+Alt chord. Each chord that the engine does
/// not pass is registered as a preserved key instead, and each that types
/// once more under Ctrl and Alt (`tsf.keys.ctrl-alt`); TSF reports them
/// with `OnPreservedKey`.
// [spec:kbdgen:req:tsf.keys.preserved+1]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Chord {
    pub scan: u8,
    pub shift: bool,
}

impl Chord {
    /// The GUID of the chord's preserved key: under Right Alt, or under
    /// Ctrl and Alt when `ctrl` is set.
    pub fn guid(self, ctrl: bool) -> u128 {
        ALTGR_CHORDS
            | (u128::from(ctrl) << 9)
            | (u128::from(self.scan) << 1)
            | u128::from(self.shift)
    }

    /// The chord of a preserved key's GUID, under either modifier.
    pub fn from_guid(guid: u128) -> Option<Chord> {
        let offset = guid.checked_sub(ALTGR_CHORDS).filter(|o| *o < 0x400)?;
        Some(Chord {
            scan: u8::try_from((offset >> 1) & 0xFF).ok()?,
            shift: offset & 1 == 1,
        })
    }
}

/// The AltGr and AltGr+Shift chords of the ISO positions, by the engine's
/// action for each from `State::default()` with an empty context.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AltGrChords {
    /// The chords the engine does not pass, registered as preserved keys
    /// under Right Alt (`tsf.keys.preserved`).
    pub preserved: Vec<Chord>,
    /// The chords that type: that give a character or a dead key, as a
    /// layout DLL's character table cell does. Right Alt is AltGr exactly
    /// when there is one (`kbdl.locale`), and Ctrl+Alt types only these,
    /// registered as preserved keys under Ctrl and Alt (`tsf.keys.ctrl-alt`).
    pub typed: Vec<Chord>,
}

impl AltGrChords {
    // [spec:kbdgen:req:tsf.keys.altgr+2]
    // [spec:kbdgen:req:tsf.keys.preserved+1]
    // [spec:kbdgen:req:tsf.keys.ctrl-alt]
    pub fn of(model: &Model) -> AltGrChords {
        let start = State::default();
        let context = Context::default();
        let mut chords = AltGrChords::default();
        for shift in [false, true] {
            let modifiers = ModifierState {
                shift_l: shift,
                ..ModifierState::altgr()
            };
            for &scan in &ISO_SCAN_CODES {
                let chord = Chord { scan, shift };
                let event = KeyEvent::with(Key::Scan(scan), modifiers);
                match model.key(&start, &context, &event) {
                    (Action::Pass, _) => continue,
                    (Action::Edit { insert, .. }, state)
                        if !insert.is_empty() || state != start =>
                    {
                        chords.typed.push(chord);
                    }
                    _ => {}
                }
                chords.preserved.push(chord);
            }
        }
        chords
    }
}

/// The modifiers to send for `key` when a Ctrl and an Alt are held
/// together (`tsf.keys.ctrl-alt`), as the layout DLL's `KBDCTRL | KBDALT`
/// column has them: a key at an ISO position whose chord is in `typed`
/// gets the AltGr state, with Shift, Caps Lock, Win and the extra
/// modifiers kept; any other key at an ISO position passes (`None`), so the
/// application's Ctrl+Alt shortcut works. A Right Ctrl bound as an extra
/// modifier, a bound `B00`, and keys at no ISO position are left alone.
// [spec:kbdgen:req:tsf.keys.ctrl-alt]
pub fn ctrl_alt(
    key: &Key,
    m: ModifierState,
    windows: &Windows,
    typed: &[Chord],
) -> Option<ModifierState> {
    let bound = |k: ExtraModifierKey| windows.extra_modifiers.contains(&k);
    let ctrl_r = m.ctrl_r && !bound(ExtraModifierKey::RightCtrl);
    let held = (m.ctrl_l || ctrl_r) && (m.alt_l || m.alt_r);
    let scan = match key {
        &Key::Scan(scan) if !(scan == SCAN_B00 && bound(ExtraModifierKey::B00)) => scan,
        _ => return Some(m),
    };
    if !held || !ISO_SCAN_CODES.contains(&scan) {
        return Some(m);
    }
    let chord = Chord {
        scan,
        shift: m.shift_l || m.shift_r,
    };
    typed.contains(&chord).then_some(ModifierState {
        ctrl_l: false,
        ctrl_r: m.ctrl_r && !ctrl_r,
        alt_l: false,
        alt_r: true,
        altgr: true,
        ..m
    })
}

/// What a key down is to the text service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Role {
    /// Input the text service injected itself (`tsf.edit.inject`): it
    /// passes and changes nothing.
    Own,
    /// Shift, Ctrl, Alt, Win or Caps Lock alone: it passes without a reset.
    Modifier,
    /// Right Alt while it is AltGr (`tsf.keys.altgr`): it is eaten, so the
    /// application never sees a lone Alt.
    AltGr,
    /// An extended key, `VK_PACKET` or `VK_PROCESSKEY`: it passes and resets.
    Other,
    /// A key for the engine.
    Engine(Key),
}

/// Classifies a key down. `own` says that its message carries the
/// injection signature, and `altgr` that Right Alt is AltGr.
// [spec:kbdgen:req:tsf.keys.identity+1]
// [spec:kbdgen:req:tsf.pairing.self-sufficient]
pub fn classify(stroke: Stroke, own: bool, altgr: bool) -> Role {
    let scan = stroke.scan();
    let extended = stroke.extended();
    if own {
        return Role::Own;
    }
    if stroke.vk == VK_PACKET || stroke.vk == VK_PROCESSKEY {
        return Role::Other;
    }
    match (scan, extended) {
        (SCAN_ALT, true) if altgr => Role::AltGr,
        (SCAN_LEFT_SHIFT | SCAN_RIGHT_SHIFT | SCAN_CTRL | SCAN_ALT, _)
        | (SCAN_CAPS_LOCK, false)
        | (SCAN_LEFT_WIN | SCAN_RIGHT_WIN, true) => Role::Modifier,
        (0, _) | (_, true) => Role::Other,
        (SCAN_BACKSPACE, false) => Role::Engine(Key::Backspace),
        (SCAN_DECIMAL, false) => Role::Engine(Key::Decimal),
        (code, false) => Role::Engine(Key::Scan(code)),
    }
}

/// Modifier keys held while a key event is handled, as `GetKeyState`
/// reports them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Held {
    pub shift_l: bool,
    pub shift_r: bool,
    pub ctrl_l: bool,
    pub ctrl_r: bool,
    pub alt_l: bool,
    pub alt_r: bool,
    pub win: bool,
    /// The Caps Lock key is down.
    pub caps_down: bool,
    /// The system's Caps Lock toggle.
    pub caps_on: bool,
}

/// Modifier state the text service follows itself, from the key events it
/// sees: `B00` and Right Alt held, a Left Ctrl synthesised with Right Alt,
/// and the Shift Lock state of `tsf.keys.locale-flags`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Tracker {
    b00: bool,
    right_alt: bool,
    left_ctrl_since: Option<u32>,
    left_ctrl_synthesised: bool,
    shift_lock: bool,
}

impl Tracker {
    /// Follows a key down or up whose message has time `time`. Seeing the
    /// same event twice, as `OnTestKeyDown` then `OnKeyDown`, changes
    /// nothing more.
    // [spec:kbdgen:req:tsf.keys.altgr+2]
    // [spec:kbdgen:req:tsf.keys.locale-flags]
    pub fn observe(&mut self, stroke: Stroke, down: bool, time: u32) {
        match (stroke.scan(), stroke.extended()) {
            (SCAN_B00, false) => self.b00 = down,
            (SCAN_ALT, true) => {
                self.right_alt = down;
                if down && self.left_ctrl_since == Some(time) {
                    self.left_ctrl_synthesised = true;
                }
            }
            (SCAN_CTRL, false) if down => {
                self.left_ctrl_since.get_or_insert(time);
            }
            (SCAN_CTRL, false) => {
                self.left_ctrl_since = None;
                self.left_ctrl_synthesised = false;
            }
            (SCAN_CAPS_LOCK, false) if down => self.shift_lock = true,
            (SCAN_LEFT_SHIFT | SCAN_RIGHT_SHIFT, _) if down => self.shift_lock = false,
            _ => {}
        }
    }

    /// The engine's modifiers. A Left Ctrl the system synthesised with
    /// Right Alt (same message time) is not `ctrl_l`; `caps` follows the
    /// text service's own Shift Lock when the model sets `shift_lock`; and
    /// a held `capsLock` or `B00` extra-modifier key sets its `extra`.
    /// Right Ctrl stays `ctrl_r`, which the engine itself rebinds
    /// (`ldml.engine.extra`).
    // [spec:kbdgen:req:tsf.keys.identity+1]
    // [spec:kbdgen:req:tsf.keys.altgr+2]
    // [spec:kbdgen:req:tsf.keys.locale-flags]
    pub fn modifiers(&self, held: Held, windows: &Windows, altgr: bool) -> ModifierState {
        let mut extra = [false; 3];
        for (slot, key) in extra.iter_mut().zip(&windows.extra_modifiers) {
            *slot = match key {
                ExtraModifierKey::B00 => self.b00,
                ExtraModifierKey::CapsLock => held.caps_down,
                ExtraModifierKey::RightCtrl => false,
            };
        }
        let alt_r = held.alt_r || self.right_alt;
        ModifierState {
            shift_l: held.shift_l,
            shift_r: held.shift_r,
            caps: if windows.shift_lock {
                self.shift_lock
            } else {
                held.caps_on
            },
            ctrl_l: held.ctrl_l && !(self.left_ctrl_synthesised && alt_r),
            ctrl_r: held.ctrl_r,
            alt_l: held.alt_l,
            alt_r,
            altgr,
            cmd: held.win,
            extra,
        }
    }
}
