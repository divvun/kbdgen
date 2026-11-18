//! A fake client: a text field that applies the signals of
//! `linux.ibus.edit` as GTK's IBus module does, types what the engine
//! passes, and reports its text before the caret on time, one key late, or
//! not at all.

use std::collections::BTreeMap;

use kbd_engine::Model;
use kbd_ibus::keys::RELEASE_MASK;
use kbd_ibus::planner::{
    CAP_PREEDIT_TEXT, CAP_SURROUNDING_TEXT, PreeditMode, Reply, Session, Signal,
};

use super::cases::{Stroke, Typed};

/// `IBUS_CAP_FOCUS`, which every client here has.
const CAP_FOCUS: u32 = 1 << 3;
/// GTK's capabilities: a preedit, focus and surrounding text.
pub const GTK: u32 = CAP_PREEDIT_TEXT | CAP_FOCUS | CAP_SURROUNDING_TEXT;

/// When the client tells the engine its surrounding text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Report {
    /// Before each key, as GTK does.
    OnTime,
    /// Before each key, but as it was a key earlier, as a reading through
    /// GNOME Shell may be.
    Late,
    /// Never: the client has no `IBUS_CAP_SURROUNDING_TEXT`.
    Never,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Field {
    /// The committed text before the caret.
    pub before: String,
    pub preedit: String,
    pub mode: Option<PreeditMode>,
}

impl Field {
    pub fn apply(&mut self, signals: &[Signal]) {
        for signal in signals {
            match signal {
                Signal::CommitText(text) => self.before.push_str(text),
                Signal::UpdatePreeditText {
                    text,
                    cursor,
                    visible,
                    mode,
                } => {
                    assert_eq!(
                        usize::try_from(*cursor).unwrap(),
                        text.chars().count(),
                        "the cursor ends the preedit"
                    );
                    assert_eq!(*visible, !text.is_empty(), "shown when not empty");
                    self.preedit.clone_from(text);
                    self.mode = (!text.is_empty()).then_some(*mode);
                }
                Signal::DeleteSurroundingText { offset, nchars } => {
                    let n = usize::try_from(*nchars).unwrap();
                    assert_eq!(
                        i64::from(*offset),
                        -i64::from(*nchars),
                        "deletes before the caret"
                    );
                    assert!(self.preedit.is_empty(), "the preedit is cleared first");
                    let kept = self.before.chars().count().checked_sub(n).unwrap();
                    self.before = self.before.chars().take(kept).collect();
                }
                Signal::RequireSurroundingText => {}
            }
        }
    }

    /// The client resets or loses focus: a preedit shown to be committed is
    /// committed, any other dropped.
    pub fn settle(&mut self) {
        if self.mode == Some(PreeditMode::Commit) {
            self.before.push_str(&self.preedit);
        }
        self.preedit.clear();
        self.mode = None;
    }

    /// The text shown, the preedit at the caret; a preedit the client
    /// would drop is not text.
    pub fn text(&self) -> String {
        match self.mode {
            Some(PreeditMode::Commit) => format!("{}{}", self.before, self.preedit),
            _ => self.before.clone(),
        }
    }

    /// The client types a key the engine passed.
    pub fn type_passed(&mut self, typed: Typed) {
        match typed {
            Typed::Text(c) => self.before.push(c),
            Typed::Backspace => {
                assert!(self.preedit.is_empty(), "nothing is shown over a deletion");
                self.before.pop();
            }
            Typed::Nothing => {}
        }
    }
}

/// A client with one focused field.
pub struct Client {
    pub session: Session,
    pub field: Field,
    /// The signals of the last key press.
    pub last: Vec<Signal>,
    report: Report,
    /// The field's text before the caret before the last key.
    earlier: String,
    /// What each key down returned, by keyval and key code.
    pressed: BTreeMap<(u32, u32), bool>,
}

impl Client {
    /// A focused field with the engine enabled; `preedit` says whether the
    /// client shows a preedit.
    pub fn new(model: Model, report: Report, preedit: bool) -> Client {
        let mut session = Session::new(model);
        let mut capabilities = CAP_FOCUS;
        if preedit {
            capabilities |= CAP_PREEDIT_TEXT;
        }
        if report != Report::Never {
            capabilities |= CAP_SURROUNDING_TEXT;
        }
        session.set_capabilities(capabilities);
        session.focus_in();
        let signals = session.enable();
        assert_eq!(signals, [Signal::RequireSurroundingText]);
        Client {
            session,
            field: Field::default(),
            last: Vec::new(),
            report,
            earlier: String::new(),
            pressed: BTreeMap::new(),
        }
    }

    pub fn gtk(model: Model) -> Client {
        Client::new(model, Report::OnTime, true)
    }

    pub fn report(&self) -> Report {
        self.report
    }

    fn read(&mut self, text: &str) {
        let n = u32::try_from(text.chars().count()).unwrap();
        self.session.set_surrounding_text(text, n, n);
    }

    /// The client reads its field before a key, as its report says.
    fn read_before_key(&mut self) {
        match self.report {
            Report::OnTime => {
                let text = self.field.before.clone();
                self.read(&text);
            }
            Report::Late => {
                let text = self.earlier.clone();
                self.read(&text);
            }
            Report::Never => {}
        }
        self.earlier = self.field.before.clone();
    }

    /// The client changes its own text, as a deletion past the engine or
    /// a paste does, and reads it.
    pub fn replace_text(&mut self, text: &str) {
        self.field.before = text.to_owned();
        self.earlier = text.to_owned();
        if self.report != Report::Never {
            self.read(text);
        }
    }

    /// A key down; a key the engine passes is typed.
    pub fn press(&mut self, keyval: u32, code: u32, state: u32) -> bool {
        let Reply { handled, signals } = self.session.process_key_event(keyval, code, state);
        self.field.apply(&signals);
        self.last = signals;
        if !handled {
            self.field.type_passed(Typed::of(keyval, code, state));
        }
        self.pressed.insert((keyval, code), handled);
        handled
    }

    /// A key up, which returns what its key down did.
    pub fn release(&mut self, keyval: u32, code: u32, state: u32) {
        let release = self
            .session
            .process_key_event(keyval, code, state | RELEASE_MASK);
        let pressed = self.pressed.remove(&(keyval, code)).unwrap_or(false);
        assert_eq!(
            release.handled, pressed,
            "a release returns what its press did"
        );
        assert!(release.signals.is_empty());
    }

    /// A key pressed and released, the client reading its field first.
    pub fn key(&mut self, keyval: u32, code: u32, state: u32) -> bool {
        self.read_before_key();
        let handled = self.press(keyval, code, state);
        self.release(keyval, code, state);
        handled
    }

    /// A hardware key with no modifier and no keyval the client types.
    pub fn tap(&mut self, code: u32) -> bool {
        self.key(0, code, 0)
    }

    /// A stroke: its taps, its held keys down, the key, its held keys up.
    pub fn stroke(&mut self, stroke: &Stroke) -> bool {
        for &(keyval, code) in &stroke.taps {
            self.press(keyval, code, 0);
            self.release(keyval, code, 0);
        }
        for &(keyval, code) in &stroke.held {
            self.press(keyval, code, 0);
        }
        let handled = self.key(stroke.keyval, stroke.code, stroke.state);
        for &(keyval, code) in stroke.held.iter().rev() {
            self.release(keyval, code, stroke.state);
        }
        handled
    }

    pub fn focus_out(&mut self) {
        let signals = self.session.focus_out();
        assert!(signals.is_empty());
        self.field.settle();
    }
}
