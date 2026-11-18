//! The engine run natively: a host that knows its document, so every key
//! gets the text before the caret as its context, applies each edit to
//! it, and on a key that passes commits what is pending and resets
//! (`ldml.engine.state`, `ldml.engine.commit`). What the IBus engine leaves
//! in a field must agree with it.

use kbd_engine::{Action, Context, Key, KeyEvent, Model, State};
use kbd_ibus::planner::OPTIONS;

use super::cases::Typed;

pub struct Native {
    model: Model,
    /// The host gives the engine no context, as a password field does
    /// (`linux.ibus.security`, `tsf.security.disabled`).
    blind: bool,
    state: State,
    pub document: String,
    pub preedit: String,
}

impl Native {
    pub fn new(model: &Model) -> Native {
        Native {
            model: model.clone().with_options(OPTIONS),
            blind: false,
            state: State::default(),
            document: String::new(),
            preedit: String::new(),
        }
    }

    pub fn blind(model: &Model) -> Native {
        Native {
            blind: true,
            ..Native::new(model)
        }
    }

    fn context(&self) -> Context {
        if self.blind {
            return Context::default();
        }
        let n = self.document.chars().count();
        let skip = n.saturating_sub(self.model.context_len());
        Context {
            text: self.document.chars().skip(skip).collect(),
            authoritative: true,
            at_start: false,
        }
    }

    fn apply(&mut self, action: &Action) {
        if let Action::Edit {
            delete,
            insert,
            preedit,
            ..
        } = action
        {
            let n = self.document.chars().count();
            self.document = self.document.chars().take(n - delete).collect();
            self.document.push_str(insert);
            self.preedit.clone_from(preedit);
        }
    }

    pub fn pending(&self) -> bool {
        !self.model.pending_markers(&self.state).is_empty()
    }

    /// A key: the engine's event, or `None` for a frame key; whether the
    /// engine handled it. A key it passes is typed as `typed`.
    pub fn key(&mut self, event: Option<&KeyEvent>, typed: Typed) -> bool {
        if let Some(event) = event {
            let (action, next) = self.model.key(&self.state, &self.context(), event);
            if action != Action::Pass {
                self.apply(&action);
                self.state = next;
                return true;
            }
        }
        if !self.preedit.is_empty() || self.pending() {
            let commit = KeyEvent::new(Key::Commit);
            let (action, _) = self.model.key(&self.state, &self.context(), &commit);
            self.apply(&action);
            self.preedit.clear();
        }
        self.state = State::default();
        match typed {
            Typed::Text(c) => self.document.push(c),
            Typed::Backspace => {
                self.document.pop();
            }
            Typed::Nothing => {}
        }
        false
    }

    /// The client deletes the scalar value before the caret itself.
    pub fn delete_outside(&mut self) {
        self.document.pop();
    }
}
