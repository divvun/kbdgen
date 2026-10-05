//! API, contract and context limits.

use super::*;

// [spec:kbdgen:def:ldml.engine.api+1/test]
#[test]
fn model_round_trips_through_bytes() {
    let mut k = dead_keys(Normalization::Disabled, true);
    k.emoji.key = Some(EmojiKey {
        scan_code: 0x34,
        modifiers: Modifiers::of(&[Cmd]),
    });
    let m = model(k);
    let bytes = m.keyboard().to_bytes().unwrap();
    let decoded = Model::from_bytes(&bytes).unwrap();
    assert_eq!(decoded, m);
    assert_eq!(decoded.context_len(), 3);
    assert_eq!(
        decoded.preserved_keys(),
        Some(EmojiKey {
            scan_code: 0x34,
            modifiers: Modifiers::of(&[Cmd])
        })
    );
    assert!(matches!(Model::from_bytes(b"DVKB"), Err(Error::Decode(_))));
    assert!(matches!(
        Model::from_bytes(b"nope and more"),
        Err(Error::Decode(_))
    ));
    let options = Options {
        backspace: BackspacePolicy::CodePoint,
        ..Options::default()
    };
    assert_eq!(
        Model::from_bytes(&bytes)
            .unwrap()
            .with_options(options)
            .options(),
        options
    );
}

// [spec:kbdgen:def:ldml.engine.api+1/test]
#[test]
fn invalid_keyboard_is_an_error() {
    let mut k = dead_keys(Normalization::Disabled, false);
    k.context_len = 9;
    assert!(matches!(
        Model::from_keyboard(k, Options::default()),
        Err(Error::Invalid(e)) if e.invariant == kbd_model::Invariant::ContextLen
    ));
    let e = Error::NormalizationUnsupported;
    assert!(e.to_string().contains("normalization"));
}

// [spec:kbdgen:def:ldml.engine.state/test]
#[test]
fn state_resets_to_default() {
    let m = model(dead_keys(Normalization::Disabled, false));
    let mut s = Session::new(&m);
    s.scan(Q);
    assert_ne!(s.state, State::default());
    let copy = s.state.clone();
    assert_eq!(copy, s.state);
    assert!(State::default().tail().is_empty());
    assert!(m.pending_markers(&State::default()).is_empty());
}

// [spec:kbdgen:req:ldml.engine.contract+1/test]
#[test]
fn equal_inputs_give_equal_results() {
    let a = model(dead_keys(Normalization::Disabled, true));
    let b = Model::from_bytes(&a.keyboard().to_bytes().unwrap()).unwrap();
    let events = [
        KeyEvent::new(Key::Scan(Q)),
        KeyEvent::new(Key::Scan(A)),
        KeyEvent::new(Key::Scan(W)),
        KeyEvent::new(Key::Backspace),
        KeyEvent::new(Key::Scan(Q)),
        KeyEvent::new(Key::Commit),
        KeyEvent::with(Key::Scan(A), ModifierState::shift()),
    ];
    let mut sa = Session::new(&a);
    let mut sb = Session::new(&b);
    for event in events {
        assert_eq!(sa.press(event.clone()), sb.press(event));
        assert_eq!(sa.state, sb.state);
    }
    assert_eq!(sa.text, "á´A");
}

// [spec:kbdgen:req:ldml.engine.contract+1/test]
// [spec:kbdgen:req:ldml.engine.tsf+1/test]
#[test]
fn authoritative_flag_changes_nothing() {
    let m = model(dead_keys(Normalization::Disabled, false));
    let (_, pending) = m.key(
        &State::default(),
        &Context::new("e"),
        &KeyEvent::new(Key::Scan(Q)),
    );
    let authoritative = Context::new("e");
    let cached = Context {
        authoritative: false,
        ..authoritative.clone()
    };
    for event in [Key::Scan(E), Key::Backspace, Key::Commit, Key::Scan(Q)] {
        let event = KeyEvent::new(event);
        assert_eq!(
            m.key(&pending, &authoritative, &event),
            m.key(&pending, &cached, &event)
        );
    }
}

/// A rule matching 63 `a`s, so `context_len` is 64.
fn long_rule_keyboard() -> Keyboard {
    let mut k = keyboard(Normalization::Disabled);
    k.simple = vec![TransformGroup::Rules(vec![rule(
        vec![
            TreeItem {
                atom: TreeAtom::Char('a'),
                min: 9,
                max: 9,
            };
            7
        ],
        vec![to_text("Z")],
    )])];
    k
}

// [spec:kbdgen:req:ldml.engine.contract+1/test]
// [spec:kbdgen:def:ldml.engine.action/test]
#[test]
fn context_limit_bounds_delete_and_tail() {
    let m = model(long_rule_keyboard());
    assert_eq!(m.context_len(), 64);
    let emit = KeyEvent::new(Key::Emit("a".to_string()));
    let (action, state) = m.key(&State::default(), &Context::new("a".repeat(62)), &emit);
    assert_eq!(action, edit(62, "Z", ""));
    assert_eq!(state.tail().len(), 1);
    // Only the last 64 scalar values are read.
    let long = "b".repeat(100) + &"a".repeat(62);
    let (action, state) = m.key(&State::default(), &Context::new(long), &emit);
    assert_eq!(action, edit(62, "Z", ""));
    assert_eq!(state.tail().len(), 3);
    let (action, state) = m.key(
        &State::default(),
        &Context::new("b".repeat(200)),
        &KeyEvent::new(Key::Emit("c".to_string())),
    );
    assert_eq!(action, typed("c"));
    assert_eq!(state.tail().len(), 64);
    assert_eq!(state.tail().plain(), "b".repeat(63) + "c");
}

// [spec:kbdgen:sem:ldml.engine.output.segment+1/test]
#[test]
fn tail_keeps_markers_among_last_scalars() {
    let mut k = dead_keys(Normalization::Disabled, false);
    k.simple.clear();
    let m = model(k);
    assert_eq!(m.context_len(), 1);
    let mut s = Session::new(&m);
    s.emit("ab");
    s.scan(W);
    s.scan(Q);
    assert_eq!(
        s.state.tail().elements(),
        [
            TextElem::Char('b'),
            TextElem::Marker(GRAVE),
            TextElem::Marker(ACUTE)
        ]
    );
    s.emit("c");
    assert_eq!(s.state.tail().elements(), [TextElem::Char('c')]);
}

// [spec:kbdgen:thm:ldml.engine.no-markers-out/test]
#[test]
fn markers_never_reach_the_application() {
    let m = model(transform_keyboard());
    let mut s = Session::new(&m);
    for _ in 0..3 {
        let action = s.id("mark");
        assert_eq!(action, typed(""));
    }
    assert_eq!(s.emit("b"), typed("b!"));
    assert_eq!(s.text, "b!");
    assert_eq!(s.state.tail().markers().count(), 3);
    let (action, _) = m.key(&s.state, &s.context(), &KeyEvent::new(Key::Commit));
    assert_eq!(action, typed(""));
}

// [spec:kbdgen:req:ldml.crate.ffi/test]
// [spec:kbdgen:req:ldml.crate.engine+1/test]
#[test]
fn public_types_are_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Model>();
    assert_send_sync::<State>();
    assert_send_sync::<Context>();
    assert_send_sync::<KeyEvent>();
    assert_send_sync::<Key>();
    assert_send_sync::<Gesture>();
    assert_send_sync::<ModifierState>();
    assert_send_sync::<Action>();
    assert_send_sync::<Options>();
    assert_send_sync::<Error>();
}
