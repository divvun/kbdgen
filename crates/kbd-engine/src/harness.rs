//! The test harness of `ldml.test.harness`: a document and a [`State`]
//! driven through [`Model::key`] alone, with no Windows, application or OS
//! dependency (`tsf.test.engine`).
//!
//! CLDR's keyboardTest3 vectors and kbdgen's golden vectors both run
//! through it, so they apply edits exactly alike.

use alloc::string::String;

use crate::{Action, Context, KeyEvent, Model, State};

// [spec:kbdgen:def:ldml.test.harness+1]
/// A document, the text before the caret, with the engine state and what
/// the last events did to it.
///
/// Each event gets the last `context_len` scalar values of the document as
/// `Context.text`, with `at_start` set when that is the whole document and
/// the document begins at a start of text. An `Edit` deletes `delete`
/// scalar values and appends `insert`, and its `preedit` and `layer` are
/// recorded. A `Pass` changes nothing but the record that the event
/// passed.
#[derive(Debug, Clone)]
pub struct Harness<'m> {
    model: &'m Model,
    state: State,
    document: String,
    at_start: bool,
    preedit: String,
    layer: Option<String>,
    last: Option<Action>,
}

impl<'m> Harness<'m> {
    /// An empty document that begins at a start of text, in the reset
    /// state.
    pub fn new(model: &'m Model) -> Self {
        Harness {
            model,
            state: State::default(),
            document: String::new(),
            at_start: true,
            preedit: String::new(),
            layer: None,
            last: None,
        }
    }

    /// Replaces the document, keeping the state, as keyboardTest3's
    /// `startContext` does. The engine drops the state's markers when the
    /// new document does not end with their text (`ldml.engine.context`).
    pub fn set_document(&mut self, text: &str) {
        self.document = String::from(text);
    }

    /// Whether the document begins at a start of text. A host that cannot
    /// tell passes `false`.
    pub fn set_at_start(&mut self, at_start: bool) {
        self.at_start = at_start;
    }

    /// Sets `State::default()`, as a host does when the caret moves.
    pub fn reset(&mut self) {
        self.state = State::default();
    }

    /// The context the next event gets.
    pub fn context(&self) -> Context {
        let count = self.document.chars().count();
        let skip = count.saturating_sub(self.model.context_len());
        Context {
            text: self.document.chars().skip(skip).collect(),
            authoritative: true,
            at_start: self.at_start && skip == 0,
        }
    }

    /// Sends `event` and applies its action; returns that action.
    pub fn send(&mut self, event: &KeyEvent) -> &Action {
        let (action, state) = self.model.key(&self.state, &self.context(), event);
        self.state = state;
        if let Action::Edit {
            delete,
            insert,
            preedit,
            layer,
        } = &action
        {
            let keep = self.document.chars().count().saturating_sub(*delete);
            let mut document: String = self.document.chars().take(keep).collect();
            document.push_str(insert);
            self.document = document;
            self.preedit.clone_from(preedit);
            self.layer.clone_from(layer);
        }
        self.last.insert(action)
    }

    pub fn document(&self) -> &str {
        &self.document
    }

    /// The preedit of the last edit.
    pub fn preedit(&self) -> &str {
        &self.preedit
    }

    /// The touch layer the last edit switched to, if it did.
    pub fn layer(&self) -> Option<&str> {
        self.layer.as_deref()
    }

    /// The action of the last event, if any was sent.
    pub fn last_action(&self) -> Option<&Action> {
        self.last.as_ref()
    }

    /// Whether the last event passed.
    pub fn passed(&self) -> bool {
        matches!(self.last, Some(Action::Pass))
    }

    pub fn state(&self) -> &State {
        &self.state
    }
}
