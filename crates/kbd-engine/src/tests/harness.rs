//! The conformance harness (`ldml.test.harness`) over the dead-key
//! keyboard.

use super::*;
use crate::harness::Harness;

fn scan_event(code: u8) -> KeyEvent {
    KeyEvent::new(Key::Scan(code))
}

// [spec:kbdgen:def:ldml.test.harness+1/test]
// [spec:kbdgen:req:tsf.test.engine/test]
#[test]
fn edits_delete_scalars_then_append() {
    let m = model(dead_keys(Normalization::Disabled, false));
    let mut h = Harness::new(&m);
    h.set_document("xyz");
    assert_eq!(h.send(&scan_event(Q)), &edit(0, "", "´"));
    assert_eq!((h.document(), h.preedit()), ("xyz", "´"));
    h.send(&scan_event(R));
    assert_eq!((h.document(), h.preedit()), ("xyz´x", ""));
    assert!(!h.passed());
    assert_eq!(h.last_action(), Some(&edit(0, "´x", "")));
}

// [spec:kbdgen:def:ldml.test.harness+1/test]
#[test]
fn pass_changes_nothing_but_the_record() {
    let m = model(dead_keys(Normalization::Disabled, false));
    let mut h = Harness::new(&m);
    h.set_document("ab");
    h.send(&scan_event(Q));
    let before = h.state().clone();
    assert_eq!(h.send(&scan_event(0x01)), &Action::Pass);
    assert!(h.passed());
    assert_eq!((h.document(), h.preedit()), ("ab", "´"));
    assert_eq!(h.state(), &before);
}

// [spec:kbdgen:def:ldml.test.harness+1/test]
#[test]
fn context_is_the_document_tail() {
    let m = model(dead_keys(Normalization::Disabled, false));
    let n = m.context_len();
    let mut h = Harness::new(&m);
    let whole = "b".repeat(n);
    h.set_document(&whole);
    assert_eq!(
        h.context(),
        Context {
            text: whole.clone(),
            authoritative: true,
            at_start: true,
        }
    );
    h.set_document(&alloc::format!("a{whole}"));
    assert_eq!(h.context().text, whole);
    assert!(
        !h.context().at_start,
        "a cut context is not the whole document"
    );
    h.set_document(&whole);
    h.set_at_start(false);
    assert!(!h.context().at_start);
}

// [spec:kbdgen:def:ldml.test.harness+1/test]
#[test]
fn reset_and_new_document_drop_markers() {
    let m = model(dead_keys(Normalization::Disabled, false));
    let mut h = Harness::new(&m);
    h.send(&scan_event(Q));
    h.reset();
    assert_eq!(h.state(), &State::default());
    h.send(&scan_event(A));
    assert_eq!(h.document(), "a");
    h.send(&scan_event(Q));
    h.set_document("b");
    h.send(&scan_event(A));
    assert_eq!(h.document(), "ba", "the markers went with the old text");
}
