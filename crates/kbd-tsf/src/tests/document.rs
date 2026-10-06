//! Edits, preedit, cache and resets through `Document` and the planner.

use kbd_engine::{Action, Key, KeyEvent, ModifierState, State};

use super::Typist;
use super::fake::Fake;
use super::fixture::*;
use crate::document::Flags;

// [spec:kbdgen:req:tsf.edit.preedit+1/test]
// [spec:kbdgen:req:tsf.edit.apply+1/test]
// [spec:kbdgen:req:tsf.test.host/test]
#[test]
fn store_shows_dead_key_preedit_then_composes() {
    let mut t = Typist::store();
    assert!(t.scan(ACUTE));
    assert_eq!(t.fake.preedit(), "´");
    assert!(t.document.composing());
    assert!(t.scan(B));
    assert_eq!(t.fake.text(), "b\u{301}");
    assert_eq!(t.fake.composition, None);
    assert!(!t.document.composing());
}

// [spec:kbdgen:req:tsf.edit.cache/test]
// [spec:kbdgen:req:tsf.edit.preedit+1/test]
// [spec:kbdgen:req:tsf.edit.inject+1/test]
#[test]
fn transitory_context_composes_without_preedit() {
    let mut t = Typist::transitory();
    assert!(t.scan(ACUTE));
    assert_eq!(t.fake.text(), "");
    assert_eq!(t.fake.composition, None);
    assert!(t.scan(B));
    assert_eq!(t.fake.text(), "b\u{301}");
    assert_eq!(t.document.cache(), "b\u{301}");
    assert!(t.fake.injected.is_empty());
}

// [spec:kbdgen:req:tsf.edit.inject+1/test]
// [spec:kbdgen:req:tsf.edit.cache/test]
#[test]
fn transitory_deletion_goes_through_injection() {
    let mut t = Typist::transitory();
    t.fake.units = "xq".encode_utf16().collect();
    t.fake.select(2, 2);
    t.scan(Q);
    assert!(t.scan(APOSTROPHE));
    assert_eq!(t.fake.injected, [(1, "ʠ".to_owned())]);
    assert_eq!(t.fake.text(), "xqʠ");
    assert_eq!(t.document.cache(), "ʠ");
}

// [spec:kbdgen:req:tsf.edit.apply+1/test]
// [spec:kbdgen:thm:tsf.edit.units/test]
#[test]
fn store_replaces_surrogate_pair_before_caret() {
    let mut t = Typist::store();
    t.fake = Fake::with_text("x");
    t.scan(Z);
    t.scan(APOSTROPHE);
    assert_eq!(t.fake.text(), "x𝕫");
    t.scan(APOSTROPHE);
    assert_eq!(t.fake.text(), "xʐ");
    assert!(t.fake.injected.is_empty());
}

// [spec:kbdgen:req:tsf.edit.inject+1/test]
#[test]
fn transitory_surrogate_deletion_sends_one_backspace() {
    let mut t = Typist::transitory();
    t.scan(Z);
    t.scan(APOSTROPHE);
    t.scan(APOSTROPHE);
    assert_eq!(t.fake.injected, [(1, "𝕫".to_owned()), (1, "ʐ".to_owned())]);
    assert_eq!(t.fake.text(), "ʐ");
}

// [spec:kbdgen:req:tsf.edit.reset+1/test]
#[test]
fn passed_key_commits_shown_preedit() {
    let mut t = Typist::store();
    t.scan(ACUTE);
    assert!(!t.press(KeyEvent::new(Key::Scan(0x01))));
    assert_eq!(t.fake.text(), "´");
    assert_eq!(t.fake.composition, None);
    assert_eq!(t.document.state(), &State::default());
}

// [spec:kbdgen:req:tsf.edit.reset+1/test]
#[test]
fn passed_key_commits_unshown_flush_first() {
    let mut t = Typist::transitory();
    t.scan(Q);
    t.scan(ACUTE);
    assert!(!t.press(KeyEvent::new(Key::Scan(0x01))));
    assert_eq!(t.fake.text(), "q´");
    assert_eq!(t.document.cache(), "");
    assert_eq!(t.document.state(), &State::default());
}

// [spec:kbdgen:req:tsf.edit.session+1/test]
#[test]
fn nonempty_selection_resets_engine_first() {
    let mut t = Typist::store();
    t.fake = Fake::with_text("xy");
    t.scan(ACUTE);
    t.fake.composition = None;
    t.fake.select(0, 1);
    assert!(t.scan(A));
    assert_eq!(t.fake.text(), "ay´");
}

// [spec:kbdgen:req:tsf.edit.preedit+1/test]
// [spec:kbdgen:req:tsf.edit.session+1/test]
#[test]
fn preedit_over_selection_starts_at_caret() {
    let mut t = Typist::store();
    t.fake = Fake::with_text("xy");
    t.fake.select(1, 2);
    assert!(t.scan(ACUTE));
    assert_eq!(t.fake.text(), "x´y");
    assert_eq!(t.fake.composition, Some((1, 2)));
    assert!(t.scan(A));
    assert_eq!(t.fake.text(), "xáy");
}

// [spec:kbdgen:req:tsf.security.disabled+1/test]
#[test]
fn disabled_context_maps_keys_without_preedit_or_cache() {
    let mut t = Typist::new(
        Fake::default(),
        Flags {
            transitory: true,
            disabled: true,
        },
    );
    assert!(t.scan(ACUTE));
    assert_eq!(t.fake.composition, None);
    assert!(t.scan(A));
    assert_eq!(t.fake.text(), "á");
    assert_eq!(t.document.cache(), "");
    let mut stored = Typist::new(
        Fake::default(),
        Flags {
            transitory: false,
            disabled: true,
        },
    );
    stored.scan(ACUTE);
    assert_eq!(stored.fake.composition, None);
    stored.scan(B);
    assert_eq!(stored.fake.text(), "b\u{301}");
}

// [spec:kbdgen:req:tsf.edit.inject+1/test]
// [spec:kbdgen:req:tsf.edit.reset+1/test]
#[test]
fn injection_shortfall_resets_the_context() {
    let mut t = Typist::transitory();
    t.scan(Q);
    t.fake.inject_limit = Some(1);
    assert!(t.scan(APOSTROPHE));
    assert_eq!(t.document.cache(), "");
    assert_eq!(t.document.state(), &State::default());
}

// [spec:kbdgen:req:tsf.keys.claim+1/test]
// [spec:kbdgen:req:tsf.engine.contract/test]
#[test]
fn backspace_claim_depends_on_context_text() {
    let mut t = Typist::store();
    t.fake = Fake::with_text("xʠ");
    assert!(t.press(KeyEvent::new(Key::Backspace)));
    assert_eq!(t.fake.text(), "xq");
    assert!(!t.press(KeyEvent::new(Key::Backspace)));
    assert_eq!(t.fake.text(), "xq");
}

// [spec:kbdgen:req:tsf.keys.claim+1/test]
#[test]
fn backspace_cancels_pending_dead_key() {
    let mut t = Typist::store();
    t.fake = Fake::with_text("x");
    t.scan(ACUTE);
    assert!(t.press(KeyEvent::new(Key::Backspace)));
    assert_eq!(t.fake.text(), "x");
    assert_eq!(t.fake.composition, None);
}

// [spec:kbdgen:req:tsf.keys.claim+1/test]
// [spec:kbdgen:def:tsf.engine.api/test]
// [spec:kbdgen:req:tsf.keys.phases/test]
#[test]
fn deciding_twice_changes_nothing() {
    let t = Typist::store();
    let mut fake = Fake::with_text("q");
    let event = KeyEvent::new(Key::Scan(APOSTROPHE));
    let first = t.document.decide(&t.model, &mut fake, t.flags, &event);
    let second = t.document.decide(&t.model, &mut fake, t.flags, &event);
    assert_eq!(first, second);
    assert_eq!(fake.text(), "q");
    assert!(matches!(first.action, Action::Edit { delete: 1, .. }));
}

// [spec:kbdgen:req:tsf.keys.claim+1/test]
// [spec:kbdgen:req:tsf.engine.contract/test]
#[test]
fn form_key_without_output_is_eaten_silently() {
    let mut t = Typist::store();
    assert!(t.scan_with(A, ModifierState::altgr()));
    assert_eq!(t.fake.text(), "");
    assert!(t.scan(SPACE));
    assert!(!t.scan(0x3B));
}

// [spec:kbdgen:req:tsf.engine.contract/test]
#[test]
fn shortcuts_pass_and_ctrl_altgr_reaches_ctrl_alt() {
    let mut t = Typist::store();
    let ctrl = ModifierState {
        ctrl_l: true,
        ..ModifierState::default()
    };
    assert!(!t.scan_with(Q, ctrl));
    let left_alt = ModifierState {
        alt_l: true,
        ..ModifierState::default()
    };
    assert!(!t.scan_with(Q, left_alt));
    let ctrl_altgr = ModifierState {
        ctrl_l: true,
        ..ModifierState::altgr()
    };
    assert!(t.scan_with(A, ctrl_altgr));
    assert!(t.scan_with(T, ModifierState::altgr()));
    assert_eq!(t.fake.text(), "ät\u{301}");
}

// [spec:kbdgen:req:tsf.keys.claim+1/test]
#[test]
fn b00_extra_modifier_is_consumed_and_selects() {
    let mut t = Typist::store();
    assert!(t.scan(B00));
    let extra = ModifierState {
        extra: [true, false, false],
        ..ModifierState::default()
    };
    assert!(t.scan_with(A, extra));
    assert_eq!(t.fake.text(), "1");
}

// [spec:kbdgen:req:tsf.keys.identity+1/test]
#[test]
fn decimal_key_types_model_decimal() {
    let mut t = Typist::store();
    assert!(t.press(KeyEvent::new(Key::Decimal)));
    assert_eq!(t.fake.text(), ",");
}

// [spec:kbdgen:req:tsf.edit.reset+1/test]
#[test]
fn reset_and_termination_clear_state_and_cache() {
    let mut t = Typist::transitory();
    t.scan(Q);
    t.scan(ACUTE);
    t.document.reset(None);
    assert_eq!(t.document.state(), &State::default());
    assert_eq!(t.document.cache(), "");
    let mut s = Typist::store();
    s.scan(ACUTE);
    s.document.reset(Some(&mut s.fake));
    assert_eq!(s.fake.text(), "´");
    assert_eq!(s.fake.composition, None);
    s.scan(ACUTE);
    s.fake.composition = None;
    s.document.terminated();
    assert!(!s.document.composing());
    assert!(s.scan(A));
    assert_eq!(s.fake.text(), "´´a");
}
