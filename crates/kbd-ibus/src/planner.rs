//! The host logic of one engine object (`linux.ibus.planner`), with no
//! D-Bus calls: what each call ibus-daemon makes does to the engine state,
//! and which signals it emits, in order, before its reply. `crate::dbus`
//! only decodes the calls and encodes the signals; a Fcitx5 addon or a
//! Stevia completer would drive the same [`Session`].

use kbd_engine::{
    Action, BackspacePolicy, Context, Key, KeyEvent, Model, ModifierState, Options, OutputForm,
    State,
};
use kbd_model::Host;

use crate::keys::{self, Facts, Keys, Role};

/// `IBUS_ENGINE_PREEDIT_*`: what the client does with a shown preedit when
/// the input context resets or loses focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreeditMode {
    /// Drop it.
    Clear = 0,
    /// Commit it.
    Commit = 1,
}

/// A signal of `org.freedesktop.IBus.Engine`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[expect(
    clippy::enum_variant_names,
    reason = "the variants are named as the D-Bus signals"
)]
pub enum Signal {
    CommitText(String),
    /// The preedit, its cursor in scalar values, whether it is shown.
    UpdatePreeditText {
        text: String,
        cursor: u32,
        visible: bool,
        mode: PreeditMode,
    },
    /// Delete `nchars` scalar values from `offset` relative to the caret.
    DeleteSurroundingText {
        offset: i32,
        nchars: u32,
    },
    #[expect(
        dead_code,
        reason = "surrounding text (linux.ibus.context) asks for it"
    )]
    RequireSurroundingText,
}

/// What a `ProcessKeyEvent` call gives: the signals, then the reply.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Reply {
    /// The engine handled the key, so the client does not type it.
    pub handled: bool,
    pub signals: Vec<Signal>,
}

/// The engine's options for every session: NFC output, the macOS
/// Backspace, and host `linux`.
// [spec:kbdgen:req:linux.ibus.planner]
pub const OPTIONS: Options = Options {
    output_form: OutputForm::Nfc,
    backspace: BackspacePolicy::CancelOrPass,
    host: Some(Host::Linux),
};

fn count(text: &str) -> u32 {
    u32::try_from(text.chars().count()).unwrap_or(u32::MAX)
}

/// One engine object's state.
#[derive(Debug)]
pub struct Session {
    model: Model,
    facts: Facts,
    state: State,
    keys: Keys,
    /// The preedit shown, empty for none.
    shown: String,
}

impl Session {
    pub fn new(model: Model) -> Session {
        let model = model.with_options(OPTIONS);
        let facts = Facts::of(&model);
        Session {
            model,
            facts,
            state: State::default(),
            keys: Keys::default(),
            shown: String::new(),
        }
    }

    fn context(&self) -> Context {
        Context::default()
    }

    /// A key event. A press is classified by its key code; a release
    /// returns what its press did; an event IBus forwarded passes.
    // [spec:kbdgen:req:linux.ibus.planner]
    // [spec:kbdgen:req:linux.ibus.keys]
    pub fn process_key_event(&mut self, keyval: u32, keycode: u32, state: u32) -> Reply {
        if state & keys::FORWARD_MASK != 0 {
            return Reply::default();
        }
        let press = state & keys::RELEASE_MASK == 0;
        if keycode != 0 {
            self.keys.observe(keycode, press);
        }
        if !press {
            return Reply {
                handled: self.keys.released(keyval, keycode),
                signals: Vec::new(),
            };
        }
        let reply = match keys::classify(keyval, keycode) {
            Role::Modifier => Reply::default(),
            Role::Frame => self.pass(),
            Role::Engine(key) => {
                let modifiers = if keycode == 0 {
                    ModifierState::default()
                } else {
                    self.keys.modifiers(state, &self.facts)
                };
                self.engine_key(KeyEvent::with(key, modifiers))
            }
        };
        self.keys.pressed(keyval, keycode, reply.handled);
        reply
    }

    fn engine_key(&mut self, event: KeyEvent) -> Reply {
        let (action, next) = self.model.key(&self.state, &self.context(), &event);
        match action {
            Action::Pass => self.pass(),
            Action::Edit {
                delete,
                insert,
                preedit,
                ..
            } => {
                let signals = self.apply(delete, &insert, &preedit);
                self.state = next;
                Reply {
                    handled: true,
                    signals,
                }
            }
        }
    }

    /// A key that passes: what is pending is committed by the edit of a
    /// `Commit`, then the state resets and the client types the key.
    // [spec:kbdgen:req:linux.ibus.planner]
    fn pass(&mut self) -> Reply {
        let mut signals = Vec::new();
        if !self.shown.is_empty() || !self.model.pending_markers(&self.state).is_empty() {
            let commit = KeyEvent::new(Key::Commit);
            if let (
                Action::Edit {
                    delete,
                    insert,
                    preedit,
                    ..
                },
                _,
            ) = self.model.key(&self.state, &self.context(), &commit)
            {
                signals = self.apply(delete, &insert, &preedit);
            }
        }
        self.state = State::default();
        Reply {
            handled: false,
            signals,
        }
    }

    /// The signals of an edit (*d*, *s*, *p*), in the order ibus-daemon
    /// applies them: the shown preedit cleared when text changes, the
    /// deletion, the insertion, then the new preedit.
    // [spec:kbdgen:req:linux.ibus.planner]
    fn apply(&mut self, delete: usize, insert: &str, preedit: &str) -> Vec<Signal> {
        let mut signals = Vec::new();
        if !self.shown.is_empty() && (delete > 0 || !insert.is_empty()) {
            signals.push(Signal::UpdatePreeditText {
                text: String::new(),
                cursor: 0,
                visible: false,
                mode: PreeditMode::Clear,
            });
            self.shown.clear();
        }
        if delete > 0 {
            let nchars = u32::try_from(delete).unwrap_or(u32::MAX);
            signals.push(Signal::DeleteSurroundingText {
                offset: 0i32.saturating_sub_unsigned(nchars),
                nchars,
            });
        }
        if !insert.is_empty() {
            signals.push(Signal::CommitText(insert.to_owned()));
        }
        if preedit != self.shown {
            signals.push(Signal::UpdatePreeditText {
                text: preedit.to_owned(),
                cursor: count(preedit),
                visible: !preedit.is_empty(),
                mode: PreeditMode::Commit,
            });
            preedit.clone_into(&mut self.shown);
        }
        signals
    }

    /// Resets the state; a shown preedit is the client's to commit.
    fn clear(&mut self) {
        self.state = State::default();
        self.shown.clear();
    }

    /// Focus moved, or the engine was switched on or off: the keys seen
    /// held may be released elsewhere.
    // [spec:kbdgen:sem:linux.ibus.modifiers]
    fn refocus(&mut self) -> Vec<Signal> {
        self.keys.forget();
        self.clear();
        Vec::new()
    }

    pub fn focus_in(&mut self) -> Vec<Signal> {
        self.refocus()
    }

    pub fn focus_out(&mut self) -> Vec<Signal> {
        self.refocus()
    }

    pub fn reset(&mut self) -> Vec<Signal> {
        self.clear();
        Vec::new()
    }

    pub fn enable(&mut self) -> Vec<Signal> {
        self.refocus()
    }

    pub fn disable(&mut self) -> Vec<Signal> {
        self.refocus()
    }

    pub fn set_capabilities(&mut self, _capabilities: u32) {}

    pub fn set_content_type(&mut self, _purpose: u32, _hints: u32) {}

    /// A reading of the client's text, positions in scalar values.
    pub fn set_surrounding_text(&mut self, _text: &str, _cursor: u32, _anchor: u32) {}
}
