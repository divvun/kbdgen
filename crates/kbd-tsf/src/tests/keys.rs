//! Key identity, AltGr, extra modifiers and Shift Lock.

use kbd_engine::{Action, Context, Key, KeyEvent, Model, ModifierState, Options, State};
use kbd_model::Windows;

use super::fixture;
use crate::keys::{
    AltGrChords, Held, Role, Stroke, Tracker, VK_PACKET, VK_PROCESSKEY, classify, ctrl_alt,
};

fn down(scan: u8, extended: bool) -> Stroke {
    Stroke {
        vk: 0,
        lparam: (u32::from(scan) << 16) | (u32::from(extended) << 24) | 1,
    }
}

fn b00_bound() -> Windows {
    fixture::keyboard().windows
}

// [spec:kbdgen:req:tsf.keys.identity+1/test]
#[test]
fn keys_are_identified_by_scan_code() {
    let q = Stroke {
        vk: u16::from(b'A'),
        ..down(0x10, false)
    };
    assert_eq!(classify(q, false, false), Role::Engine(Key::Scan(0x10)));
    assert_eq!(
        classify(down(0x0E, false), false, false),
        Role::Engine(Key::Backspace)
    );
    assert_eq!(
        classify(down(0x53, false), false, false),
        Role::Engine(Key::Decimal)
    );
    assert_eq!(classify(down(0x53, true), false, false), Role::Other);
    assert_eq!(classify(down(0x4B, true), false, false), Role::Other);
    assert_eq!(classify(down(0x00, false), false, false), Role::Other);
}

// [spec:kbdgen:req:tsf.keys.identity+1/test]
// [spec:kbdgen:req:tsf.edit.inject+1/test]
#[test]
fn packets_process_keys_and_own_input_skip_engine() {
    for vk in [VK_PACKET, VK_PROCESSKEY] {
        let stroke = Stroke {
            vk,
            ..down(0x10, false)
        };
        assert_eq!(classify(stroke, false, false), Role::Other);
    }
    assert_eq!(classify(down(0x0E, false), true, false), Role::Own);
}

// [spec:kbdgen:req:tsf.keys.claim+1/test]
// [spec:kbdgen:req:tsf.keys.altgr+2/test]
#[test]
fn lone_modifiers_and_altgr_roles() {
    for (scan, extended) in [
        (0x2A, false),
        (0x36, false),
        (0x1D, false),
        (0x1D, true),
        (0x38, false),
        (0x3A, false),
        (0x5B, true),
        (0x5C, true),
    ] {
        assert_eq!(
            classify(down(scan, extended), false, true),
            Role::Modifier,
            "{scan:02x} {extended}"
        );
    }
    assert_eq!(classify(down(0x38, true), false, true), Role::AltGr);
    assert_eq!(classify(down(0x38, true), false, false), Role::Modifier);
}

// [spec:kbdgen:req:tsf.keys.identity+1/test]
#[test]
fn repeat_and_physical_key_come_from_lparam() {
    let stroke = Stroke {
        vk: 0,
        lparam: (1 << 30) | (1 << 24) | (0x38 << 16) | 1,
    };
    assert!(stroke.repeat());
    assert!(stroke.extended());
    assert_eq!(stroke.key(), 0x138);
    assert!(!down(0x38, false).repeat());
}

// [spec:kbdgen:req:tsf.keys.altgr+2/test]
#[test]
fn synthesised_left_ctrl_is_not_ctrl() {
    let mut tracker = Tracker::default();
    tracker.observe(down(0x1D, false), true, 500);
    tracker.observe(down(0x38, true), true, 500);
    let held = Held {
        ctrl_l: true,
        alt_r: true,
        ..Held::default()
    };
    let m = tracker.modifiers(held, &Windows::default(), true);
    assert!(!m.ctrl_l && m.alt_r && m.altgr);

    let mut tracker = Tracker::default();
    tracker.observe(down(0x1D, false), true, 400);
    tracker.observe(down(0x38, true), true, 500);
    let m = tracker.modifiers(held, &Windows::default(), true);
    assert!(m.ctrl_l && m.alt_r);

    tracker.observe(down(0x1D, false), false, 600);
    tracker.observe(down(0x1D, false), true, 700);
    assert!(tracker.modifiers(held, &Windows::default(), true).ctrl_l);
}

// [spec:kbdgen:req:tsf.keys.altgr+2/test]
#[test]
fn eaten_right_alt_still_counts_as_held() {
    let mut tracker = Tracker::default();
    tracker.observe(down(0x38, true), true, 1);
    let m = tracker.modifiers(Held::default(), &Windows::default(), true);
    assert!(m.alt_r && m.altgr);
    tracker.observe(down(0x38, true), false, 2);
    assert!(
        !tracker
            .modifiers(Held::default(), &Windows::default(), true)
            .alt_r
    );
}

// [spec:kbdgen:req:tsf.keys.identity+1/test]
// [spec:kbdgen:req:tsf.keys.claim+1/test]
#[test]
fn held_b00_and_caps_set_their_extra() {
    let mut tracker = Tracker::default();
    tracker.observe(down(0x56, false), true, 1);
    tracker.observe(down(0x56, false), true, 1);
    assert_eq!(
        tracker
            .modifiers(Held::default(), &b00_bound(), false)
            .extra,
        [true, false, false]
    );
    tracker.observe(down(0x56, false), false, 2);
    assert_eq!(
        tracker
            .modifiers(Held::default(), &b00_bound(), false)
            .extra,
        [false; 3]
    );
    let caps = Windows {
        extra_modifiers: vec![
            kbd_model::ExtraModifierKey::RightCtrl,
            kbd_model::ExtraModifierKey::CapsLock,
        ],
        ..Windows::default()
    };
    let held = Held {
        caps_down: true,
        ctrl_r: true,
        ..Held::default()
    };
    let m = tracker.modifiers(held, &caps, false);
    assert_eq!(m.extra, [false, true, false]);
    assert!(m.ctrl_r, "the engine rebinds Right Ctrl itself");
}

// [spec:kbdgen:req:tsf.keys.locale-flags/test]
#[test]
fn shift_lock_follows_caps_and_shift() {
    let windows = Windows {
        shift_lock: true,
        ..Windows::default()
    };
    let toggled = Held {
        caps_on: true,
        ..Held::default()
    };
    let mut tracker = Tracker::default();
    assert!(!tracker.modifiers(toggled, &windows, false).caps);
    tracker.observe(down(0x3A, false), true, 1);
    tracker.observe(down(0x3A, false), false, 2);
    assert!(tracker.modifiers(Held::default(), &windows, false).caps);
    tracker.observe(down(0x3A, false), true, 3);
    assert!(tracker.modifiers(Held::default(), &windows, false).caps);
    tracker.observe(down(0x36, false), true, 4);
    assert!(!tracker.modifiers(toggled, &windows, false).caps);
    assert!(tracker.modifiers(toggled, &Windows::default(), false).caps);
}

// [spec:kbdgen:req:tsf.keys.locale-flags/test]
#[test]
fn shift_backspace_gives_engine_mark() {
    let mut keyboard = fixture::keyboard();
    keyboard.windows.lrm_rlm = true;
    let model = Model::from_keyboard(keyboard, Options::default()).unwrap();
    let held = Held {
        shift_r: true,
        ..Held::default()
    };
    let m = Tracker::default().modifiers(held, &Windows::default(), false);
    let event = KeyEvent::with(Key::Backspace, m);
    let (action, _) = model.key(&State::default(), &Context::default(), &event);
    assert!(matches!(action, Action::Edit { insert, .. } if insert == "\u{200F}"));
    let plain = KeyEvent::with(Key::Backspace, ModifierState::default());
    let (action, _) = model.key(&State::default(), &Context::default(), &plain);
    assert_eq!(action, Action::Pass);
}

// [spec:kbdgen:req:tsf.keys.identity+1/test]
// [spec:kbdgen:req:tsf.keys.recover/test]
#[test]
fn missing_scan_code_restored_from_dummy_mapping() {
    let wpf = Stroke {
        vk: 0xBB,
        lparam: 1,
    };
    assert_eq!(wpf.restored(0x0D).scan(), 0x0D);
    assert!(!wpf.restored(0x0D).extended());
    let end = Stroke {
        vk: 0x23,
        lparam: 1,
    }
    .restored(0xE04F);
    assert_eq!((end.scan(), end.extended()), (0x4F, true));
    let full = down(0x10, false);
    assert_eq!(full.restored(0x2C), full);
    assert_eq!(full.restored(0x10), full);
    let delete = Stroke {
        vk: 0x2E,
        ..down(0x53, false)
    };
    assert!(delete.restored(0xE053).extended());
    assert_eq!(
        classify(down(0x53, false).restored(0x53), false, false),
        Role::Engine(Key::Decimal)
    );
}

// [spec:kbdgen:req:tsf.keys.altgr+2/test]
// [spec:kbdgen:req:tsf.keys.preserved+1/test]
#[test]
fn altgr_chords_cover_non_passing_positions() {
    let model = fixture::model();
    let chords = AltGrChords::of(&model).preserved;
    let plain: Vec<u8> = chords.iter().filter(|c| !c.shift).map(|c| c.scan).collect();
    assert_eq!(plain, [0x0D, 0x10, 0x14, 0x1E, 0x2B, 0x56, 0x2C, 0x30]);
    let shifted: Vec<u8> = chords.iter().filter(|c| c.shift).map(|c| c.scan).collect();
    assert_eq!(
        shifted,
        [0x56],
        "only the B00 modifier key without an AltGr+Shift layer"
    );
    assert!(chords.iter().all(|c| {
        [false, true]
            .iter()
            .all(|&ctrl| crate::keys::Chord::from_guid(c.guid(ctrl)) == Some(*c))
    }));
    assert_eq!(
        crate::keys::Chord::from_guid(crate::keys::ALTGR_CHORDS - 1),
        None
    );
    assert_eq!(
        crate::keys::Chord::from_guid(crate::keys::ALTGR_CHORDS + 0x400),
        None
    );
}

// [spec:kbdgen:req:tsf.keys.altgr+2/test]
// [spec:kbdgen:req:tsf.keys.ctrl-alt/test]
#[test]
fn typed_chords_give_text_or_dead_keys() {
    let chords = AltGrChords::of(&fixture::model());
    let typed: Vec<(u8, bool)> = chords.typed.iter().map(|c| (c.scan, c.shift)).collect();
    assert_eq!(typed, [(fixture::ACUTE, false), (fixture::T, false)]);
    assert!(chords.typed.iter().all(|c| chords.preserved.contains(c)));
}

/// What `ctrl_alt` makes of `key` under `m` for the fixture.
fn through_ctrl_alt(key: &Key, m: ModifierState, windows: &Windows) -> Option<ModifierState> {
    let model = fixture::model();
    let chords = AltGrChords::of(&model);
    ctrl_alt(key, m, windows, &chords.typed)
}

/// The output of `scan` under `m` from the start, for the fixture.
fn output(scan: u8, m: ModifierState) -> (Action, bool) {
    let model = fixture::model();
    let event = KeyEvent::with(Key::Scan(scan), m);
    let (action, state) = model.key(&State::default(), &Context::default(), &event);
    (action, state != State::default())
}

// [spec:kbdgen:req:tsf.keys.ctrl-alt/test]
#[test]
fn ctrl_with_either_alt_types_as_altgr() {
    let sides = [
        (true, false, true, false),
        (false, true, true, false),
        (true, false, false, true),
        (false, true, false, true),
    ];
    for (ctrl_l, ctrl_r, alt_l, alt_r) in sides {
        let m = ModifierState {
            ctrl_l,
            ctrl_r,
            alt_l,
            alt_r,
            caps: true,
            ..ModifierState::default()
        };
        let altgr = ModifierState {
            caps: true,
            ..ModifierState::altgr()
        };
        for scan in [fixture::T, fixture::ACUTE] {
            let sent = through_ctrl_alt(&Key::Scan(scan), m, &b00_bound());
            assert_eq!(sent, Some(altgr), "{m:?} {scan:02x}");
        }
    }
    let (action, _) = output(fixture::T, ModifierState::altgr());
    assert!(matches!(action, Action::Edit { insert, .. } if insert == "t\u{301}"));
    assert!(output(fixture::ACUTE, ModifierState::altgr()).1);
}

// [spec:kbdgen:req:tsf.keys.ctrl-alt/test]
#[test]
fn ctrl_alt_without_altgr_output_passes() {
    let ctrl_alt_r = ModifierState {
        ctrl_l: true,
        alt_r: true,
        altgr: true,
        ..ModifierState::default()
    };
    // The gap, a position with no key, and the empty key, which only the
    // `ctrl alt` layer fills and the layout DLL's AltGr column leaves
    // empty.
    for scan in [fixture::Q, fixture::A, fixture::APOSTROPHE] {
        assert_eq!(
            through_ctrl_alt(&Key::Scan(scan), ctrl_alt_r, &b00_bound()),
            None,
            "{scan:02x}"
        );
    }
    let (action, _) = output(fixture::A, ctrl_alt_r);
    assert!(matches!(action, Action::Edit { insert, .. } if insert == "ä"));
    let shifted = ModifierState {
        shift_r: true,
        ..ctrl_alt_r
    };
    assert_eq!(
        through_ctrl_alt(&Key::Scan(fixture::T), shifted, &b00_bound()),
        None
    );
}

// [spec:kbdgen:req:tsf.keys.ctrl-alt/test]
#[test]
fn other_keys_keep_held_modifiers() {
    let ctrl_alt_l = ModifierState {
        ctrl_l: true,
        alt_l: true,
        ..ModifierState::default()
    };
    let windows = b00_bound();
    for key in [
        Key::Backspace,
        Key::Decimal,
        Key::Scan(fixture::SPACE),
        Key::Scan(fixture::B00),
    ] {
        assert_eq!(
            through_ctrl_alt(&key, ctrl_alt_l, &windows),
            Some(ctrl_alt_l)
        );
    }
    let t = Key::Scan(fixture::T);
    for m in [
        ModifierState::altgr(),
        ModifierState {
            ctrl_l: true,
            ..ModifierState::default()
        },
        ModifierState {
            alt_l: true,
            ..ModifierState::default()
        },
    ] {
        assert_eq!(through_ctrl_alt(&t, m, &windows), Some(m));
    }
    let right_ctrl = Windows {
        extra_modifiers: vec![kbd_model::ExtraModifierKey::RightCtrl],
        ..Windows::default()
    };
    let extra_alt = ModifierState {
        ctrl_r: true,
        alt_l: true,
        ..ModifierState::default()
    };
    assert_eq!(
        through_ctrl_alt(&t, extra_alt, &right_ctrl),
        Some(extra_alt)
    );
    let both = ModifierState {
        ctrl_l: true,
        ..extra_alt
    };
    let sent = through_ctrl_alt(&t, both, &right_ctrl).unwrap();
    assert!(sent.ctrl_r && !sent.ctrl_l && !sent.alt_l && sent.alt_r && sent.altgr);
}
