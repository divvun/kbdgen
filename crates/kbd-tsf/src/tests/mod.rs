//! Host tests of the text service's TSF-independent logic against the
//! fixture keyboard and a fake context (`tsf.test.host`).

pub mod fake;
pub mod fixture;

mod document;
mod keys;
mod registration;

use kbd_engine::{Key, KeyEvent, Model, ModifierState};

use crate::document::{Document, Flags};
use fake::Fake;

/// A document typed into as the Windows layer drives it: an eaten key's
/// decision is applied, a passed key commits and resets.
pub struct Typist {
    pub model: Model,
    pub document: Document,
    pub fake: Fake,
    pub flags: Flags,
}

impl Typist {
    pub fn new(fake: Fake, flags: Flags) -> Typist {
        Typist {
            model: fixture::model(),
            document: Document::default(),
            fake,
            flags,
        }
    }

    /// A full text store, as a WPF `TextBox` presents.
    pub fn store() -> Typist {
        Typist::new(Fake::default(), Flags::default())
    }

    /// A transitory context, as a Win32 `EDIT` presents: TSF reads no text
    /// before the caret there.
    pub fn transitory() -> Typist {
        Typist::new(
            Fake::default(),
            Flags {
                transitory: true,
                disabled: false,
            },
        )
    }

    /// Sends `event`; returns whether it was eaten.
    pub fn press(&mut self, event: KeyEvent) -> bool {
        let decision = self
            .document
            .decide(&self.model, &mut self.fake, self.flags, &event);
        if decision.eats() {
            let limit = self.model.context_len();
            self.document.apply(&mut self.fake, decision, limit)
        } else {
            self.document.pass(&self.model, &mut self.fake, self.flags);
            false
        }
    }

    pub fn scan(&mut self, code: u8) -> bool {
        self.press(KeyEvent::new(Key::Scan(code)))
    }

    pub fn scan_with(&mut self, code: u8, modifiers: ModifierState) -> bool {
        self.press(KeyEvent::with(Key::Scan(code), modifiers))
    }
}
