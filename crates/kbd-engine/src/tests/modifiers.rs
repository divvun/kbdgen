//! Hardware modifiers: exact matching, AltGr, shortcuts, extra modifiers and the decimal key.

use super::*;

// [spec:kbdgen:sem:ldml.engine.modifiers/test]
#[test]
fn layers_match_modifiers_exactly() {
    let m = model(modifier_keyboard());
    let none = ModifierState::default();
    assert_eq!(out(&m, none), typed("n"));
    assert_eq!(out(&m, ModifierState::shift()), typed("s"));
    let right_shift = ModifierState {
        shift_r: true,
        ..none
    };
    assert_eq!(out(&m, right_shift), typed("s"));
    let caps = ModifierState { caps: true, ..none };
    assert_eq!(out(&m, caps), typed("c"));
    let caps_shift = ModifierState {
        caps: true,
        shift_l: true,
        ..none
    };
    assert_eq!(out(&m, caps_shift), typed("x"));
}

// [spec:kbdgen:sem:ldml.engine.modifiers/test]
#[test]
fn alt_sides_and_other_layer() {
    let m = model(modifier_keyboard());
    let none = ModifierState::default();
    assert_eq!(
        out(
            &m,
            ModifierState {
                alt_r: true,
                ..none
            }
        ),
        typed("r")
    );
    assert_eq!(
        out(
            &m,
            ModifierState {
                alt_l: true,
                ..none
            }
        ),
        typed("l")
    );
    // altL does not tolerate a held right Alt; no layer matches both.
    let both = ModifierState {
        alt_l: true,
        alt_r: true,
        ..none
    };
    assert_eq!(out(&m, both), typed("o"));
    let extra = ModifierState {
        alt_r: true,
        shift_l: true,
        ..none
    };
    assert_eq!(out(&m, extra), typed("o"));
}

// [spec:kbdgen:sem:ldml.engine.modifiers/test]
#[test]
fn caps_has_no_fallback_without_layer() {
    let mut k = keyboard(Normalization::Disabled);
    k.keys = vec![ModelKey::new("a", text("a")), ModelKey::new("A", text("A"))];
    k.hardware = Some(Hardware {
        form: form(),
        min_device_width: None,
        layers: vec![
            layer(vec![set(&[])], vec![vec![0]]),
            layer(vec![set(&[Shift])], vec![vec![1]]),
        ],
    });
    let m = model(k);
    let caps = ModifierState {
        caps: true,
        ..ModifierState::default()
    };
    assert_eq!(out(&m, caps), Action::Pass);
    let caps_shift = ModifierState {
        caps: true,
        shift_l: true,
        ..ModifierState::default()
    };
    assert_eq!(out(&m, caps_shift), Action::Pass);
}

// [spec:kbdgen:def:ldml.scope.v1/test]
#[test]
fn implied_caps_layers_give_macos_caps_shift() {
    // v4 implied layers: caps → uppercase, caps+shift → uppercase too.
    let mut k = keyboard(Normalization::Disabled);
    k.keys = vec![ModelKey::new("a", text("a")), ModelKey::new("A", text("A"))];
    k.hardware = Some(Hardware {
        form: form(),
        min_device_width: None,
        layers: vec![
            layer(vec![set(&[])], vec![vec![0]]),
            layer(
                vec![set(&[Caps]), set(&[Shift]), set(&[Caps, Shift])],
                vec![vec![1]],
            ),
        ],
    });
    let m = model(k);
    let caps_shift = ModifierState {
        caps: true,
        shift_l: true,
        ..ModifierState::default()
    };
    assert_eq!(out(&m, caps_shift), typed("A"));
    assert_eq!(
        out(
            &m,
            ModifierState {
                caps: true,
                ..ModifierState::default()
            }
        ),
        typed("A")
    );
}

// [spec:kbdgen:req:ldml.engine.shortcuts+1/test]
#[test]
fn shortcuts_pass_before_layer_matching() {
    let m = model(modifier_keyboard());
    let none = ModifierState::default();
    assert_eq!(out(&m, ModifierState { cmd: true, ..none }), Action::Pass);
    let cmd_shift = ModifierState {
        cmd: true,
        shift_l: true,
        ..none
    };
    assert_eq!(out(&m, cmd_shift), Action::Pass);
    assert_eq!(
        out(
            &m,
            ModifierState {
                ctrl_l: true,
                ..none
            }
        ),
        Action::Pass
    );
    assert_eq!(
        out(
            &m,
            ModifierState {
                ctrl_r: true,
                ..none
            }
        ),
        Action::Pass
    );
    // Ctrl with Alt is not a shortcut; no ctrl-alt layer here, so Other.
    let ctrl_alt = ModifierState {
        ctrl_l: true,
        alt_l: true,
        ..none
    };
    assert_eq!(out(&m, ctrl_alt), typed("o"));
}

// [spec:kbdgen:req:ldml.engine.shortcuts+1/test]
#[test]
fn left_alt_passes_only_on_windows() {
    let alt_l = ModifierState {
        alt_l: true,
        ..ModifierState::default()
    };
    let mut k = modifier_keyboard();
    k.host = Some(Host::Windows);
    assert_eq!(out(&model(k.clone()), alt_l), Action::Pass);
    k.host = Some(Host::MacOs);
    assert_eq!(out(&model(k.clone()), alt_l), typed("l"));
    // A shared keyboard takes the consumer's host from the options.
    k.host = None;
    let options = Options {
        host: Some(Host::Windows),
        ..Options::default()
    };
    assert_eq!(out(&model_with(k.clone(), options), alt_l), Action::Pass);
    assert_eq!(out(&model(k), alt_l), typed("l"));
}

// [spec:kbdgen:def:ldml.model.native/test]
// [spec:kbdgen:req:ldml.engine.shortcuts+1/test]
#[test]
fn native_only_layers_are_never_selected() {
    // A layer whose sets include `ctrl` is native-only, even for `none`.
    let mut k = keyboard(Normalization::Disabled);
    k.keys = vec![ModelKey::new("a", text("a"))];
    k.hardware = Some(Hardware {
        form: form(),
        min_device_width: None,
        layers: vec![layer(vec![set(&[]), set(&[Ctrl])], vec![vec![0]])],
    });
    let m = model(k);
    assert_eq!(out(&m, ModifierState::default()), Action::Pass);
}

/// None, `altR` or `ctrl alt`, and Other.
fn altgr_keyboard(alt_layer: Vec<ModifierSet>) -> Keyboard {
    let mut k = keyboard(Normalization::Disabled);
    k.keys = vec![
        ModelKey::new("n", text("n")),
        ModelKey::new("g", text("g")),
        ModelKey::new("o", text("o")),
    ];
    k.hardware = Some(Hardware {
        form: form(),
        min_device_width: None,
        layers: vec![
            layer(vec![set(&[])], vec![vec![0]]),
            layer(alt_layer, vec![vec![1]]),
            layer(vec![ModifierSet::Other], vec![vec![2]]),
        ],
    });
    k
}

// [spec:kbdgen:sem:ldml.engine.altgr/test]
// [spec:kbdgen:req:ldml.engine.tsf+1/test]
#[test]
fn altgr_reaches_ctrl_alt_and_alt_r_layers() {
    for sets in [vec![set(&[AltR])], vec![set(&[Alt, Ctrl])]] {
        let m = model(altgr_keyboard(sets.clone()));
        assert_eq!(out(&m, ModifierState::altgr()), typed("g"), "{sets:?}");
    }
    // Without the AltGr flag, Right Alt does not reach `ctrl alt`.
    let m = model(altgr_keyboard(vec![set(&[Alt, Ctrl])]));
    let alt_r = ModifierState {
        alt_r: true,
        ..ModifierState::default()
    };
    assert_eq!(out(&m, alt_r), typed("o"));
    // Ctrl with AltGr reaches a layer naming ctrl and alt.
    let ctrl_altgr = ModifierState {
        ctrl_l: true,
        ..ModifierState::altgr()
    };
    assert_eq!(out(&m, ctrl_altgr), typed("g"));
}

// [spec:kbdgen:sem:ldml.engine.altgr/test]
#[test]
fn altgr_retry_comes_before_other() {
    let m = model(altgr_keyboard(vec![set(&[Ctrl, AltR])]));
    assert_eq!(out(&m, ModifierState::altgr()), typed("g"));
    let m = model(altgr_keyboard(vec![set(&[CtrlR, AltR])]));
    assert_eq!(out(&m, ModifierState::altgr()), typed("o"));
}

/// Extra modifiers bound to Right Ctrl, Caps Lock and B00.
fn extra_keyboard() -> Keyboard {
    let mut k = keyboard(Normalization::Disabled);
    k.windows.extra_modifiers = vec![
        ExtraModifierKey::RightCtrl,
        ExtraModifierKey::CapsLock,
        ExtraModifierKey::B00,
    ];
    k.keys = ["n", "1", "2", "3", "c", "z"]
        .iter()
        .map(|o| ModelKey::new(alloc::format!("k-{o}"), text(o)))
        .collect();
    k.hardware = Some(Hardware {
        form: form(),
        min_device_width: None,
        layers: vec![
            layer(vec![set(&[])], vec![vec![0], vec![], vec![5, 5]]),
            layer(vec![set(&[Extra1])], vec![vec![1]]),
            layer(vec![set(&[Extra2])], vec![vec![2]]),
            layer(vec![set(&[Extra3])], vec![vec![3]]),
            layer(vec![set(&[Caps])], vec![vec![4]]),
        ],
    });
    k
}

// [spec:kbdgen:sem:ldml.engine.extra/test]
#[test]
fn extra_modifier_bindings_select_columns() {
    let m = model(extra_keyboard());
    let none = ModifierState::default();
    // Right Ctrl becomes extra1 and is no longer a shortcut.
    assert_eq!(
        out(
            &m,
            ModifierState {
                ctrl_r: true,
                ..none
            }
        ),
        typed("1")
    );
    // The Caps Lock toggle is cleared; the host sets extra2 while held.
    assert_eq!(out(&m, ModifierState { caps: true, ..none }), typed("n"));
    let held = ModifierState {
        caps: true,
        extra: [false, true, false],
        ..none
    };
    assert_eq!(out(&m, held), typed("2"));
    let b00_held = ModifierState {
        extra: [false, false, true],
        ..none
    };
    assert_eq!(out(&m, b00_held), typed("3"));
    // Left Ctrl is still a shortcut.
    assert_eq!(
        out(
            &m,
            ModifierState {
                ctrl_l: true,
                ..none
            }
        ),
        Action::Pass
    );
}

// [spec:kbdgen:sem:ldml.engine.extra/test]
#[test]
fn bound_b00_key_is_consumed() {
    let m = model(extra_keyboard());
    let mut s = Session::new(&m);
    assert_eq!(s.scan(B00), edit(0, "", ""));
    assert_eq!(s.scan(Z), edit(0, "z", ""));
    // Unbound, B00 is an ordinary key.
    let mut k = extra_keyboard();
    k.windows.extra_modifiers.truncate(2);
    k.hardware.as_mut().unwrap().layers.remove(3);
    let m = model(k);
    let mut s = Session::new(&m);
    assert_eq!(s.scan(B00), edit(0, "z", ""));
}

// [spec:kbdgen:sem:ldml.engine.decimal/test]
#[test]
fn decimal_inserts_or_passes() {
    let mut k = keyboard(Normalization::Disabled);
    let m = model(k.clone());
    let decimal = KeyEvent::new(Key::Decimal);
    let state = State::default();
    assert_eq!(m.key(&state, &Context::new(""), &decimal).0, Action::Pass);
    k.decimal = Some(text(","));
    let m = model(k);
    assert_eq!(
        m.key(&state, &Context::new("1"), &decimal).0,
        edit(0, ",", "")
    );
    let shifted = KeyEvent::with(Key::Decimal, ModifierState::shift());
    assert_eq!(
        m.key(&state, &Context::new("1"), &shifted).0,
        edit(0, ",", "")
    );
    let ctrl = KeyEvent::with(
        Key::Decimal,
        ModifierState {
            ctrl_l: true,
            ..ModifierState::default()
        },
    );
    assert_eq!(m.key(&state, &Context::new("1"), &ctrl).0, Action::Pass);
}
