//! Panic containment at every D-Bus entry point (`linux.ibus.panic`).
//!
//! One process serves every input context, so a panic that unwound out of
//! a method call would end them all. Every method call and property access
//! runs its body through [`Process::contain`], which turns a panic into the
//! entry point's fallback reply and poisons the process: from then on no
//! body runs, so every key passes and nothing is emitted, and the process
//! is [`Process::done`] once its last engine object is destroyed. The panic
//! hook records where the panic happened, never its message, which could
//! hold text.

use std::cell::Cell;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Once;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use crate::diag::{self, Event};

#[cfg(panic = "abort")]
compile_error!("kbd-ibus must build with panic = \"unwind\" to contain panics");

/// Every D-Bus method and property the program serves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Entry {
    CreateEngine,
    ProcessKeyEvent,
    SetCursorLocation,
    ProcessHandWritingEvent,
    CancelHandWriting,
    SetCapabilities,
    PropertyActivate,
    PropertyShow,
    PropertyHide,
    CandidateClicked,
    FocusIn,
    FocusInId,
    FocusOut,
    FocusOutId,
    Reset,
    Enable,
    Disable,
    PageUp,
    PageDown,
    CursorUp,
    CursorDown,
    SetSurroundingText,
    PanelExtensionReceived,
    PanelExtensionRegisterKeys,
    SetContentType,
    GetFocusId,
    GetActiveSurroundingText,
    Destroy,
}

/// All entry points, for fault injection.
#[cfg(test)]
pub const ENTRIES: [Entry; 28] = [
    Entry::CreateEngine,
    Entry::ProcessKeyEvent,
    Entry::SetCursorLocation,
    Entry::ProcessHandWritingEvent,
    Entry::CancelHandWriting,
    Entry::SetCapabilities,
    Entry::PropertyActivate,
    Entry::PropertyShow,
    Entry::PropertyHide,
    Entry::CandidateClicked,
    Entry::FocusIn,
    Entry::FocusInId,
    Entry::FocusOut,
    Entry::FocusOutId,
    Entry::Reset,
    Entry::Enable,
    Entry::Disable,
    Entry::PageUp,
    Entry::PageDown,
    Entry::CursorUp,
    Entry::CursorDown,
    Entry::SetSurroundingText,
    Entry::PanelExtensionReceived,
    Entry::PanelExtensionRegisterKeys,
    Entry::SetContentType,
    Entry::GetFocusId,
    Entry::GetActiveSurroundingText,
    Entry::Destroy,
];

impl Entry {
    /// The D-Bus member, as diagnostics name it.
    // [spec:kbdgen:req:linux.ibus.logging]
    pub const fn name(self) -> &'static str {
        match self {
            Entry::CreateEngine => "CreateEngine",
            Entry::ProcessKeyEvent => "ProcessKeyEvent",
            Entry::SetCursorLocation => "SetCursorLocation",
            Entry::ProcessHandWritingEvent => "ProcessHandWritingEvent",
            Entry::CancelHandWriting => "CancelHandWriting",
            Entry::SetCapabilities => "SetCapabilities",
            Entry::PropertyActivate => "PropertyActivate",
            Entry::PropertyShow => "PropertyShow",
            Entry::PropertyHide => "PropertyHide",
            Entry::CandidateClicked => "CandidateClicked",
            Entry::FocusIn => "FocusIn",
            Entry::FocusInId => "FocusInId",
            Entry::FocusOut => "FocusOut",
            Entry::FocusOutId => "FocusOutId",
            Entry::Reset => "Reset",
            Entry::Enable => "Enable",
            Entry::Disable => "Disable",
            Entry::PageUp => "PageUp",
            Entry::PageDown => "PageDown",
            Entry::CursorUp => "CursorUp",
            Entry::CursorDown => "CursorDown",
            Entry::SetSurroundingText => "SetSurroundingText",
            Entry::PanelExtensionReceived => "PanelExtensionReceived",
            Entry::PanelExtensionRegisterKeys => "PanelExtensionRegisterKeys",
            Entry::SetContentType => "ContentType",
            Entry::GetFocusId => "FocusId",
            Entry::GetActiveSurroundingText => "ActiveSurroundingText",
            Entry::Destroy => "Destroy",
        }
    }
}

thread_local! {
    /// The entry point running on this thread, for the panic hook.
    static ENTRY: Cell<Option<Entry>> = const { Cell::new(None) };
}

/// Replaces the default panic hook, which would print the panic message,
/// and with it possibly text, to standard error. It records the entry
/// point and the location instead. It must not panic: a panic in a panic
/// hook aborts. Test builds keep the previous hook after it, so failures
/// still print.
// [spec:kbdgen:req:linux.ibus.logging]
// [spec:kbdgen:req:linux.ibus.panic]
pub fn install_hook() {
    static HOOK: Once = Once::new();
    // `set_hook` panics on a thread that is panicking.
    if std::thread::panicking() {
        return;
    }
    HOOK.call_once(|| {
        #[cfg(test)]
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            diag::record(Event::Panicked {
                entry: ENTRY.try_with(Cell::get).ok().flatten(),
                location: info.location(),
            });
            #[cfg(test)]
            previous(info);
        }));
    });
}

/// The program's state across its engine objects: whether it has
/// panicked, and how many engine objects are exported.
#[derive(Debug, Default)]
pub struct Process {
    poison: AtomicBool,
    live: AtomicUsize,
}

impl Process {
    pub fn poisoned(&self) -> bool {
        self.poison.load(Ordering::SeqCst)
    }

    /// Runs the body of entry point `entry`. In a poisoned process the
    /// body does not run, and a body that panics poisons it; either gives
    /// `fallback`.
    // [spec:kbdgen:req:linux.ibus.panic]
    pub fn contain<T>(&self, entry: Entry, fallback: T, body: impl FnOnce() -> T) -> T {
        install_hook();
        if self.poisoned() {
            return fallback;
        }
        let outer = ENTRY
            .try_with(|current| current.replace(Some(entry)))
            .ok()
            .flatten();
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            #[cfg(test)]
            faults::inject(entry);
            body()
        }));
        let _ = ENTRY.try_with(|current| current.set(outer));
        match outcome {
            Ok(value) => value,
            Err(payload) => {
                self.poison.store(true, Ordering::SeqCst);
                drop(catch_unwind(AssertUnwindSafe(move || drop(payload))));
                fallback
            }
        }
    }

    /// An engine object was exported.
    pub fn created(&self) {
        self.live.fetch_add(1, Ordering::SeqCst);
    }

    /// An engine object was destroyed.
    pub fn destroyed(&self) {
        let _ = self
            .live
            .try_update(Ordering::SeqCst, Ordering::SeqCst, |live| {
                live.checked_sub(1)
            });
    }

    /// Whether the process should exit: it is poisoned, and ibus-daemon
    /// has destroyed every engine object, so the next activation starts a
    /// fresh process.
    // [spec:kbdgen:req:linux.ibus.panic]
    pub fn done(&self) -> bool {
        self.poisoned() && self.live.load(Ordering::SeqCst) == 0
    }
}

/// Fault injection: [`Process::contain`] panics in the entry point named
/// by [`faults::arm`] on this thread.
#[cfg(test)]
pub mod faults {
    use std::cell::Cell;

    use super::Entry;

    thread_local! {
        static ARMED: Cell<Option<Entry>> = const { Cell::new(None) };
    }

    /// Makes the next call of `entry` on this thread panic.
    pub fn arm(entry: Entry) {
        ARMED.with(|armed| armed.set(Some(entry)));
    }

    pub(super) fn inject(entry: Entry) {
        if ARMED.with(|armed| armed.get()) == Some(entry) {
            ARMED.with(|armed| armed.set(None));
            panic!("injected fault in {entry:?}");
        }
    }
}
