//! Markers, dead keys, preedit and commit.

use super::*;

// [spec:kbdgen:sem:ldml.engine.preedit+1/test]
// [spec:kbdgen:sem:ldml.engine.insert/test]
#[test]
fn dead_key_composes_with_next_key() {
    let m = model(dead_keys(Normalization::Disabled, false));
    let mut s = Session::new(&m);
    assert_eq!(s.scan(Q), edit(0, "", "´"));
    assert_eq!(s.pending(), ["acute"]);
    assert_eq!(s.scan(A), edit(0, "á", ""));
    assert!(s.pending().is_empty());
    assert_eq!(s.text, "á");
}

// [spec:kbdgen:def:ldml.scope.v1+1/test]
// [spec:kbdgen:sem:ldml.engine.transforms/test]
#[test]
fn unmatched_dead_key_emits_standalone_then_key() {
    let m = model(dead_keys(Normalization::Disabled, false));
    let mut s = Session::new(&m);
    s.scan(Q);
    assert_eq!(s.scan(R), edit(0, "´x", ""));
    s.scan(Q);
    assert_eq!(s.scan(SPACE), edit(0, "´", ""));
    assert_eq!(s.text, "´x´");
}

// [spec:kbdgen:sem:ldml.engine.commit+1/test]
#[test]
fn commit_inserts_flush_of_pending_marker() {
    let m = model(dead_keys(Normalization::Disabled, false));
    let mut s = Session::new(&m);
    s.emit("b");
    s.scan(Q);
    assert_eq!(s.press(KeyEvent::new(Key::Commit)), edit(0, "´", ""));
    assert_eq!(s.text, "b´");
    assert!(s.state.tail().is_plain());
    assert_eq!(s.press(KeyEvent::new(Key::Commit)), edit(0, "", ""));
}

// [spec:kbdgen:sem:ldml.engine.commit+1/test]
#[test]
fn commit_drops_markers_without_flush() {
    let m = model(dead_keys(Normalization::Disabled, false));
    let mut s = Session::new(&m);
    s.scan(W);
    assert_eq!(s.preedit, "");
    s.scan(Q);
    assert_eq!(s.preedit, "´");
    assert_eq!(s.pending(), ["grave", "acute"]);
    assert_eq!(s.press(KeyEvent::new(Key::Commit)), edit(0, "´", ""));
    assert!(s.state.tail().is_plain());
}

// [spec:kbdgen:sem:ldml.engine.preedit+1/test]
#[test]
fn preedit_concatenates_trailing_flush_outputs() {
    let mut k = dead_keys(Normalization::Disabled, false);
    k.flush.insert(GRAVE, "`".to_string());
    let m = model(k);
    let mut s = Session::new(&m);
    s.scan(Q);
    assert_eq!(s.scan(W), edit(0, "", "´`"));
    assert_eq!(s.pending(), ["acute", "grave"]);
}

// [spec:kbdgen:def:ldml.engine.state/test]
#[test]
fn shortcut_pass_keeps_state_and_host_commits() {
    let m = model(dead_keys(Normalization::Disabled, false));
    let mut s = Session::new(&m);
    s.scan(Q);
    let before = s.state.clone();
    let ctrl = ModifierState {
        ctrl_l: true,
        ..ModifierState::default()
    };
    let (action, state) = m.key(&before, &s.context(), &KeyEvent::with(Key::Scan(A), ctrl));
    assert_eq!(action, Action::Pass);
    assert_eq!(state, before);
    s.scan_with(A, ctrl);
    assert_eq!(s.text, "´");
    assert_eq!(s.state, State::default());
}

// [spec:kbdgen:sem:ldml.engine.context+1/test]
#[test]
fn changed_context_drops_markers() {
    let m = model(dead_keys(Normalization::Disabled, false));
    let mut s = Session::new(&m);
    s.emit("ab");
    s.scan(Q);
    // The caret moved without the host resetting the state: the suffix
    // test fails, so the marker is gone.
    let moved = Context::new("zz");
    let (action, _) = m.key(&s.state, &moved, &KeyEvent::new(Key::Scan(A)));
    assert_eq!(action, edit(0, "a", ""));
    // The unchanged context keeps it.
    let (action, _) = m.key(&s.state, &Context::new("ab"), &KeyEvent::new(Key::Scan(A)));
    assert_eq!(action, edit(0, "á", ""));
    // Longer text with the same suffix still keeps it.
    let (action, _) = m.key(&s.state, &Context::new("xab"), &KeyEvent::new(Key::Scan(A)));
    assert_eq!(action, edit(0, "á", ""));
}

// [spec:kbdgen:sem:ldml.engine.hardware/test]
#[test]
fn gap_empty_and_missing_keys_consume() {
    let m = model(dead_keys(Normalization::Disabled, false));
    let mut s = Session::new(&m);
    s.scan(Q);
    let pending = s.state.clone();
    assert_eq!(s.scan(T), edit(0, "", "´"));
    assert_eq!(s.state, pending);
    assert_eq!(s.scan(S), edit(0, "", "´"));
    // Z is in the form but the base layer has no key there.
    assert_eq!(s.scan(Z), edit(0, "", "´"));
    assert_eq!(s.state, pending);
    // A scan code outside the form passes.
    assert_eq!(s.scan(0x02), Action::Pass);
}

// [spec:kbdgen:sem:ldml.engine.hardware/test]
#[test]
fn keyboard_without_hardware_passes_scan_codes() {
    let mut k = dead_keys(Normalization::Disabled, false);
    k.hardware = None;
    let m = model(k);
    let (action, _) = m.key(
        &State::default(),
        &Context::new(""),
        &KeyEvent::new(Key::Scan(Q)),
    );
    assert_eq!(action, Action::Pass);
}

// [spec:kbdgen:def:ldml.engine.event+1/test]
#[test]
fn repeat_is_handled_like_a_press() {
    let m = model(dead_keys(Normalization::Disabled, false));
    let state = State::default();
    let context = Context::new("x");
    let press = KeyEvent::new(Key::Scan(A));
    let repeat = KeyEvent {
        repeat: true,
        ..press.clone()
    };
    assert_eq!(
        m.key(&state, &context, &press),
        m.key(&state, &context, &repeat)
    );
}
