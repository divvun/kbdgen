//! Touch sets, gestures, layer switches and test keys.

use super::*;

/// Phone, tablet and large touch sets with gestures and layer switches.
fn touch_keyboard() -> Keyboard {
    let mut k = keyboard(Normalization::Disabled);
    let mut a = ModelKey::new("a", text("a"));
    a.long_press = vec![1, 2];
    a.long_press_default = Some(2);
    a.multi_tap = vec![3, 4];
    a.flick = Some(0);
    let mut shift = ModelKey::new("shift", Text::new());
    shift.role = Some(Role::Shift);
    shift.layer_id = Some("shift".to_string());
    let mut to_symbols = ModelKey::new("to-symbols", Text::new());
    to_symbols.layer_id = Some("symbols".to_string());
    let mut upper = ModelKey::new("A", text("A"));
    upper.layer_id = Some("base".to_string());
    k.keys = vec![
        a,
        ModelKey::new("a-acute", text("á")),
        ModelKey::new("a-grave", text("à")),
        ModelKey::new("b", text("b")),
        ModelKey::new("c", text("c")),
        shift,
        to_symbols,
        ModelKey::gap("gap"),
        upper,
        ModelKey::new("n", text("n")),
    ];
    k.flicks = vec![Flick {
        id: "flick-a".to_string(),
        segments: vec![
            FlickSegment {
                directions: vec![Direction::S, Direction::E],
                key: 9,
            },
            FlickSegment {
                directions: vec![Direction::N],
                key: 3,
            },
        ],
    }];
    let touch_layer = |id: &str, rows: Vec<Vec<u16>>| TouchLayer {
        id: id.to_string(),
        rows,
    };
    k.touch = vec![
        TouchSet {
            name: Some("phone".to_string()),
            bottom_row: BottomRow::Host,
            min_device_width: None,
            layers: vec![
                touch_layer("base", vec![vec![0, 5, 6, 7]]),
                touch_layer("shift", vec![vec![8]]),
                touch_layer("symbols", vec![vec![3]]),
            ],
            base: 0,
        },
        TouchSet {
            name: Some("tablet".to_string()),
            bottom_row: BottomRow::Host,
            min_device_width: Some(95),
            layers: vec![touch_layer("base", vec![vec![4]])],
            base: 0,
        },
        TouchSet {
            name: Some("tablet-large".to_string()),
            bottom_row: BottomRow::Authored,
            min_device_width: Some(190),
            layers: vec![touch_layer("base", vec![vec![3]])],
            base: 0,
        },
    ];
    k
}

fn touch(m: &Model, layer: usize, col: usize, gesture: Gesture) -> Action {
    m.key(
        &State::default(),
        &Context::new(""),
        &KeyEvent::new(Key::Touch {
            set: 0,
            layer,
            row: 0,
            col,
            gesture,
        }),
    )
    .0
}

// [spec:kbdgen:sem:ldml.engine.touch/test]
#[test]
fn touch_set_follows_device_width() {
    let m = model(touch_keyboard());
    assert_eq!(m.touch_set_for_width(0), Some(0));
    assert_eq!(m.touch_set_for_width(94), Some(0));
    assert_eq!(m.touch_set_for_width(95), Some(1));
    assert_eq!(m.touch_set_for_width(189), Some(1));
    assert_eq!(m.touch_set_for_width(400), Some(2));
    assert_eq!(m.touch_set_by_name("tablet-large"), Some(2));
    assert_eq!(m.touch_set_by_name("watch"), None);
    let mut k = touch_keyboard();
    k.touch[0].min_device_width = Some(50);
    assert_eq!(model(k).touch_set_for_width(49), None);
}

// [spec:kbdgen:sem:ldml.engine.touch/test]
#[test]
fn hardware_set_presented_as_touch() {
    let mut k = modifier_keyboard();
    k.hardware.as_mut().unwrap().layers[1].id = Some("shift".to_string());
    let m = model(k);
    assert_eq!(m.touch_set_for_width(300), Some(0));
    assert_eq!(touch(&m, 1, 0, Gesture::Tap), typed("s"));
    assert_eq!(
        m.key(
            &State::default(),
            &Context::new(""),
            &KeyEvent::new(Key::Touch {
                set: 1,
                layer: 0,
                row: 0,
                col: 0,
                gesture: Gesture::Tap
            })
        )
        .0,
        Action::Pass
    );
    assert_eq!(
        model(keyboard(Normalization::Disabled)).touch_set_for_width(300),
        None
    );
}

// [spec:kbdgen:sem:ldml.engine.touch.gestures+1/test]
#[test]
fn tap_long_press_and_flick_resolve_keys() {
    let m = model(touch_keyboard());
    assert_eq!(touch(&m, 0, 0, Gesture::Tap), typed("a"));
    assert_eq!(touch(&m, 0, 0, Gesture::LongPress(1)), typed("á"));
    assert_eq!(touch(&m, 0, 0, Gesture::LongPress(2)), typed("à"));
    assert_eq!(touch(&m, 0, 0, Gesture::LongPress(0)), typed("à"));
    assert_eq!(touch(&m, 0, 0, Gesture::LongPress(3)), edit(0, "", ""));
    let flick = |d: Vec<Direction>| touch(&m, 0, 0, Gesture::Flick(d));
    assert_eq!(flick(vec![Direction::S, Direction::E]), typed("n"));
    assert_eq!(flick(vec![Direction::N]), typed("b"));
    assert_eq!(flick(vec![Direction::S]), edit(0, "", ""));
}

// [spec:kbdgen:sem:ldml.engine.touch.gestures+1/test]
#[test]
fn multi_tap_cycles_through_key_list() {
    let m = model(touch_keyboard());
    let tap = |n| touch(&m, 0, 0, Gesture::MultiTap(n));
    assert_eq!(tap(1), typed("a"));
    assert_eq!(tap(2), typed("b"));
    assert_eq!(tap(3), typed("c"));
    assert_eq!(tap(4), typed("a"));
    assert_eq!(tap(5), typed("b"));
    assert_eq!(tap(0), edit(0, "", ""));
}

// [spec:kbdgen:sem:ldml.engine.touch.gestures+1/test]
#[test]
fn roles_gaps_and_holes_pass() {
    let m = model(touch_keyboard());
    assert_eq!(touch(&m, 0, 1, Gesture::Tap), Action::Pass);
    assert_eq!(touch(&m, 0, 3, Gesture::Tap), Action::Pass);
    assert_eq!(touch(&m, 0, 9, Gesture::Tap), Action::Pass);
    assert_eq!(touch(&m, 7, 0, Gesture::Tap), Action::Pass);
}

// [spec:kbdgen:sem:ldml.engine.touch.gestures+1/test]
// [spec:kbdgen:def:ldml.engine.action/test]
#[test]
fn layer_switch_follows_output() {
    let m = model(touch_keyboard());
    assert_eq!(
        touch(&m, 0, 2, Gesture::Tap),
        with_layer(0, "", "", "symbols")
    );
    assert_eq!(
        touch(&m, 1, 0, Gesture::Tap),
        with_layer(0, "A", "", "base")
    );
}

// [spec:kbdgen:sem:ldml.engine.insert/test]
#[test]
fn layer_switch_runs_no_transforms() {
    let mut k = touch_keyboard();
    // Any context would be rewritten if transforms ran on empty output.
    k.simple = vec![TransformGroup::Rules(vec![rule(
        vec![TreeItem::one(TreeAtom::Any)],
        vec![to_text("!")],
    )])];
    let m = model(k);
    let event = KeyEvent::new(Key::Touch {
        set: 0,
        layer: 0,
        row: 0,
        col: 2,
        gesture: Gesture::Tap,
    });
    let (action, _) = m.key(&State::default(), &Context::new("q"), &event);
    assert_eq!(action, with_layer(0, "", "", "symbols"));
    let (action, _) = m.key(
        &State::default(),
        &Context::new("q"),
        &KeyEvent::new(Key::Emit("b".to_string())),
    );
    assert_eq!(action, edit(0, "!", ""));
}

// [spec:kbdgen:sem:ldml.engine.test-keys+1/test]
#[test]
fn id_and_emit_keys_for_tests() {
    let m = model(touch_keyboard());
    let mut s = Session::new(&m);
    // A key on no layer is still pressed by id.
    assert_eq!(s.id("a-grave"), typed("à"));
    assert_eq!(s.id("no-such-key"), edit(0, "", ""));
    assert_eq!(
        s.press(KeyEvent::new(Key::Id {
            id: "a".to_string(),
            gesture: Gesture::LongPress(1)
        })),
        typed("á")
    );
    assert_eq!(s.emit("xyz"), typed("xyz"));
    assert_eq!(s.text, "àáxyz");
}
