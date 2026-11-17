//! `kbd-engine` for WebAssembly hosts: the ChromeOS input method runs this
//! crate, built for `wasm32-unknown-unknown`, on the models kbdgen writes.
//!
//! The module exports a C ABI of integers (`abi`), so a host needs nothing
//! but `WebAssembly.instantiate` and the module's `memory`; there is no
//! generated JavaScript. Text crosses the boundary as UTF-8 in two buffers
//! the module owns: the host asks for an input buffer of a given length
//! (`kbd_input`), writes into it and makes the call that reads it; results
//! are read from the output buffer (`kbd_output`, `kbd_output_len`) before
//! the next call. Any call may grow the memory, so a host takes a fresh
//! view of `memory.buffer` after each one.
//!
//! A loaded model is a *session*: the model with the engine `State` the
//! host would otherwise own (`ldml.engine.state`), named by a handle ≥ 1.
//! `kbd_key` runs `Model::key` with that state and keeps the next state,
//! so a host only resets it (`kbd_reset`) when the engine contract asks
//! it to. All of this lives in one thread-local value, which on
//! `wasm32-unknown-unknown` is simply the module's, and which gives every
//! native test thread its own.
//!
//! Nothing here panics: every failure is a return code, and a model that
//! does not load leaves its error message in the output buffer.

// [spec:kbdgen:req:ldml.chromeos.wasm]
// [spec:kbdgen:req:ldml.crate.ffi+1]
#![deny(unsafe_code)]

mod abi;
#[cfg(test)]
mod tests;

use std::cell::RefCell;

use kbd_engine::{
    Action, BackspacePolicy, Context, Key, KeyEvent, Model, ModifierState, Options, OutputForm,
    State,
};
use kbd_model::{ExtraModifierKey, Host};

/// `kbd_key`'s key: a scan code below `0x100` is `Key::Scan`.
pub const KEY_DECIMAL: u32 = 0x100;
pub const KEY_BACKSPACE: u32 = 0x101;
pub const KEY_COMMIT: u32 = 0x102;

/// `kbd_key`'s modifier bits, one per `ModifierState` field.
pub const SHIFT_L: u32 = 1;
pub const SHIFT_R: u32 = 1 << 1;
pub const CAPS: u32 = 1 << 2;
pub const CTRL_L: u32 = 1 << 3;
pub const CTRL_R: u32 = 1 << 4;
pub const ALT_L: u32 = 1 << 5;
pub const ALT_R: u32 = 1 << 6;
pub const ALTGR: u32 = 1 << 7;
pub const CMD: u32 = 1 << 8;
pub const EXTRA1: u32 = 1 << 9;
pub const EXTRA2: u32 = 1 << 10;
pub const EXTRA3: u32 = 1 << 11;
const ALL_MODIFIERS: u32 = (EXTRA3 << 1) - 1;

/// `kbd_key`'s flag bits.
pub const AT_START: u32 = 1;
pub const AUTHORITATIVE: u32 = 1 << 1;

/// `kbd_key`'s results: an action, or a negative error.
pub const PASS: i32 = 0;
pub const EDIT: i32 = 1;
pub const NO_SUCH_MODEL: i32 = -1;
pub const INVALID_EVENT: i32 = -2;
pub const INVALID_CONTEXT: i32 = -3;
pub const UNAVAILABLE: i32 = -4;

/// The options of the ChromeOS host: NFC output, the macOS Backspace that
/// cancels a pending dead key and otherwise lets the application delete,
/// and `chromeOS` for a keyboard built for no host in particular.
const OPTIONS: Options = Options {
    output_form: OutputForm::Nfc,
    backspace: BackspacePolicy::CancelOrPass,
    host: Some(Host::ChromeOs),
};

struct Session {
    model: Model,
    state: State,
}

#[derive(Default)]
struct Instance {
    /// Slot *i* holds the session with handle *i* + 1.
    sessions: Vec<Option<Session>>,
    input: Vec<u8>,
    /// After an edit, the inserted text followed by the preedit; after a
    /// failed load, the error message.
    output: Vec<u8>,
    insert_len: usize,
    delete: usize,
}

thread_local! {
    static INSTANCE: RefCell<Instance> = RefCell::new(Instance::default());
}

/// Runs `f` on the instance, or gives `fallback` if it cannot be borrowed,
/// which no export does while another runs.
fn with_instance<T>(fallback: T, f: impl FnOnce(&mut Instance) -> T) -> T {
    INSTANCE
        .try_with(|cell| cell.try_borrow_mut().ok().map(|mut i| f(&mut i)))
        .ok()
        .flatten()
        .unwrap_or(fallback)
}

fn decode_key(key: u32) -> Option<Key> {
    match key {
        KEY_DECIMAL => Some(Key::Decimal),
        KEY_BACKSPACE => Some(Key::Backspace),
        KEY_COMMIT => Some(Key::Commit),
        _ => u8::try_from(key).ok().map(Key::Scan),
    }
}

fn decode_modifiers(bits: u32) -> Option<ModifierState> {
    if bits & !ALL_MODIFIERS != 0 {
        return None;
    }
    let held = |bit: u32| bits & bit != 0;
    Some(ModifierState {
        shift_l: held(SHIFT_L),
        shift_r: held(SHIFT_R),
        caps: held(CAPS),
        ctrl_l: held(CTRL_L),
        ctrl_r: held(CTRL_R),
        alt_l: held(ALT_L),
        alt_r: held(ALT_R),
        altgr: held(ALTGR),
        cmd: held(CMD),
        extra: [held(EXTRA1), held(EXTRA2), held(EXTRA3)],
    })
}

impl Instance {
    /// Sizes the input buffer to `len` zero bytes and gives its address.
    fn input(&mut self, len: usize) -> usize {
        self.input.clear();
        self.input.resize(len, 0);
        self.input.as_mut_ptr().expose_provenance()
    }

    fn set_output(&mut self, text: &str) {
        self.output.clear();
        self.output.extend_from_slice(text.as_bytes());
        self.insert_len = self.output.len();
        self.delete = 0;
    }

    fn session(&mut self, handle: u32) -> Option<&mut Session> {
        let slot = usize::try_from(handle).ok()?.checked_sub(1)?;
        self.sessions.get_mut(slot)?.as_mut()
    }

    /// Loads the model in the first `len` bytes of the input buffer into a
    /// new session and gives its handle, or 0 with the error as output.
    fn load(&mut self, len: usize) -> u32 {
        let loaded = match self.input.get(..len) {
            Some(bytes) => Model::from_bytes(bytes).map_err(|e| e.to_string()),
            None => Err(format!(
                "the model is {len} bytes, but the input buffer holds {}",
                self.input.len()
            )),
        };
        let model = match loaded {
            Ok(model) => model.with_options(OPTIONS),
            Err(message) => {
                self.set_output(&message);
                return 0;
            }
        };
        let session = Some(Session {
            model,
            state: State::default(),
        });
        let slot = match self.sessions.iter().position(Option::is_none) {
            Some(slot) => {
                if let Some(free) = self.sessions.get_mut(slot) {
                    *free = session;
                }
                slot
            }
            None => {
                self.sessions.push(session);
                self.sessions.len().saturating_sub(1)
            }
        };
        slot.checked_add(1)
            .and_then(|handle| u32::try_from(handle).ok())
            .unwrap_or(0)
    }

    fn unload(&mut self, handle: u32) {
        let slot = usize::try_from(handle)
            .ok()
            .and_then(|h| h.checked_sub(1))
            .and_then(|slot| self.sessions.get_mut(slot));
        if let Some(slot) = slot {
            *slot = None;
        }
    }

    /// Runs `Model::key` on the session's state with the context in the
    /// first `context_len` bytes of the input buffer, keeping the next
    /// state. An edit leaves its texts as output.
    fn key(
        &mut self,
        handle: u32,
        key: u32,
        modifiers: u32,
        context_len: usize,
        flags: u32,
    ) -> i32 {
        let (Some(key), Some(modifiers)) = (decode_key(key), decode_modifiers(modifiers)) else {
            return INVALID_EVENT;
        };
        let Some(text) = self
            .input
            .get(..context_len)
            .and_then(|bytes| std::str::from_utf8(bytes).ok())
        else {
            return INVALID_CONTEXT;
        };
        let context = Context {
            text: text.to_owned(),
            authoritative: flags & AUTHORITATIVE != 0,
            at_start: flags & AT_START != 0,
        };
        let Some(session) = self.session(handle) else {
            return NO_SUCH_MODEL;
        };
        let event = KeyEvent::with(key, modifiers);
        let (action, state) = session.model.key(&session.state, &context, &event);
        session.state = state;
        match action {
            Action::Pass => {
                self.set_output("");
                PASS
            }
            Action::Edit {
                delete,
                insert,
                preedit,
                ..
            } => {
                self.set_output(&insert);
                self.output.extend_from_slice(preedit.as_bytes());
                self.delete = delete;
                EDIT
            }
        }
    }

    fn reset(&mut self, handle: u32) {
        if let Some(session) = self.session(handle) {
            session.state = State::default();
        }
    }

    /// How many markers are pending (`Model::pending_markers`): what a
    /// `Commit` would flush. Markers elsewhere in the state change no text.
    fn pending(&mut self, handle: u32) -> u32 {
        self.session(handle)
            .and_then(|s| u32::try_from(s.model.pending_markers(&s.state).len()).ok())
            .unwrap_or(0)
    }

    fn context_len(&mut self, handle: u32) -> u32 {
        self.session(handle)
            .and_then(|s| u32::try_from(s.model.context_len()).ok())
            .unwrap_or(0)
    }

    /// The keys bound as extra modifiers, two bits per component from
    /// `extra1` up: 0 none, 1 Right Ctrl, 2 Caps Lock, 3 `B00`.
    fn extra_modifiers(&mut self, handle: u32) -> u32 {
        let Some(session) = self.session(handle) else {
            return 0;
        };
        session
            .model
            .keyboard()
            .windows
            .extra_modifiers
            .iter()
            .take(3)
            .zip([0u32, 2, 4])
            .map(|(key, shift)| {
                let code: u32 = match key {
                    ExtraModifierKey::RightCtrl => 1,
                    ExtraModifierKey::CapsLock => 2,
                    ExtraModifierKey::B00 => 3,
                };
                code << shift
            })
            .fold(0, |packed, bits| packed | bits)
    }

    fn shift_lock(&mut self, handle: u32) -> u32 {
        self.session(handle)
            .map_or(0, |s| u32::from(s.model.keyboard().windows.shift_lock))
    }
}
