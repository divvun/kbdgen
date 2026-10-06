//! Backspace: rules, the default policies and LRM/RLM.

use super::*;

// [spec:kbdgen:sem:ldml.engine.backspace.default/test]
#[test]
fn backspace_cancels_pending_dead_key_only() {
    let m = model(dead_keys(Normalization::Disabled, false));
    let mut s = Session::new(&m);
    s.emit("e");
    s.scan(Q);
    assert_eq!(s.backspace(), edit(0, "", ""));
    assert_eq!(s.text, "e");
    assert!(s.pending().is_empty());
    assert_eq!(s.backspace(), Action::Pass);
    assert_eq!(s.text, "");
}

// [spec:kbdgen:sem:ldml.engine.backspace+1/test]
#[test]
fn backspace_rule_runs_instead_of_default() {
    let m = model(dead_keys(Normalization::Disabled, true));
    let mut s = Session::new(&m);
    s.scan(W);
    s.scan(Q);
    // The rule removes one marker; the default would remove both.
    assert_eq!(s.backspace(), edit(0, "", ""));
    assert_eq!(s.pending(), ["grave"]);
    assert_eq!(s.backspace(), edit(0, "", ""));
    assert_eq!(s.backspace(), Action::Pass);
}

// [spec:kbdgen:sem:ldml.engine.backspace+1/test]
#[test]
fn backspace_skips_simple_groups() {
    let mut k = keyboard(Normalization::Disabled);
    k.simple = vec![TransformGroup::Rules(vec![rule(
        chars("ab"),
        vec![to_text("X")],
    )])];
    k.backspace = vec![TransformGroup::Rules(vec![rule(
        chars("abc"),
        vec![to_text("ab")],
    )])];
    let m = model(k);
    let (action, _) = m.key(
        &State::default(),
        &Context::new("abc"),
        &KeyEvent::new(Key::Backspace),
    );
    assert_eq!(action, edit(1, "", ""));
}

// [spec:kbdgen:sem:ldml.engine.backspace+1/test]
#[test]
fn backspace_shortcut_passes() {
    let m = model(dead_keys(Normalization::Disabled, true));
    let mut s = Session::new(&m);
    s.scan(Q);
    let ctrl = ModifierState {
        ctrl_l: true,
        ..ModifierState::default()
    };
    assert_eq!(s.press(KeyEvent::with(Key::Backspace, ctrl)), Action::Pass);
}

// [spec:kbdgen:sem:ldml.engine.backspace+1/test]
#[test]
fn shift_backspace_inserts_lrm_or_rlm() {
    let mut k = keyboard(Normalization::Disabled);
    k.windows.lrm_rlm = true;
    let m = model(k);
    let mut s = Session::new(&m);
    assert_eq!(
        s.press(KeyEvent::with(Key::Backspace, ModifierState::shift())),
        edit(0, "\u{200E}", "")
    );
    let right = ModifierState {
        shift_r: true,
        ..ModifierState::default()
    };
    assert_eq!(
        s.press(KeyEvent::with(Key::Backspace, right)),
        edit(0, "\u{200F}", "")
    );
    assert_eq!(s.backspace(), Action::Pass);
}

// [spec:kbdgen:sem:ldml.engine.backspace.default/test]
// [spec:kbdgen:req:ldml.scope.deferred+1/test]
#[test]
fn code_point_backspace_deletes_one_scalar() {
    let options = Options {
        backspace: BackspacePolicy::CodePoint,
        ..Options::default()
    };
    let m = model_with(dead_keys(Normalization::Disabled, false), options);
    let mut s = Session::new(&m);
    s.emit("ab");
    s.scan(W);
    s.scan(Q);
    // The `b` goes with the markers after it.
    assert_eq!(s.backspace(), edit(1, "", ""));
    assert_eq!(s.text, "a");
    assert!(s.state.tail().is_plain());
    // An emoji ZWJ sequence loses one scalar value at a time.
    let (action, _) = m.key(
        &State::default(),
        &Context::new("\u{1F469}\u{200D}\u{1F4BB}"),
        &KeyEvent::new(Key::Backspace),
    );
    assert_eq!(action, edit(1, "", ""));
    assert_eq!(
        m.key(
            &State::default(),
            &Context::new(""),
            &KeyEvent::new(Key::Backspace)
        )
        .0,
        Action::Pass
    );
}

// [spec:kbdgen:sem:ldml.engine.backspace.default/test]
#[test]
fn code_point_backspace_takes_markers_before() {
    let options = Options {
        backspace: BackspacePolicy::CodePoint,
        ..Options::default()
    };
    let mut k = dead_keys(Normalization::Disabled, false);
    // A key whose output puts a marker before its letter.
    k.keys.push(ModelKey::new(
        "marked-b",
        Text(vec![TextElem::Marker(GRAVE), TextElem::Char('b')]),
    ));
    let m = model_with(k, options);
    let mut s = Session::new(&m);
    s.emit("a");
    s.id("marked-b");
    assert_eq!(s.text, "ab");
    assert_eq!(s.backspace(), edit(1, "", ""));
    assert_eq!(s.text, "a");
    assert!(s.state.tail().is_plain());
    // With only markers left, CodePoint removes them.
    let mut t = Session::new(&m);
    t.scan(Q);
    assert_eq!(t.backspace(), edit(0, "", ""));
    assert_eq!(t.state, State::default());
}
