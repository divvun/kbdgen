//! Panic containment at every entry point (`tsf.component.panic`).
//!
//! TSF loads the text service into every process the user types in, and an
//! unwind that reaches an `extern "system"` boundary aborts that process.
//! Every export and every COM method therefore runs its body through
//! [`contain`], which turns a panic into `E_UNEXPECTED` and poisons the
//! process's text service: from then on it eats no key and requests no edit
//! session until it is unloaded.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Once;
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(panic = "abort")]
compile_error!("kbd-tsf must build with panic = \"unwind\" to contain panics");

/// `E_UNEXPECTED`, which a contained panic returns.
pub const E_UNEXPECTED: i32 = 0x8000_FFFF_u32 as i32;

/// Whether this process's text service has panicked.
pub static POISON: AtomicBool = AtomicBool::new(false);

/// A panic was contained; converts to the `E_UNEXPECTED` error of the
/// entry point's error type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Panicked;

/// Every export and COM method of the text service.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Entry {
    DllGetClassObject,
    DllCanUnloadNow,
    DllRegisterServer,
    DllUnregisterServer,
    CreateInstance,
    LockServer,
    Activate,
    ActivateEx,
    Deactivate,
    OnInitDocumentMgr,
    OnUninitDocumentMgr,
    OnSetFocus,
    OnPushContext,
    OnPopContext,
    OnKeySetFocus,
    OnTestKeyDown,
    OnTestKeyUp,
    OnKeyDown,
    OnKeyUp,
    OnPreservedKey,
    OnEndEdit,
    OnCompositionTerminated,
    OnCompartmentChange,
    OnActivated,
    EnumDisplayAttributeInfo,
    GetDisplayAttributeInfo,
    DoEditSession,
    InfoGetGuid,
    InfoGetDescription,
    InfoGetAttributeInfo,
    InfoSetAttributeInfo,
    InfoReset,
    EnumClone,
    EnumNext,
    EnumReset,
    EnumSkip,
}

/// All entry points, for fault injection.
pub const ENTRIES: [Entry; 36] = [
    Entry::DllGetClassObject,
    Entry::DllCanUnloadNow,
    Entry::DllRegisterServer,
    Entry::DllUnregisterServer,
    Entry::CreateInstance,
    Entry::LockServer,
    Entry::Activate,
    Entry::ActivateEx,
    Entry::Deactivate,
    Entry::OnInitDocumentMgr,
    Entry::OnUninitDocumentMgr,
    Entry::OnSetFocus,
    Entry::OnPushContext,
    Entry::OnPopContext,
    Entry::OnKeySetFocus,
    Entry::OnTestKeyDown,
    Entry::OnTestKeyUp,
    Entry::OnKeyDown,
    Entry::OnKeyUp,
    Entry::OnPreservedKey,
    Entry::OnEndEdit,
    Entry::OnCompositionTerminated,
    Entry::OnCompartmentChange,
    Entry::OnActivated,
    Entry::EnumDisplayAttributeInfo,
    Entry::GetDisplayAttributeInfo,
    Entry::DoEditSession,
    Entry::InfoGetGuid,
    Entry::InfoGetDescription,
    Entry::InfoGetAttributeInfo,
    Entry::InfoSetAttributeInfo,
    Entry::InfoReset,
    Entry::EnumClone,
    Entry::EnumNext,
    Entry::EnumReset,
    Entry::EnumSkip,
];

/// Whether this process's text service is poisoned.
pub fn poisoned() -> bool {
    POISON.load(Ordering::SeqCst)
}

/// Replaces the default panic hook, which would print the panic message,
/// and with it possibly typed text, to the host's standard error.
// [spec:kbdgen:req:tsf.component.logging]
fn silence_panics() {
    static HOOK: Once = Once::new();
    if cfg!(not(test)) {
        HOOK.call_once(|| std::panic::set_hook(Box::new(|_| {})));
    }
}

/// Runs the body of entry point `entry`. A panic in it returns
/// `Panicked` as the error and marks `poison`.
// [spec:kbdgen:req:tsf.component.panic]
pub fn contain<T, E: From<Panicked>>(
    poison: &AtomicBool,
    entry: Entry,
    body: impl FnOnce() -> Result<T, E>,
) -> Result<T, E> {
    silence_panics();
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        #[cfg(test)]
        faults::inject(entry);
        #[cfg(not(test))]
        let _ = entry;
        body()
    }));
    match outcome {
        Ok(result) => result,
        Err(payload) => {
            poison.store(true, Ordering::SeqCst);
            drop(catch_unwind(AssertUnwindSafe(move || drop(payload))));
            Err(E::from(Panicked))
        }
    }
}

/// Fault injection: [`contain`] panics in the entry point named by
/// [`faults::arm`] on this thread.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq)]
    struct Hr(i32);

    impl From<Panicked> for Hr {
        fn from(_: Panicked) -> Self {
            Hr(E_UNEXPECTED)
        }
    }

    // [spec:kbdgen:req:tsf.component.panic/test]
    // [spec:kbdgen:req:tsf.test.host/test]
    #[test]
    fn injected_panic_returns_unexpected_and_poisons() {
        for entry in ENTRIES {
            let poison = AtomicBool::new(false);
            faults::arm(entry);
            let result: Result<u32, Hr> = contain(&poison, entry, || Ok(1));
            assert_eq!(result, Err(Hr(E_UNEXPECTED)), "{entry:?}");
            assert!(poison.load(Ordering::SeqCst), "{entry:?} poisons");
        }
    }

    #[test]
    fn calm_entry_returns_body_result_unpoisoned() {
        let poison = AtomicBool::new(false);
        faults::arm(Entry::OnKeyDown);
        let result: Result<u32, Hr> = contain(&poison, Entry::OnKeyUp, || Ok(7));
        assert_eq!(result, Ok(7));
        let failed: Result<u32, Hr> = contain(&poison, Entry::OnKeyUp, || Err(Hr(5)));
        assert_eq!(failed, Err(Hr(5)));
        assert!(!poison.load(Ordering::SeqCst));
        let _: Result<u32, Hr> = contain(&poison, Entry::OnKeyDown, || Ok(0));
    }

    #[test]
    fn every_entry_is_listed_once() {
        for (i, entry) in ENTRIES.iter().enumerate() {
            assert!(!ENTRIES[..i].contains(entry), "{entry:?} twice");
        }
    }
}
