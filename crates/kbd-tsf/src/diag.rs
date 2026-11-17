//! Diagnostics (`tsf.component.logging`): the events the text service
//! records (`tsf.component.events`), and the per-thread gate that decides
//! whether it records them.
//!
//! An [`Event`] is `Copy + 'static`, so no field can own a string or borrow
//! a host's buffer: typed text, context text and key identities cannot
//! reach a diagnostic. Its `&'static str` fields are literals compiled into
//! the text service (interface names, reason codes, the file names of its
//! own DLLs) or a panic's source file. Text read at run time could only
//! become `'static` by leaking it, which `tests/self_contained.rs` forbids.
//! The writer adds the process's image name itself. On Windows `crate::etw`
//! encodes each event for ETW TraceLogging and `win::trace` writes it.

use std::cell::Cell;
use std::panic::Location;

use crate::guard::Entry;
use crate::locate::Inert;

/// `TF_TMF_SECUREMODE`: the welcome screen, UAC prompts, credential UI.
pub const SECURE_MODE: u32 = 0x2;
/// `TF_TMF_IMMERSIVEMODE`: a Store app.
pub const IMMERSIVE_MODE: u32 = 0x4000_0000;

/// What the text service records.
// [spec:kbdgen:req:tsf.component.events]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    /// The text service activated on a thread manager whose
    /// `GetActiveFlags` gave `flags`.
    Activated {
        flags: u32,
    },
    /// Activation failed: a call on `interface` returned `hresult`.
    ActivationFailed {
        interface: &'static str,
        hresult: i32,
    },
    KeyboardLoaded,
    /// The active profile is inert (`tsf.data.inert`).
    KeyboardInert(Inert),
    /// A panic was contained in `entry` (`tsf.component.panic`). Only its
    /// location is kept: its message could hold text.
    Panicked {
        entry: Option<Entry>,
        location: Option<&'static Location<'static>>,
    },
    /// What `DllRegisterServer` returned.
    Register(Registration),
    /// What `DllUnregisterServer` returned.
    Unregister(Registration),
}

/// The outcome of a registration export: its `HRESULT`, and, when
/// `crate::registration` refused, the refusal's reason code and the name
/// of the file it is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Registration {
    pub hresult: i32,
    pub refusal: Option<&'static str>,
    pub file: Option<&'static str>,
}

/// Fails to compile if an [`Event`] could hold a `String`, a `Vec` or a
/// borrow of a host's buffer.
const fn copy_and_static<T: Copy + 'static>() {}
const _: () = copy_and_static::<Event>();

/// Whether this thread may record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gate {
    /// The text service is not activated on this thread, or its activation
    /// flags could not be read. Nothing is known of the host, which might
    /// be the secure desktop.
    Closed,
    /// Inside `DllRegisterServer` or `DllUnregisterServer`, which an
    /// installer or `regsvr32` calls, never the secure desktop, and where
    /// no text exists.
    Installer,
    /// Activated with these `GetActiveFlags`.
    Activated(u32),
}

impl Gate {
    // [spec:kbdgen:req:tsf.component.logging+1]
    // [spec:kbdgen:req:tsf.security.secure-mode+1]
    pub const fn open(self) -> bool {
        match self {
            Gate::Closed => false,
            Gate::Installer => true,
            Gate::Activated(flags) => flags & (SECURE_MODE | IMMERSIVE_MODE) == 0,
        }
    }
}

thread_local! {
    static GATE: Cell<Gate> = const { Cell::new(Gate::Closed) };
}

fn set(gate: Gate) -> Gate {
    GATE.try_with(|current| current.replace(gate))
        .unwrap_or(Gate::Closed)
}

/// Sets this thread's gate on activation from `GetActiveFlags`; `None`,
/// flags that could not be read, keeps it closed.
// [spec:kbdgen:req:tsf.component.logging+1]
// [spec:kbdgen:req:tsf.component.interfaces+2]
pub fn activated(flags: Option<u32>) {
    set(flags.map_or(Gate::Closed, Gate::Activated));
}

/// Closes this thread's gate on deactivation.
pub fn deactivated() {
    set(Gate::Closed);
}

/// Runs `body`, the body of a registration export, with this thread's
/// gate open, and restores the gate afterwards, also on a panic.
// [spec:kbdgen:req:tsf.component.logging+1]
pub fn installing<T>(body: impl FnOnce() -> T) -> T {
    struct Restore(Gate);
    impl Drop for Restore {
        fn drop(&mut self) {
            set(self.0);
        }
    }
    let _restore = Restore(set(Gate::Installer));
    body()
}

/// Records `event` if this thread's gate is open. It never panics and
/// never fails: an event that cannot be written is dropped.
// [spec:kbdgen:req:tsf.component.logging+1]
pub fn record(event: Event) {
    if GATE.try_with(Cell::get).is_ok_and(Gate::open) {
        emit(event);
    }
}

fn emit(event: Event) {
    #[cfg(test)]
    recorder::push(event);
    #[cfg(windows)]
    crate::win::trace::write(event);
    #[cfg(not(any(test, windows)))]
    let _ = event;
}

/// What this thread recorded, for tests: the recorder sits behind the gate
/// beside the ETW writer, so an event missing here was not written.
#[cfg(test)]
pub mod recorder {
    use std::cell::RefCell;

    use super::Event;

    thread_local! {
        static EVENTS: RefCell<Vec<Event>> = const { RefCell::new(Vec::new()) };
    }

    pub(super) fn push(event: Event) {
        EVENTS.with_borrow_mut(|events| events.push(event));
    }

    /// The events recorded on this thread since the last call.
    pub fn take() -> Vec<Event> {
        EVENTS.with_borrow_mut(std::mem::take)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::guard::{Panicked, contain, faults};
    use crate::locate::Reason;
    use std::sync::atomic::AtomicBool;

    fn every_event() -> Vec<Event> {
        vec![
            Event::Activated { flags: 0 },
            Event::ActivationFailed {
                interface: "ITfKeystrokeMgr::AdviseKeyEventSink",
                hresult: 0x8000_FFFF_u32 as i32,
            },
            Event::KeyboardLoaded,
            Event::KeyboardInert(Reason::NoLayout.into()),
            Event::Panicked {
                entry: Some(Entry::OnKeyDown),
                location: Some(Location::caller()),
            },
            Event::Register(Registration {
                hresult: 0,
                refusal: None,
                file: None,
            }),
            Event::Unregister(Registration {
                hresult: 0x8007_0002_u32 as i32,
                refusal: Some("missing"),
                file: Some("divvun_tip_x64.dll"),
            }),
        ]
    }

    // [spec:kbdgen:req:tsf.component.logging+1/test]
    // [spec:kbdgen:req:tsf.security.secure-mode+1/test]
    #[test]
    fn nothing_is_written_in_secure_or_immersive_mode() {
        recorder::take();
        for flags in [
            SECURE_MODE,
            IMMERSIVE_MODE,
            SECURE_MODE | IMMERSIVE_MODE,
            IMMERSIVE_MODE | 0x40,
        ] {
            activated(Some(flags));
            every_event().into_iter().for_each(record);
            assert_eq!(recorder::take(), Vec::new(), "flags {flags:#x}");
        }
        deactivated();
    }

    // [spec:kbdgen:req:tsf.component.logging+1/test]
    #[test]
    fn nothing_is_written_before_activation_or_after_deactivation() {
        recorder::take();
        every_event().into_iter().for_each(record);
        assert_eq!(recorder::take(), Vec::new(), "never activated");
        activated(None);
        every_event().into_iter().for_each(record);
        assert_eq!(recorder::take(), Vec::new(), "flags unknown");
        activated(Some(0));
        deactivated();
        every_event().into_iter().for_each(record);
        assert_eq!(recorder::take(), Vec::new(), "deactivated");
    }

    // [spec:kbdgen:req:tsf.component.logging+1/test]
    // [spec:kbdgen:req:tsf.component.events/test]
    #[test]
    fn written_on_the_desktop_and_while_installing() {
        recorder::take();
        activated(Some(0x40));
        every_event().into_iter().for_each(record);
        assert_eq!(recorder::take(), every_event());
        deactivated();
        let inside = installing(|| {
            record(Event::KeyboardLoaded);
            42
        });
        assert_eq!(inside, 42);
        record(Event::KeyboardLoaded);
        assert_eq!(recorder::take(), [Event::KeyboardLoaded]);
    }

    #[test]
    fn installing_restores_the_gate_after_a_panic() {
        activated(Some(SECURE_MODE));
        let unwound = std::panic::catch_unwind(|| installing(|| panic!("in registration")));
        assert!(unwound.is_err());
        assert_eq!(GATE.with(Cell::get), Gate::Activated(SECURE_MODE));
        deactivated();
    }

    #[derive(Debug, PartialEq)]
    struct Hr;

    impl From<Panicked> for Hr {
        fn from(_: Panicked) -> Self {
            Hr
        }
    }

    // [spec:kbdgen:req:tsf.component.logging+1/test]
    // [spec:kbdgen:req:tsf.component.events/test]
    // [spec:kbdgen:req:tsf.component.panic/test]
    #[test]
    fn a_panic_records_its_entry_and_location_only() {
        recorder::take();
        activated(Some(0));
        let poison = AtomicBool::new(false);
        faults::arm(Entry::OnKeyDown);
        let result: Result<(), Hr> = contain(&poison, Entry::OnKeyDown, || Ok(()));
        assert_eq!(result, Err(Hr));
        let events = recorder::take();
        let [
            Event::Panicked {
                entry: Some(Entry::OnKeyDown),
                location: Some(location),
            },
        ] = events.as_slice()
        else {
            panic!("one panic event, got {events:?}");
        };
        assert!(location.file().ends_with("guard.rs"), "{location}");
        assert!(location.line() > 0);
        assert!(!format!("{events:?}").contains("injected fault"));

        activated(Some(SECURE_MODE));
        faults::arm(Entry::OnKeyDown);
        let _: Result<(), Hr> = contain(&poison, Entry::OnKeyDown, || Ok(()));
        assert_eq!(
            recorder::take(),
            Vec::new(),
            "no panic event in secure mode"
        );
        deactivated();
    }
}
