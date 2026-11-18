//! The committed text before the caret, as the client's surrounding text
//! and the engine's own edits tell it (`linux.ibus.context`), reconciled
//! as the ChromeOS host does it (`ldml.chromeos.context`).
//!
//! Each edit records the state of the field it leads to: the text before
//! the caret and the preedit shown. A reading of the client's text agrees
//! with such a state when one text ends with the other; one that agrees
//! with the latest state completes the context, one that agrees with an
//! earlier state lags and is dropped, and any other replaces the context.
//! An empty text agrees only with an empty one: IBus says nothing of
//! where a reading's window starts, the start of a paragraph reads empty
//! whatever came before it, and every text ends with the empty text.

/// The most scalar values kept before the caret.
pub const MAX_TEXT: usize = 64;
/// The most states kept while readings lag.
const MAX_SNAPSHOTS: usize = 64;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Snapshot {
    text: String,
    preedit: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Surrounding {
    text: String,
    authoritative: bool,
    /// The last reading had a selection, which the next key replaces.
    selection: bool,
    /// The states the engine's own edits led to, oldest first.
    snapshots: Vec<Snapshot>,
}

impl Default for Surrounding {
    fn default() -> Self {
        Surrounding {
            text: String::new(),
            authoritative: false,
            selection: false,
            snapshots: vec![Snapshot::default()],
        }
    }
}

/// The last `MAX_TEXT` scalar values of `text`.
fn trim(text: &str) -> String {
    let skip = text.chars().count().saturating_sub(MAX_TEXT);
    text.chars().skip(skip).collect()
}

fn agrees(reading: &str, expected: &str) -> bool {
    reading.is_empty() == expected.is_empty()
        && (reading.ends_with(expected) || expected.ends_with(reading))
}

impl Surrounding {
    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn authoritative(&self) -> bool {
        self.authoritative
    }

    /// Forgets the text before the caret, as after the caret moved.
    // [spec:kbdgen:sem:linux.ibus.context]
    pub fn forget(&mut self, preedit: &str) {
        *self = Surrounding::default();
        self.restart(preedit);
    }

    /// Keeps only the current state as one a reading may agree with.
    pub fn restart(&mut self, preedit: &str) {
        self.snapshots = vec![Snapshot {
            text: self.text.clone(),
            preedit: preedit.to_owned(),
        }];
    }

    /// Records the state an operation led to.
    fn expect(&mut self, preedit: &str) {
        let current = Snapshot {
            text: self.text.clone(),
            preedit: preedit.to_owned(),
        };
        if self.snapshots.last() != Some(&current) {
            self.snapshots.push(current);
            if self.snapshots.len() > MAX_SNAPSHOTS {
                self.snapshots.remove(0);
            }
        }
    }

    /// The engine's edit deleted `count` scalar values before the caret.
    pub fn deleted(&mut self, count: usize, preedit: &str) {
        let keep = self.text.chars().count().saturating_sub(count);
        self.text = self.text.chars().take(keep).collect();
        self.expect(preedit);
    }

    pub fn committed(&mut self, text: &str, preedit: &str) {
        self.text = trim(&format!("{}{text}", self.text));
        self.expect(preedit);
    }

    /// The engine showed `preedit`, or cleared it.
    pub fn shown(&mut self, preedit: &str) {
        self.expect(preedit);
    }

    /// The committed text a reading shows if it is of `snapshot`: whether
    /// a reading includes the preedit is the client's choice, so both are
    /// tried.
    fn of(reading: &str, snapshot: &Snapshot) -> Option<String> {
        let preedit = &snapshot.preedit;
        if !preedit.is_empty()
            && let Some(text) = reading.strip_suffix(preedit.as_str())
            && agrees(reading, &format!("{}{preedit}", snapshot.text))
        {
            return Some(text.to_owned());
        }
        agrees(reading, &snapshot.text).then(|| reading.to_owned())
    }

    /// A reading of the text before the caret, `selection` saying that the
    /// client has text selected.
    // [spec:kbdgen:sem:linux.ibus.context]
    pub fn read(&mut self, reading: &str, selection: bool, preedit: &str) {
        self.selection = selection;
        if selection {
            return;
        }
        let latest = self.snapshots.len().saturating_sub(1);
        for (i, snapshot) in self.snapshots.iter().enumerate().rev() {
            let Some(text) = Surrounding::of(reading, snapshot) else {
                continue;
            };
            if i == latest {
                if text.chars().count() >= self.text.chars().count() {
                    self.text = trim(&text);
                }
                self.authoritative = true;
                self.restart(preedit);
            } else {
                self.snapshots.drain(..i);
            }
            return;
        }
        let text = if preedit.is_empty() {
            reading
        } else {
            reading.strip_suffix(preedit).unwrap_or(reading)
        };
        self.text = trim(text);
        self.authoritative = true;
        self.restart(preedit);
    }

    /// A key replaced the selection: nothing before the caret is known.
    pub fn take_selection(&mut self, preedit: &str) -> bool {
        let selection = self.selection;
        if selection {
            self.forget(preedit);
        }
        selection
    }
}
