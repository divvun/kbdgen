//! A fake client: a text field that applies the signals of
//! `linux.ibus.edit` as GTK's IBus module does.

use crate::planner::{PreeditMode, Signal};

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

    /// What the user sees: the text with the preedit at the caret.
    pub fn shown(&self) -> String {
        format!("{}{}", self.before, self.preedit)
    }
}
