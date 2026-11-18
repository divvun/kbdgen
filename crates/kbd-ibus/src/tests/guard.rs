use crate::diag::{Event, recorder};
use crate::guard::{ENTRIES, Entry, Process, faults};

// [spec:kbdgen:req:linux.ibus.panic/test]
#[test]
fn injected_panic_gives_fallback_and_poisons() {
    for entry in ENTRIES {
        let process = Process::default();
        faults::arm(entry);
        assert_eq!(process.contain(entry, 0, || 1), 0, "{entry:?}");
        assert!(process.poisoned(), "{entry:?} poisons");
    }
}

// [spec:kbdgen:req:linux.ibus.panic/test]
#[test]
fn poisoned_process_runs_no_body() {
    let process = Process::default();
    faults::arm(Entry::ProcessKeyEvent);
    assert!(!process.contain(Entry::ProcessKeyEvent, false, || true));
    let mut ran = false;
    let reply = process.contain(Entry::FocusIn, "fallback", || {
        ran = true;
        "body"
    });
    assert_eq!((reply, ran), ("fallback", false));
}

#[test]
fn calm_entry_returns_body_result() {
    let process = Process::default();
    faults::arm(Entry::Reset);
    assert_eq!(process.contain(Entry::Enable, 0, || 7), 7);
    assert!(!process.poisoned());
    process.contain(Entry::Reset, (), || ());
}

// [spec:kbdgen:req:linux.ibus.panic/test]
#[test]
fn done_once_poisoned_with_no_engine() {
    let process = Process::default();
    process.created();
    process.created();
    assert!(!process.done());
    faults::arm(Entry::ProcessKeyEvent);
    process.contain(Entry::ProcessKeyEvent, (), || ());
    process.destroyed();
    assert!(!process.done(), "one engine object is left");
    process.destroyed();
    assert!(process.done());
    process.destroyed();
    assert!(process.done());

    let calm = Process::default();
    calm.created();
    calm.destroyed();
    assert!(!calm.done(), "an unpoisoned process keeps serving");
}

// [spec:kbdgen:req:linux.ibus.logging/test]
// [spec:kbdgen:req:linux.ibus.panic/test]
#[test]
fn panic_records_entry_and_location_only() {
    recorder::take();
    let process = Process::default();
    faults::arm(Entry::SetSurroundingText);
    process.contain(Entry::SetSurroundingText, (), || ());
    let events = recorder::take();
    let [
        Event::Panicked {
            entry: Some(Entry::SetSurroundingText),
            location: Some(location),
        },
    ] = events.as_slice()
    else {
        panic!("one panic event, got {events:?}");
    };
    assert!(location.file().ends_with("guard.rs"), "{location}");
    let line = events[0].to_string();
    assert!(
        line.starts_with("panic in SetSurroundingText at "),
        "{line}"
    );
    assert!(!line.contains("injected"), "{line}");
}

#[test]
fn every_entry_is_listed_once() {
    for (i, entry) in ENTRIES.iter().enumerate() {
        assert!(!ENTRIES[..i].contains(entry), "{entry:?} twice");
    }
}

// [spec:kbdgen:req:linux.ibus.logging/test]
#[test]
fn entry_names_are_the_dbus_members() {
    for entry in ENTRIES {
        let name = entry.name();
        let debug = format!("{entry:?}");
        let member = debug
            .strip_prefix("Get")
            .or_else(|| debug.strip_prefix("Set").filter(|_| name == "ContentType"))
            .unwrap_or(&debug);
        assert_eq!(name, member);
    }
}
