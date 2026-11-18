//! Diagnostics (`linux.ibus.logging`): the only things the engine writes
//! to standard error, which ibus-daemon passes on when it runs verbosely.
//!
//! An [`Event`] holds only messages compiled into the program, a reason of
//! `linux.ibus.inert` with a [`FileName`], which can be made only from a
//! path, and a panic's location without its message. So typed text,
//! surrounding text, preedit text, keyvals, key codes and engine states
//! cannot reach a diagnostic: no variant has a field that could hold them.
//! `tests/self_contained.rs` checks that nothing else in the program writes
//! to standard error or standard output.

use std::fmt;
use std::panic::Location;
use std::path::Path;

use crate::data::Reason;
use crate::guard::Entry;

/// The last component of a data file's path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileName(String);

impl FileName {
    pub fn of(path: &Path) -> FileName {
        FileName(
            path.file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default(),
        )
    }
}

/// What the engine records.
// [spec:kbdgen:req:linux.ibus.logging]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// `CreateEngine` refused, for `reason`, the engine whose `file` said so
    /// (`linux.ibus.inert`).
    Inert { reason: Reason, file: FileName },
    /// A panic was contained in `entry` (`linux.ibus.panic`).
    Panicked {
        entry: Option<Entry>,
        location: Option<&'static Location<'static>>,
    },
    /// The program could not serve ibus-daemon.
    Failed(&'static str),
}

impl fmt::Display for Event {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Event::Inert { reason, file } => write!(f, "{}: {}", file.0, reason.code()),
            Event::Panicked { entry, location } => {
                f.write_str("panic")?;
                if let Some(entry) = entry {
                    write!(f, " in {}", entry.name())?;
                }
                if let Some(location) = location {
                    write!(
                        f,
                        " at {}:{}:{}",
                        location.file(),
                        location.line(),
                        location.column()
                    )?;
                }
                Ok(())
            }
            Event::Failed(message) => f.write_str(message),
        }
    }
}

/// Writes `event` as one line of standard error. It never panics: a line
/// that cannot be written is dropped.
// [spec:kbdgen:req:linux.ibus.logging]
pub fn record(event: Event) {
    #[cfg(test)]
    recorder::push(event);
    #[cfg(not(test))]
    {
        use std::io::Write as _;
        let line = format!("ibus-engine-divvun: {event}\n");
        let _ = std::io::stderr().write_all(line.as_bytes());
    }
}

/// What this thread recorded, for tests, in place of standard error.
#[cfg(test)]
pub mod recorder {
    use std::cell::RefCell;

    use super::Event;

    thread_local! {
        static EVENTS: RefCell<Vec<Event>> = const { RefCell::new(Vec::new()) };
    }

    pub(super) fn push(event: Event) {
        let _ = EVENTS.try_with(|events| events.borrow_mut().push(event));
    }

    /// The events recorded on this thread since the last call.
    pub fn take() -> Vec<Event> {
        EVENTS.with_borrow_mut(std::mem::take)
    }
}
