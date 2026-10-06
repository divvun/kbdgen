//! The text service's state for one context: the engine `State`, the text
//! cache of non-authoritative contexts and whether a preedit is shown, and
//! the key, pass and reset logic over a [`TextContext`].

use kbd_engine::{Action, Context, Key, KeyEvent, Model, State};

use crate::planner::{Mode, Step, plan};
use crate::text::TextContext;

/// What the context told the text service before it read it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Flags {
    /// The context's static flags hold `TS_SS_TRANSITORY`.
    pub transitory: bool,
    /// The context is keyboard-disabled or empty (`tsf.security.disabled`).
    pub disabled: bool,
}

/// An engine decision for one key, made without changing the document, so
/// that `OnTestKeyDown` can make it and `OnKeyDown` commit it
/// (`tsf.keys.claim`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decision {
    pub context: Context,
    pub mode: Mode,
    /// The selection was not empty, so the document resets first.
    pub reset: bool,
    pub action: Action,
    pub state: State,
}

impl Decision {
    /// Whether the key is eaten: the engine did not pass it.
    pub fn eats(&self) -> bool {
        self.action != Action::Pass
    }
}

/// The per-context state. Dropping it discards the cache with the context.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Document {
    state: State,
    /// For contexts that are not authoritative: the scalar values the text
    /// service's own edits left before the caret, at most `context_len`.
    cache: String,
    composing: bool,
}

impl Document {
    pub fn state(&self) -> &State {
        &self.state
    }

    pub fn cache(&self) -> &str {
        &self.cache
    }

    /// Whether a preedit is shown as a composition.
    pub fn composing(&self) -> bool {
        self.composing
    }

    /// The engine's decision for `event`, from a fresh read of `text`.
    /// Nothing changes until [`Document::apply`].
    // [spec:kbdgen:req:tsf.edit.session]
    // [spec:kbdgen:req:tsf.edit.cache]
    // [spec:kbdgen:req:tsf.engine.contract]
    pub fn decide(
        &self,
        model: &Model,
        text: &mut dyn TextContext,
        flags: Flags,
        event: &KeyEvent,
    ) -> Decision {
        let reset = !text.selection_empty();
        let read = text.read_before(model.context_len()).ok();
        let (context, mode) = match read {
            Some(read) if !flags.transitory => (
                Context {
                    text: read.text,
                    authoritative: true,
                    at_start: read.at_start,
                },
                Mode {
                    authoritative: true,
                    disabled: flags.disabled,
                },
            ),
            _ => {
                let cached = if flags.disabled || reset {
                    String::new()
                } else {
                    self.cache.clone()
                };
                let context = Context {
                    text: cached,
                    authoritative: false,
                    at_start: false,
                };
                let mode = Mode {
                    authoritative: false,
                    disabled: flags.disabled,
                };
                (context, mode)
            }
        };
        let state = if reset {
            State::default()
        } else {
            self.state.clone()
        };
        let (action, state) = model.key(&state, &context, event);
        Decision {
            context,
            mode,
            reset,
            action,
            state,
        }
    }

    /// Applies a decision; returns whether the key was eaten. If an
    /// injection sends fewer events than given, the document resets.
    // [spec:kbdgen:req:tsf.edit.apply]
    // [spec:kbdgen:req:tsf.edit.inject]
    // [spec:kbdgen:req:tsf.edit.cache]
    // [spec:kbdgen:req:tsf.edit.preedit]
    pub fn apply(&mut self, text: &mut dyn TextContext, decision: Decision, limit: usize) -> bool {
        if decision.reset {
            self.reset(Some(text));
        }
        let Action::Edit { delete, insert, .. } = &decision.action else {
            return false;
        };
        let mut applied = true;
        for step in plan(&decision.action, &decision.context, decision.mode) {
            applied &= match step {
                Step::Replace { units, text: s } => text.replace(units, &s).is_ok(),
                Step::Preedit(preedit) if preedit.is_empty() && !self.composing => true,
                Step::Preedit(preedit) => {
                    let shown = text.set_preedit(&preedit).is_ok();
                    self.composing = shown && !preedit.is_empty();
                    shown
                }
                Step::Insert(s) => text.insert(&s).is_ok(),
                Step::Inject {
                    backspaces,
                    text: s,
                } => text.inject(backspaces, &s).is_ok(),
            };
        }
        if !applied {
            self.reset(Some(text));
            return true;
        }
        if !decision.mode.authoritative && !decision.mode.disabled {
            self.remember(*delete, insert, limit);
        }
        self.state = decision.state;
        true
    }

    fn remember(&mut self, delete: usize, insert: &str, limit: usize) {
        let keep = self.cache.chars().count().saturating_sub(delete);
        let mut cache: Vec<char> = self.cache.chars().take(keep).collect();
        cache.extend(insert.chars());
        let skip = cache.len().saturating_sub(limit);
        self.cache = cache.into_iter().skip(skip).collect();
    }

    /// A key other than a lone modifier passes: commit what is pending,
    /// then reset. A shown preedit stays as committed text; otherwise the
    /// edit of a `Commit` event is applied first, which gives the same text
    /// (`ldml.engine.preedit`).
    // [spec:kbdgen:req:tsf.edit.reset]
    pub fn pass(&mut self, model: &Model, text: &mut dyn TextContext, flags: Flags) {
        if self.composing {
            let _ = text.commit_preedit();
        } else if self.state != State::default() {
            let decision = self.decide(model, text, flags, &KeyEvent::new(Key::Commit));
            if !decision.reset {
                self.apply(text, decision, model.context_len());
            }
        }
        self.clear();
    }

    /// Resets to `State::default()` and clears the cache. With a context, a
    /// shown preedit stays as committed text.
    // [spec:kbdgen:req:tsf.edit.reset]
    pub fn reset(&mut self, text: Option<&mut dyn TextContext>) {
        if self.composing
            && let Some(text) = text
        {
            let _ = text.commit_preedit();
        }
        self.clear();
    }

    /// The application ended the composition, leaving its text
    /// (`tsf.edit.preedit`).
    pub fn terminated(&mut self) {
        self.clear();
    }

    fn clear(&mut self) {
        self.state = State::default();
        self.cache.clear();
        self.composing = false;
    }
}
