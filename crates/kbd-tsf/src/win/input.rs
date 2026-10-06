//! The key event sink: key identity, claims and the engine round trip of
//! each key (`tsf.keys.*`), and the AltGr chords TSF reports as preserved
//! keys.

use kbd_engine::KeyEvent;
use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, GetKeyboardLayout, MAPVK_VK_TO_VSC_EX, MapVirtualKeyExW, VIRTUAL_KEY, VK_CAPITAL,
    VK_LCONTROL, VK_LMENU, VK_LSHIFT, VK_LWIN, VK_RCONTROL, VK_RMENU, VK_RSHIFT, VK_RWIN,
};
use windows::Win32::UI::TextServices::{
    GUID_COMPARTMENT_EMPTYCONTEXT, GUID_COMPARTMENT_KEYBOARD_DISABLED, ITfCompartmentMgr,
    ITfContext, ITfKeyEventSink_Impl, TS_SD_READONLY, TS_SS_TRANSITORY,
};
use windows::Win32::UI::WindowsAndMessaging::{GetMessageExtraInfo, GetMessageTime};
use windows_core::{BOOL, GUID, Interface, Ref, Result};

use crate::claim::{Blocked, Route, route};
use crate::document::Flags;
use crate::guard::{Entry, POISON, contain, poisoned};
use crate::keys::{Chord, Held, Role, Stroke, classify};

use super::session::{Outcome, Pending, SIGNATURE, Task, mask_menu};
use super::tip::Tip_Impl;

/// Whether the test or the commit half of a key down is being handled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Test,
    Commit,
}

fn held() -> Held {
    let state = |vk: VIRTUAL_KEY| unsafe { GetKeyState(i32::from(vk.0)) };
    let down = |vk: VIRTUAL_KEY| state(vk) < 0;
    Held {
        shift_l: down(VK_LSHIFT),
        shift_r: down(VK_RSHIFT),
        ctrl_l: down(VK_LCONTROL),
        ctrl_r: down(VK_RCONTROL),
        alt_l: down(VK_LMENU),
        alt_r: down(VK_RMENU),
        win: down(VK_LWIN) || down(VK_RWIN),
        caps_down: down(VK_CAPITAL),
        caps_on: state(VK_CAPITAL) & 1 != 0,
    }
}

/// The key event of the message being handled, whether the text service
/// injected it, and the message time.
// [spec:kbdgen:req:tsf.keys.identity]
fn current(wparam: WPARAM, lparam: LPARAM) -> (Stroke, bool, u32) {
    let stroke = Stroke {
        vk: wparam.0 as u16,
        lparam: lparam.0 as u32,
    };
    let layout = unsafe { GetKeyboardLayout(0) };
    let mapped =
        unsafe { MapVirtualKeyExW(u32::from(stroke.vk), MAPVK_VK_TO_VSC_EX, Some(layout)) };
    let stroke = stroke.restored(mapped);
    let own = usize::try_from(unsafe { GetMessageExtraInfo() }.0) == Ok(SIGNATURE);
    let time = u32::from_ne_bytes(unsafe { GetMessageTime() }.to_ne_bytes());
    (stroke, own, time)
}

fn compartment_set(context: &ITfContext, guid: &GUID) -> bool {
    context
        .cast::<ITfCompartmentMgr>()
        .and_then(|mgr| unsafe { mgr.GetCompartment(guid) })
        .and_then(|compartment| unsafe { compartment.GetValue() })
        .is_ok_and(|value| i32::try_from(&value).is_ok_and(|v| v != 0))
}

/// What a key needs from its context: why it would pass, and the flags.
/// A password field is keyboard-disabled or empty (`tsf.security.disabled`).
// [spec:kbdgen:req:tsf.security.disabled+1]
// [spec:kbdgen:req:tsf.edit.session]
fn examine(context: &ITfContext, inert: bool) -> (Blocked, Flags) {
    let status = unsafe { context.GetStatus() }.unwrap_or_default();
    let blocked = Blocked {
        poisoned: poisoned(),
        inert,
        read_only: status.dwDynamicFlags & TS_SD_READONLY != 0,
    };
    let flags = Flags {
        transitory: status.dwStaticFlags & TS_SS_TRANSITORY != 0,
        disabled: compartment_set(context, &GUID_COMPARTMENT_KEYBOARD_DISABLED)
            || compartment_set(context, &GUID_COMPARTMENT_EMPTYCONTEXT),
    };
    (blocked, flags)
}

impl Tip_Impl {
    /// The engine event for `key` with the modifiers held now, if a
    /// keyboard is loaded.
    fn event(&self, key: kbd_engine::Key, repeat: bool) -> Option<KeyEvent> {
        let inner = self.shared.try_borrow().ok()?;
        let keyboard = inner.keyboard.as_ref()?;
        let windows = &keyboard.model.keyboard().windows;
        let modifiers = inner.tracker.modifiers(held(), windows, keyboard.altgr);
        Some(KeyEvent {
            key,
            modifiers,
            repeat,
        })
    }

    /// Handles a key down; returns whether it is eaten. `OnTestKeyDown`
    /// decides without changing the document and `OnKeyDown` commits the
    /// same decision. TSF calls `OnKeyDown` only for keys `OnTestKeyDown`
    /// ate, so a key that passes commits and resets in the test half.
    // [spec:kbdgen:req:tsf.keys.claim]
    // [spec:kbdgen:req:tsf.keys.identity]
    // [spec:kbdgen:req:tsf.keys.altgr]
    // [spec:kbdgen:req:tsf.pairing.self-sufficient]
    // [spec:kbdgen:req:tsf.component.self-contained]
    fn key_down(
        &self,
        context: Option<&ITfContext>,
        wparam: WPARAM,
        lparam: LPARAM,
        phase: Phase,
    ) -> bool {
        let (stroke, own, time) = current(wparam, lparam);
        let id = (stroke.vk, stroke.key(), time);
        let (role, inert, pending) = {
            let Ok(mut inner) = self.shared.try_borrow_mut() else {
                return false;
            };
            if !own {
                inner.tracker.observe(stroke, true, time);
                inner.injected = false;
                inner.echoes = 0;
            }
            let altgr = inner.keyboard.as_ref().is_some_and(|k| k.altgr);
            let pending = match phase {
                Phase::Commit => inner.pending.take().filter(|p| p.id == id),
                Phase::Test => None,
            };
            let role = classify(stroke, own, altgr);
            (role, inner.keyboard.is_none(), pending)
        };
        let Some(context) = context else {
            return false;
        };
        let (blocked, flags) = examine(context, inert);
        let eaten = match (route(&role, blocked), pending, role) {
            (Route::Ignore, _, _) => false,
            (Route::Eat, _, _) => true,
            (_, Some(Pending { decision: None, .. }), _) => false,
            (
                _,
                Some(Pending {
                    decision: Some(decision),
                    ..
                }),
                _,
            ) => matches!(
                self.request(context, Task::Apply(decision), true),
                Outcome::Eaten(true)
            ),
            (Route::Reset, None, _) => {
                self.request(context, Task::Pass(flags), true);
                self.remember(phase, Pending { id, decision: None });
                false
            }
            (Route::Engine, None, Role::Engine(key)) => match self.event(key, stroke.repeat()) {
                Some(event) => self.engine_key(context, event, flags, phase, id),
                None => false,
            },
            (Route::Engine, None, _) => false,
        };
        if !own && let Ok(mut inner) = self.shared.try_borrow_mut() {
            inner.claims.down(stroke.key(), eaten);
        }
        eaten
    }

    /// Asks the engine about `event`: in the test half the decision is
    /// kept for the commit half; a pass is handled at once.
    fn engine_key(
        &self,
        context: &ITfContext,
        event: KeyEvent,
        flags: Flags,
        phase: Phase,
        id: (u16, u16, u32),
    ) -> bool {
        match phase {
            Phase::Commit => matches!(
                self.request(context, Task::Key(event, flags), true),
                Outcome::Eaten(true)
            ),
            Phase::Test => {
                let Outcome::Decided(decision) =
                    self.request(context, Task::Decide(event, flags), false)
                else {
                    return false;
                };
                let eats = decision.eats();
                if !eats {
                    self.request(context, Task::Pass(flags), true);
                }
                let decision = eats.then_some(decision);
                self.remember(phase, Pending { id, decision });
                eats
            }
        }
    }

    fn remember(&self, phase: Phase, pending: Pending) {
        if phase == Phase::Test
            && let Ok(mut inner) = self.shared.try_borrow_mut()
        {
            inner.pending = Some(pending);
        }
    }

    /// An AltGr chord that TSF reports as a preserved key. When it is
    /// eaten, a mask key follows, so that the application does not take
    /// the lone Alt it has seen as a request for its menu bar.
    // [spec:kbdgen:req:tsf.keys.altgr]
    // [spec:kbdgen:req:tsf.keys.claim]
    fn altgr_key(&self, context: Option<&ITfContext>, chord: Chord) -> bool {
        let Some(context) = context else {
            return false;
        };
        let inert = self
            .shared
            .try_borrow()
            .map_or(true, |inner| inner.keyboard.is_none());
        let (blocked, flags) = examine(context, inert);
        let stroke = Stroke {
            vk: 0,
            lparam: u32::from(chord.scan) << 16,
        };
        let role = classify(stroke, false, false);
        if route(&role, blocked) != Route::Engine {
            return false;
        }
        let Role::Engine(key) = role else {
            return false;
        };
        let Some(event) = self.event(key, false) else {
            return false;
        };
        let eaten = matches!(
            self.request(context, Task::Key(event, flags), true),
            Outcome::Eaten(true)
        );
        if eaten {
            mask_menu();
        }
        eaten
    }

    /// Whether a key up is eaten: exactly when its key down was.
    // [spec:kbdgen:req:tsf.keys.claim]
    fn key_up(&self, wparam: WPARAM, lparam: LPARAM, phase: Phase) -> bool {
        let (stroke, own, time) = current(wparam, lparam);
        let Ok(mut inner) = self.shared.try_borrow_mut() else {
            return false;
        };
        if own {
            return false;
        }
        inner.tracker.observe(stroke, false, time);
        match phase {
            Phase::Test => inner.claims.holds(stroke.key()),
            Phase::Commit => inner.claims.up(stroke.key()),
        }
    }
}

impl ITfKeyEventSink_Impl for Tip_Impl {
    fn OnSetFocus(&self, foreground: BOOL) -> Result<()> {
        contain(&POISON, Entry::OnKeySetFocus, || {
            if !foreground.as_bool() {
                self.reset_document();
            }
            Ok(())
        })
    }

    fn OnTestKeyDown(&self, pic: Ref<ITfContext>, wparam: WPARAM, lparam: LPARAM) -> Result<BOOL> {
        contain(&POISON, Entry::OnTestKeyDown, || {
            Ok(self
                .key_down(pic.as_ref(), wparam, lparam, Phase::Test)
                .into())
        })
    }

    fn OnTestKeyUp(&self, _pic: Ref<ITfContext>, wparam: WPARAM, lparam: LPARAM) -> Result<BOOL> {
        contain(&POISON, Entry::OnTestKeyUp, || {
            Ok(self.key_up(wparam, lparam, Phase::Test).into())
        })
    }

    fn OnKeyDown(&self, pic: Ref<ITfContext>, wparam: WPARAM, lparam: LPARAM) -> Result<BOOL> {
        contain(&POISON, Entry::OnKeyDown, || {
            Ok(self
                .key_down(pic.as_ref(), wparam, lparam, Phase::Commit)
                .into())
        })
    }

    fn OnKeyUp(&self, _pic: Ref<ITfContext>, wparam: WPARAM, lparam: LPARAM) -> Result<BOOL> {
        contain(&POISON, Entry::OnKeyUp, || {
            Ok(self.key_up(wparam, lparam, Phase::Commit).into())
        })
    }

    // [spec:kbdgen:req:tsf.keys.altgr]
    fn OnPreservedKey(&self, pic: Ref<ITfContext>, rguid: *const GUID) -> Result<BOOL> {
        contain(&POISON, Entry::OnPreservedKey, || {
            let chord = unsafe { rguid.as_ref() }.and_then(|g| Chord::from_guid(g.to_u128()));
            let eaten = chord.is_some_and(|chord| self.altgr_key(pic.as_ref(), chord));
            Ok(eaten.into())
        })
    }
}
