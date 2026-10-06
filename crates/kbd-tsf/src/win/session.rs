//! Edit sessions: the `TextContext` of `crate::text` over an `ITfContext`,
//! the `SendInput` fallback, and the requests that run document work in
//! a session.

use std::cell::RefCell;
use std::mem::ManuallyDrop;
use std::rc::Rc;

use kbd_engine::KeyEvent;
use windows::Win32::System::Variant::VARIANT;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT, KEYEVENTF_KEYUP,
    KEYEVENTF_UNICODE, SendInput, VIRTUAL_KEY, VK_BACK,
};
use windows::Win32::UI::TextServices::{
    GUID_PROP_ATTRIBUTE, ITfComposition, ITfCompositionSink, ITfContext, ITfContextComposition,
    ITfEditSession, ITfEditSession_Impl, ITfRange, TF_AE_NONE, TF_ANCHOR_END, TF_ANCHOR_START,
    TF_DEFAULT_SELECTION, TF_ES_ASYNCDONTCARE, TF_ES_READ, TF_ES_READWRITE, TF_ES_SYNC,
    TF_SELECTION, TF_SELECTIONSTYLE,
};
use windows_core::{ComObject, Interface, Result, implement};

use crate::document::{Decision, Flags};
use crate::guard::{Entry, POISON, contain, poisoned};
use crate::server::Live;
use crate::text::{Failed, Read, TextContext, decode_before};

use super::tip::{Inner, Tip_Impl};

/// The `dwExtraInfo` of every event the text service injects: "DVKT".
pub const SIGNATURE: usize = 0x4456_4B54;

/// A key down's identity and what `OnTestKeyDown` decided for it, which
/// `OnKeyDown` commits (`tsf.keys.claim`). `None` means the key passed and
/// its pass was handled.
// [spec:kbdgen:req:tsf.keys.phases]
pub struct Pending {
    pub id: (u16, u16, u32),
    pub decision: Option<Decision>,
}

/// The work of one edit session.
pub enum Task {
    Decide(KeyEvent, Flags),
    Apply(Decision),
    /// Decide and apply, or pass.
    Key(KeyEvent, Flags),
    Pass(Flags),
    /// End a composition left behind by a reset, leaving its text.
    End(ITfComposition),
}

/// What a session did.
pub enum Outcome {
    Decided(Decision),
    Eaten(bool),
    Done,
}

#[implement(ITfEditSession)]
struct Session {
    _live: Live,
    shared: Rc<RefCell<Inner>>,
    context: ITfContext,
    sink: ITfCompositionSink,
    task: RefCell<Option<Task>>,
    outcome: RefCell<Option<Outcome>>,
}

impl ITfEditSession_Impl for Session_Impl {
    fn DoEditSession(&self, ec: u32) -> Result<()> {
        contain(&POISON, Entry::DoEditSession, || {
            let Some(task) = self.task.borrow_mut().take() else {
                return Ok(());
            };
            let outcome = self.run(task, Some(ec));
            *self.outcome.borrow_mut() = Some(outcome);
            Ok(())
        })
    }
}

impl Session {
    /// Runs `task` with the edit cookie `ec`, or detached from any session.
    // [spec:kbdgen:req:tsf.edit.session+1]
    fn run(&self, task: Task, ec: Option<u32>) -> Outcome {
        if let Task::End(composition) = task {
            if let Some(ec) = ec {
                let attribute = self.shared.try_borrow().ok().and_then(|i| i.attribute);
                let mut text = TsfText::new(ec, &self.context, &self.sink, attribute);
                text.composition = Some(composition);
                let _ = text.commit_preedit();
            }
            return Outcome::Done;
        }
        let Ok(mut inner) = self.shared.try_borrow_mut() else {
            return Outcome::Eaten(false);
        };
        inner.switch(&self.context);
        let Some(keyboard) = inner.keyboard.clone() else {
            return Outcome::Eaten(false);
        };
        let model = &keyboard.model;
        let limit = model.context_len();
        let mut tsf = ec.map(|ec| TsfText::new(ec, &self.context, &self.sink, inner.attribute));
        let mut detached = Detached::default();
        let text: &mut dyn TextContext = match tsf.as_mut() {
            Some(tsf) => {
                tsf.composition = inner.composition.take();
                tsf
            }
            None => &mut detached,
        };
        let document = &mut inner.document;
        let outcome = match task {
            Task::Decide(event, flags) => {
                Outcome::Decided(document.decide(model, text, detach(flags, ec), &event))
            }
            Task::Apply(decision) => Outcome::Eaten(document.apply(text, decision, limit)),
            Task::Key(event, flags) => {
                let decision = document.decide(model, text, detach(flags, ec), &event);
                if decision.eats() {
                    Outcome::Eaten(document.apply(text, decision, limit))
                } else {
                    document.pass(model, text, detach(flags, ec));
                    Outcome::Eaten(false)
                }
            }
            Task::Pass(flags) => {
                document.pass(model, text, detach(flags, ec));
                Outcome::Done
            }
            Task::End(_) => Outcome::Done,
        };
        let (composition, injected, echoes) = match tsf.as_mut() {
            Some(tsf) => (tsf.composition.take(), tsf.injected, tsf.echoes),
            None => (None, detached.injected, 0),
        };
        inner.composition = composition;
        inner.injected |= injected;
        inner.echoes = inner.echoes.saturating_add(echoes);
        outcome
    }
}

/// Without an edit session nothing can be read, so the cache stands in
/// for the context, as for a transitory one.
fn detach(flags: Flags, ec: Option<u32>) -> Flags {
    Flags {
        transitory: flags.transitory || ec.is_none(),
        ..flags
    }
}

impl Tip_Impl {
    /// Runs `task` for `context` in a synchronous edit session (an
    /// asynchronous one for `Task::End`). If TSF grants none, the task runs
    /// detached, with injected input only.
    // [spec:kbdgen:req:tsf.edit.session+1]
    pub(super) fn request(&self, context: &ITfContext, task: Task, write: bool) -> Outcome {
        if poisoned() {
            return Outcome::Eaten(false);
        }
        let ending = matches!(task, Task::End(_));
        let session = ComObject::new(Session {
            _live: Live::default(),
            shared: self.shared.clone(),
            context: context.clone(),
            sink: self.composition_sink(),
            task: RefCell::new(Some(task)),
            outcome: RefCell::new(None),
        });
        let interface: ITfEditSession = session.to_interface();
        let flags = match (ending, write) {
            (true, _) => TF_ES_ASYNCDONTCARE | TF_ES_READWRITE,
            (false, true) => TF_ES_SYNC | TF_ES_READWRITE,
            (false, false) => TF_ES_SYNC | TF_ES_READ,
        };
        let client = self.set_own(true);
        let _ = unsafe { context.RequestEditSession(client, &interface, flags) };
        self.set_own(false);
        if let Some(outcome) = session.outcome.borrow_mut().take() {
            return outcome;
        }
        match session.task.borrow_mut().take() {
            Some(task) if !ending => session.run(task, None),
            _ => Outcome::Done,
        }
    }

    /// Marks the text service's own edit requests; returns the client id.
    fn set_own(&self, own: bool) -> u32 {
        match self.shared.try_borrow_mut() {
            Ok(mut inner) => {
                inner.own_edit = own;
                inner.client
            }
            Err(_) => 0,
        }
    }
}

/// Sends `backspaces` Backspace presses and `text` as Unicode presses in
/// one `SendInput` call, each event signed with [`SIGNATURE`]; `Failed`
/// unless every event was sent.
// [spec:kbdgen:req:tsf.edit.inject+1]
// [spec:kbdgen:sem:tsf.security.integrity]
fn inject(backspaces: usize, text: &str) -> std::result::Result<(), Failed> {
    let mut inputs = Vec::new();
    for _ in 0..backspaces {
        inputs.push(key(VK_BACK, 0x0E, KEYBD_EVENT_FLAGS(0)));
        inputs.push(key(VK_BACK, 0x0E, KEYEVENTF_KEYUP));
    }
    for unit in text.encode_utf16() {
        inputs.push(key(VIRTUAL_KEY(0), unit, KEYEVENTF_UNICODE));
        inputs.push(key(
            VIRTUAL_KEY(0),
            unit,
            KEYEVENTF_UNICODE | KEYEVENTF_KEYUP,
        ));
    }
    if send(&inputs) { Ok(()) } else { Err(Failed) }
}

/// A key event signed with [`SIGNATURE`].
fn key(vk: VIRTUAL_KEY, scan: u16, flags: KEYBD_EVENT_FLAGS) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: scan,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: SIGNATURE,
            },
        },
    }
}

/// Sends `inputs` in one call; returns whether every event was sent.
fn send(inputs: &[INPUT]) -> bool {
    let size = i32::try_from(size_of::<INPUT>()).unwrap_or(i32::MAX);
    let sent = unsafe { SendInput(inputs, size) };
    usize::try_from(sent).is_ok_and(|sent| sent == inputs.len())
}

/// Presses and releases the unassigned virtual key `0xFF`, so that an
/// application that saw Right Alt go down, and none of the chord's keys,
/// does not open its menu bar when Alt goes up (`tsf.keys.altgr`).
// [spec:kbdgen:req:tsf.keys.preserved+1]
pub fn mask_menu() {
    let mask = VIRTUAL_KEY(0xFF);
    send(&[
        key(mask, 0, KEYBD_EVENT_FLAGS(0)),
        key(mask, 0, KEYEVENTF_KEYUP),
    ]);
}

/// The text context when TSF grants no edit session: only injection.
#[derive(Default)]
struct Detached {
    injected: bool,
}

impl TextContext for Detached {
    fn read_before(&mut self, _max: usize) -> std::result::Result<Read, Failed> {
        Err(Failed)
    }

    fn selection_empty(&mut self) -> bool {
        true
    }

    fn replace(&mut self, _units: usize, _text: &str) -> std::result::Result<(), Failed> {
        Err(Failed)
    }

    fn set_preedit(&mut self, preedit: &str) -> std::result::Result<(), Failed> {
        if preedit.is_empty() {
            Ok(())
        } else {
            Err(Failed)
        }
    }

    fn commit_preedit(&mut self) -> std::result::Result<(), Failed> {
        Ok(())
    }

    fn insert(&mut self, text: &str) -> std::result::Result<(), Failed> {
        self.injected = true;
        inject(0, text)
    }

    fn inject(&mut self, backspaces: usize, text: &str) -> std::result::Result<(), Failed> {
        self.injected = true;
        inject(backspaces, text)
    }
}

/// A context inside an edit session with cookie `ec`.
struct TsfText<'a> {
    ec: u32,
    context: &'a ITfContext,
    sink: &'a ITfCompositionSink,
    attribute: Option<u32>,
    composition: Option<ITfComposition>,
    /// Where the last replacement ended: the preedit goes there.
    after: Option<ITfRange>,
    injected: bool,
    /// `SetText` edits at the selection, each of which TSF echoes.
    echoes: u32,
}

impl<'a> TsfText<'a> {
    fn new(
        ec: u32,
        context: &'a ITfContext,
        sink: &'a ITfCompositionSink,
        attribute: Option<u32>,
    ) -> Self {
        TsfText {
            ec,
            context,
            sink,
            attribute,
            composition: None,
            after: None,
            injected: false,
            echoes: 0,
        }
    }

    fn selection(&self) -> Result<ITfRange> {
        let mut selections = [TF_SELECTION::default()];
        let mut fetched = 0;
        unsafe {
            self.context
                .GetSelection(self.ec, TF_DEFAULT_SELECTION, &mut selections, &mut fetched)
        }?;
        let [selection] = selections;
        ManuallyDrop::into_inner(selection.range).ok_or_else(windows_core::Error::empty)
    }

    fn select(&self, range: &ITfRange) -> Result<()> {
        let selection = TF_SELECTION {
            range: ManuallyDrop::new(Some(range.clone())),
            style: TF_SELECTIONSTYLE {
                ase: TF_AE_NONE,
                fInterimChar: false.into(),
            },
        };
        let result = unsafe {
            self.context
                .SetSelection(self.ec, std::slice::from_ref(&selection))
        };
        drop(ManuallyDrop::into_inner(selection.range));
        result
    }

    /// The reading and replacing caret: the preedit's start, or the
    /// selection's start.
    fn anchor(&self) -> Result<ITfRange> {
        let range = match &self.composition {
            Some(composition) => unsafe { composition.GetRange() }?,
            None => self.selection()?,
        };
        unsafe { range.Collapse(self.ec, TF_ANCHOR_START) }?;
        Ok(range)
    }

    /// Underlines the composition with the preedit display attribute, or
    /// clears it, as `mark` says.
    fn decorate(&self, range: &ITfRange, mark: bool) -> Result<()> {
        let property = unsafe { self.context.GetProperty(&GUID_PROP_ATTRIBUTE) }?;
        match self.attribute {
            Some(atom) if mark => {
                let value = VARIANT::from(i32::try_from(atom).unwrap_or_default());
                unsafe { property.SetValue(self.ec, range, &value) }
            }
            _ => unsafe { property.Clear(self.ec, range) },
        }
    }

    /// Moves the composition's start past the text the last replacement
    /// inserted at it, which TSF may have added to the composition.
    fn trim(&self, composition: &ITfComposition) -> Result<()> {
        match &self.after {
            Some(after) => unsafe { composition.ShiftStart(self.ec, after) },
            None => Ok(()),
        }
    }
}

impl TextContext for TsfText<'_> {
    fn read_before(&mut self, max: usize) -> std::result::Result<Read, Failed> {
        let range = self.anchor()?;
        let requested = max.saturating_mul(2);
        let shift = i32::try_from(requested).unwrap_or(i32::MAX);
        let mut moved = 0;
        unsafe { range.ShiftStart(self.ec, -shift, &mut moved, std::ptr::null()) }?;
        let mut units = vec![0u16; requested];
        let mut got = 0u32;
        unsafe { range.GetText(self.ec, 0, &mut units, &mut got) }?;
        units.truncate(usize::try_from(got).unwrap_or_default());
        Ok(decode_before(&units, requested, max))
    }

    fn selection_empty(&mut self) -> bool {
        self.composition.is_some()
            || self
                .selection()
                .and_then(|range| unsafe { range.IsEmpty(self.ec) })
                .is_ok_and(|empty| empty.as_bool())
    }

    // [spec:kbdgen:req:tsf.edit.apply+1]
    fn replace(&mut self, units: usize, text: &str) -> std::result::Result<(), Failed> {
        let range = match &self.composition {
            Some(_) => self.anchor()?,
            None => self.selection()?,
        };
        if units > 0 {
            let shift = i32::try_from(units).unwrap_or(i32::MAX);
            let mut moved = 0;
            unsafe { range.ShiftStart(self.ec, -shift, &mut moved, std::ptr::null()) }?;
            if moved != -shift {
                return Err(Failed);
            }
        }
        let wide: Vec<u16> = text.encode_utf16().collect();
        unsafe { range.SetText(self.ec, 0, &wide) }?;
        unsafe { range.Collapse(self.ec, TF_ANCHOR_END) }?;
        if self.composition.is_none() {
            self.select(&range)?;
        }
        self.after = Some(range);
        Ok(())
    }

    // [spec:kbdgen:req:tsf.edit.preedit+1]
    fn set_preedit(&mut self, preedit: &str) -> std::result::Result<(), Failed> {
        let wide: Vec<u16> = preedit.encode_utf16().collect();
        let composition = match (self.composition.take(), wide.is_empty()) {
            (None, true) => return Ok(()),
            (Some(composition), true) => {
                self.trim(&composition)?;
                let range = unsafe { composition.GetRange() }?;
                self.decorate(&range, false)?;
                unsafe { range.SetText(self.ec, 0, &[]) }?;
                unsafe { composition.EndComposition(self.ec) }?;
                self.select(&range)?;
                return Ok(());
            }
            (Some(composition), false) => {
                self.trim(&composition)?;
                composition
            }
            (None, false) => {
                let at = match self.after.take() {
                    Some(after) => after,
                    None => self.anchor()?,
                };
                let contexts: ITfContextComposition = self.context.cast()?;
                unsafe { contexts.StartComposition(self.ec, &at, self.sink) }?
            }
        };
        let range = unsafe { composition.GetRange() }?;
        unsafe { range.SetText(self.ec, 0, &wide) }?;
        let range = unsafe { composition.GetRange() }?;
        self.decorate(&range, true)?;
        unsafe { range.Collapse(self.ec, TF_ANCHOR_END) }?;
        self.select(&range)?;
        self.composition = Some(composition);
        Ok(())
    }

    fn commit_preedit(&mut self) -> std::result::Result<(), Failed> {
        let Some(composition) = self.composition.take() else {
            return Ok(());
        };
        let range = unsafe { composition.GetRange() }?;
        let _ = self.decorate(&range, false);
        unsafe { composition.EndComposition(self.ec) }?;
        unsafe { range.Collapse(self.ec, TF_ANCHOR_END) }?;
        self.select(&range)?;
        Ok(())
    }

    // [spec:kbdgen:req:tsf.edit.own]
    fn insert(&mut self, text: &str) -> std::result::Result<(), Failed> {
        self.echoes += 1;
        let range = self.selection()?;
        let wide: Vec<u16> = text.encode_utf16().collect();
        unsafe { range.SetText(self.ec, 0, &wide) }?;
        unsafe { range.Collapse(self.ec, TF_ANCHOR_END) }?;
        self.select(&range)?;
        Ok(())
    }

    fn inject(&mut self, backspaces: usize, text: &str) -> std::result::Result<(), Failed> {
        self.injected = true;
        inject(backspaces, text)
    }
}

/// An edit session on `context` for a text service with no state.
#[cfg(test)]
pub(super) fn test_session(context: ITfContext) -> ITfEditSession {
    let sink: ITfCompositionSink = super::tip::Tip::new().into();
    ComObject::new(Session {
        _live: Live::default(),
        shared: Rc::default(),
        context,
        sink,
        task: RefCell::new(None),
        outcome: RefCell::new(None),
    })
    .to_interface()
}
