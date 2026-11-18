//! The host logic of one engine object (`linux.ibus.planner`), with no
//! D-Bus calls: what each call ibus-daemon makes does to the engine state,
//! and which signals it emits, in order, before its reply. `crate::dbus`
//! only decodes the calls and encodes the signals; a Fcitx5 addon or a
//! Stevia completer would drive the same [`Session`].
//!
//! Where the edits go depends on the field (`Mode`): a client that reports
//! its text gets the engine's edits as they are; one that does not gets
//! the engine's own text as the context, and for a model that rewrites
//! committed text the end of it held back as preedit, since such a client
//! cannot be relied on to delete; a password field gets no preedit and no
//! context at all.

use kbd_engine::{
    Action, BackspacePolicy, Context, Key, KeyEvent, Model, ModifierState, Options, OutputForm,
    State,
};
use kbd_model::{Host, MarkerIndex, Text, TextElem};

use crate::context::Surrounding;
use crate::keys::{self, Facts, Keys, Role};

/// `IBUS_CAP_PREEDIT_TEXT` and `IBUS_CAP_SURROUNDING_TEXT`.
pub const CAP_PREEDIT_TEXT: u32 = 1 << 0;
pub const CAP_SURROUNDING_TEXT: u32 = 1 << 5;
/// `IBUS_INPUT_PURPOSE_PASSWORD` and `IBUS_INPUT_PURPOSE_PIN`.
pub const PURPOSE_PASSWORD: u32 = 8;
pub const PURPOSE_PIN: u32 = 9;

/// What stands for a pending dead key with no display of its own.
const DOTTED_CIRCLE: &str = "\u{25CC}";

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
// [spec:kbdgen:sem:linux.ibus.semantics]
pub const OPTIONS: Options = Options {
    output_form: OutputForm::Nfc,
    backspace: BackspacePolicy::CancelOrPass,
    host: Some(Host::Linux),
};

fn count(text: &str) -> u32 {
    u32::try_from(text.chars().count()).unwrap_or(u32::MAX)
}

/// A preedit shown, and what a reset does with it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Preedit {
    text: String,
    mode: PreeditMode,
}

/// Where the engine's context comes from and where its edits go.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// A password or PIN field (`linux.ibus.security`).
    Secure,
    /// The client reports its text (`linux.ibus.context`), and shows a
    /// preedit when `preedit`.
    Surrounding { preedit: bool },
    /// The context is the engine's own edits, the last `held` scalar
    /// values of which are shown as preedit, not committed, where the
    /// client shows a `preedit` (`linux.ibus.context.fallback`).
    Cached { held: usize, preedit: bool },
    /// Neither surrounding text nor a preedit, for a model that rewrites
    /// committed text: the context is empty, so the engine never deletes.
    Bare,
}

/// One engine object's state.
#[derive(Debug)]
pub struct Session {
    model: Model,
    facts: Facts,
    /// The model rewrites committed text (`ldml.linux.rewrites`).
    rewrites: bool,
    state: State,
    keys: Keys,
    capabilities: u32,
    purpose: u32,
    shown: Option<Preedit>,
    surrounding: Surrounding,
    /// The scalar values the engine's own edits produced since the last
    /// reset, at most `context_len`.
    cache: String,
    /// How many of the last scalar values of `cache` are held back.
    held: usize,
}

impl Session {
    pub fn new(model: Model) -> Session {
        let model = model.with_options(OPTIONS);
        let facts = Facts::of(&model);
        let rewrites = !model.keyboard().rewrites().is_empty();
        Session {
            model,
            facts,
            rewrites,
            state: State::default(),
            keys: Keys::default(),
            capabilities: 0,
            purpose: 0,
            shown: None,
            surrounding: Surrounding::default(),
            cache: String::new(),
            held: 0,
        }
    }

    fn shown_text(&self) -> &str {
        self.shown
            .as_ref()
            .map_or("", |preedit| preedit.text.as_str())
    }

    // [spec:kbdgen:req:linux.ibus.security]
    // [spec:kbdgen:sem:linux.ibus.context]
    // [spec:kbdgen:sem:linux.ibus.context.fallback]
    fn mode(&self) -> Mode {
        let preedit = self.capabilities & CAP_PREEDIT_TEXT != 0;
        if matches!(self.purpose, PURPOSE_PASSWORD | PURPOSE_PIN) {
            Mode::Secure
        } else if self.capabilities & CAP_SURROUNDING_TEXT != 0 {
            Mode::Surrounding { preedit }
        } else if preedit {
            Mode::Cached {
                held: if self.rewrites {
                    self.model.context_len()
                } else {
                    0
                },
                preedit,
            }
        } else if self.rewrites {
            Mode::Bare
        } else {
            Mode::Cached { held: 0, preedit }
        }
    }

    /// The context the engine reads. A reading is a window of the text,
    /// not known to start where the text starts, so `at_start` is false.
    // [spec:kbdgen:sem:linux.ibus.context]
    // [spec:kbdgen:sem:linux.ibus.context.fallback]
    fn context(&self, mode: Mode) -> Context {
        match mode {
            Mode::Secure | Mode::Bare => Context::default(),
            Mode::Surrounding { .. } => Context {
                text: self.surrounding.text().to_owned(),
                authoritative: self.surrounding.authoritative(),
                at_start: false,
            },
            Mode::Cached { .. } => Context {
                text: self.cache.clone(),
                authoritative: false,
                at_start: false,
            },
        }
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
        let mode = self.mode();
        let reply = match keys::classify(keyval, keycode) {
            Role::Modifier => Reply::default(),
            Role::Frame => self.pass(mode),
            Role::Engine(key) => {
                let modifiers = if keycode == 0 {
                    ModifierState::default()
                } else {
                    self.keys.modifiers(state, &self.facts)
                };
                self.engine_key(mode, KeyEvent::with(key, modifiers))
            }
        };
        self.keys.pressed(keyval, keycode, reply.handled);
        reply
    }

    /// A key for the engine. One typed over a selection replaces it, so
    /// it starts from the reset state and an empty context.
    // [spec:kbdgen:sem:linux.ibus.semantics]
    // [spec:kbdgen:sem:linux.ibus.context]
    fn engine_key(&mut self, mode: Mode, event: KeyEvent) -> Reply {
        if matches!(mode, Mode::Surrounding { .. }) {
            let shown = self.shown_text().to_owned();
            if self.surrounding.take_selection(&shown) {
                self.state = State::default();
            }
        }
        let (action, next) = self.model.key(&self.state, &self.context(mode), &event);
        match action {
            Action::Pass => self.pass(mode),
            Action::Edit {
                delete,
                insert,
                preedit,
                ..
            } => {
                let signals = self.edit(mode, delete, &insert, &preedit, &next, false);
                self.state = next;
                Reply {
                    handled: true,
                    signals,
                }
            }
        }
    }

    /// A key that passes: if a preedit is shown or markers are pending,
    /// the edit of a `Commit` is applied first, committing any held text
    /// with it; then the state resets and the client types the key.
    // [spec:kbdgen:req:linux.ibus.planner]
    // [spec:kbdgen:sem:linux.ibus.semantics]
    // [spec:kbdgen:sem:linux.ibus.context.fallback]
    fn pass(&mut self, mode: Mode) -> Reply {
        let mut signals = Vec::new();
        if self.shown.is_some() || !self.model.pending_markers(&self.state).is_empty() {
            let commit = KeyEvent::new(Key::Commit);
            let (action, next) = self.model.key(&self.state, &self.context(mode), &commit);
            if let Action::Edit {
                delete,
                insert,
                preedit,
                ..
            } = action
            {
                signals = self.edit(mode, delete, &insert, &preedit, &next, true);
            }
        }
        self.state = State::default();
        self.cache.clear();
        self.held = 0;
        let shown = self.shown_text().to_owned();
        self.surrounding.restart(&shown);
        Reply {
            handled: false,
            signals,
        }
    }

    /// Applies the engine's edit (*d*, *s*, *p*) where `mode` sends it,
    /// leading to `next`. `commit` commits held text, as a key that passes
    /// does.
    fn edit(
        &mut self,
        mode: Mode,
        delete: usize,
        insert: &str,
        preedit: &str,
        next: &State,
        commit: bool,
    ) -> Vec<Signal> {
        let (delete, insert, preedit) = match mode {
            Mode::Cached { held, .. } => {
                self.hold(delete, insert, preedit, if commit { 0 } else { held })
            }
            _ => (delete, insert.to_owned(), preedit.to_owned()),
        };
        let show = self.to_show(mode, preedit, next);
        self.emit(mode, delete, &insert, show)
    }

    /// The edit in the cache: *d* scalar values dropped from it and *s*
    /// appended; of the held text that gives, all but the last `keep` is
    /// committed, and the rest is shown ahead of *p*. The client is asked
    /// to delete only what reaches past the held text, which the engine,
    /// reading no more than `context_len` scalar values, never asks for.
    /// Gives the client's (*d*, *s*, *p*).
    // [spec:kbdgen:sem:linux.ibus.context.fallback]
    fn hold(
        &mut self,
        delete: usize,
        insert: &str,
        preedit: &str,
        keep: usize,
    ) -> (usize, String, String) {
        let cache: Vec<char> = self.cache.chars().collect();
        let held = self.held.min(cache.len());
        let committed = cache.len().saturating_sub(held);
        let past = delete.saturating_sub(held);
        let mut candidate: Vec<char> = cache
            .iter()
            .skip(committed)
            .take(held.saturating_sub(delete))
            .copied()
            .collect();
        candidate.extend(insert.chars());
        let commit_len = candidate.len().saturating_sub(keep);
        let commit: String = candidate.iter().take(commit_len).collect();
        let shown: String = candidate.iter().skip(commit_len).collect();
        let mut full: Vec<char> = cache
            .iter()
            .take(committed.saturating_sub(past))
            .copied()
            .collect();
        full.extend(&candidate);
        let skip = full.len().saturating_sub(self.model.context_len());
        self.cache = full.iter().skip(skip).collect();
        self.held = candidate.len().saturating_sub(commit_len);
        (past, commit, format!("{shown}{preedit}"))
    }

    /// The preedit to show for `preedit` leading to `next`: none where the
    /// field shows none; a pending dead key that shows nothing else
    /// shows its marker's display, else U+25CC, dropped by a reset, so that
    /// GNOME Shell's on-screen Backspace reaches the engine to cancel it.
    // [spec:kbdgen:sem:linux.ibus.osk-backspace]
    // [spec:kbdgen:req:linux.ibus.security]
    fn to_show(&self, mode: Mode, preedit: String, next: &State) -> Option<Preedit> {
        let shows = match mode {
            Mode::Secure | Mode::Bare => false,
            Mode::Surrounding { preedit } | Mode::Cached { preedit, .. } => preedit,
        };
        if !shows {
            return None;
        }
        if !preedit.is_empty() {
            return Some(Preedit {
                text: preedit,
                mode: PreeditMode::Commit,
            });
        }
        let Some(TextElem::Marker(marker)) = next.tail().0.last() else {
            return None;
        };
        Some(Preedit {
            text: self.display(*marker),
            mode: PreeditMode::Clear,
        })
    }

    /// The display of marker `marker` (`ldml.model.displays`), else U+25CC.
    fn display(&self, marker: MarkerIndex) -> String {
        let output = Text(vec![TextElem::Marker(marker)]);
        self.model
            .keyboard()
            .displays
            .for_output(&output)
            .unwrap_or(DOTTED_CIRCLE)
            .to_owned()
    }

    /// The signals of an edit (*d*, *s*) and the preedit to show, in the
    /// order ibus-daemon must apply them: the shown preedit cleared when
    /// text changes, the deletion, the insertion, then the new preedit.
    // [spec:kbdgen:req:linux.ibus.edit]
    // [spec:kbdgen:req:linux.ibus.planner]
    fn emit(
        &mut self,
        mode: Mode,
        delete: usize,
        insert: &str,
        show: Option<Preedit>,
    ) -> Vec<Signal> {
        let surrounding = matches!(mode, Mode::Surrounding { .. });
        let mut signals = Vec::new();
        if self.shown.is_some() && (delete > 0 || !insert.is_empty()) {
            signals.push(Signal::UpdatePreeditText {
                text: String::new(),
                cursor: 0,
                visible: false,
                mode: PreeditMode::Clear,
            });
            self.shown = None;
            if surrounding {
                self.surrounding.shown("");
            }
        }
        if delete > 0 {
            let nchars = u32::try_from(delete).unwrap_or(u32::MAX);
            signals.push(Signal::DeleteSurroundingText {
                offset: 0i32.saturating_sub_unsigned(nchars),
                nchars,
            });
            if surrounding {
                let shown = self.shown_text().to_owned();
                self.surrounding.deleted(delete, &shown);
            }
        }
        if !insert.is_empty() {
            signals.push(Signal::CommitText(insert.to_owned()));
            if surrounding {
                let shown = self.shown_text().to_owned();
                self.surrounding.committed(insert, &shown);
            }
        }
        if show != self.shown {
            let (text, mode) = show
                .as_ref()
                .map_or((String::new(), PreeditMode::Commit), |preedit| {
                    (preedit.text.clone(), preedit.mode)
                });
            signals.push(Signal::UpdatePreeditText {
                cursor: count(&text),
                visible: !text.is_empty(),
                text,
                mode,
            });
            self.shown = show;
            if surrounding {
                let shown = self.shown_text().to_owned();
                self.surrounding.shown(&shown);
            }
        }
        signals
    }

    /// Resets the state and forgets the context and the cache. A shown
    /// preedit is the client's: it commits one shown with
    /// `IBUS_ENGINE_PREEDIT_COMMIT`, which is what `Commit` would insert.
    // [spec:kbdgen:sem:linux.ibus.semantics]
    fn forget(&mut self) {
        self.state = State::default();
        self.shown = None;
        self.cache.clear();
        self.held = 0;
        self.surrounding.forget("");
    }

    /// Focus moved, or the engine was switched on or off: the keys seen
    /// held may be released elsewhere.
    // [spec:kbdgen:sem:linux.ibus.modifiers]
    fn refocus(&mut self) {
        self.keys.forget();
        self.forget();
    }

    pub fn focus_in(&mut self) -> Vec<Signal> {
        self.refocus();
        Vec::new()
    }

    pub fn focus_out(&mut self) -> Vec<Signal> {
        self.refocus();
        Vec::new()
    }

    pub fn reset(&mut self) -> Vec<Signal> {
        self.forget();
        Vec::new()
    }

    /// Asks the client for its text, as libibus's
    /// `ibus_engine_get_surrounding_text` does.
    // [spec:kbdgen:sem:linux.ibus.context]
    pub fn enable(&mut self) -> Vec<Signal> {
        self.refocus();
        vec![Signal::RequireSurroundingText]
    }

    pub fn disable(&mut self) -> Vec<Signal> {
        self.refocus();
        Vec::new()
    }

    pub fn set_capabilities(&mut self, capabilities: u32) {
        self.capabilities = capabilities;
    }

    /// A field's purpose. A password or PIN field keeps no text.
    // [spec:kbdgen:req:linux.ibus.security]
    pub fn set_content_type(&mut self, purpose: u32, _hints: u32) {
        self.purpose = purpose;
        if self.mode() == Mode::Secure {
            self.cache.clear();
            self.held = 0;
            let shown = self.shown_text().to_owned();
            self.surrounding.forget(&shown);
        }
    }

    /// A reading of the client's text, positions in scalar values: the
    /// text before the caret, or before the selection.
    // [spec:kbdgen:sem:linux.ibus.context]
    // [spec:kbdgen:req:linux.ibus.security]
    pub fn set_surrounding_text(&mut self, text: &str, cursor: u32, anchor: u32) {
        if self.mode() == Mode::Secure {
            return;
        }
        let caret = usize::try_from(cursor.min(anchor)).unwrap_or(usize::MAX);
        let reading: String = text.chars().take(caret).collect();
        let shown = self.shown_text().to_owned();
        self.surrounding.read(&reading, cursor != anchor, &shown);
    }
}
