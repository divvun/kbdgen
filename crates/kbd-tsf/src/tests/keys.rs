//! Key identity, AltGr, extra modifiers and Shift Lock.

use kbd_engine::{Action, Context, Key, KeyEvent, Model, ModifierState, Options, State};
use kbd_model::Windows;

use super::fixture;
use crate::keys::{Held, Role, Stroke, Tracker, VK_PACKET, VK_PROCESSKEY, classify};

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
// [spec:kbdgen:req:tsf.keys.altgr+1/test]
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

// [spec:kbdgen:req:tsf.keys.altgr+1/test]
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

// [spec:kbdgen:req:tsf.keys.altgr+1/test]
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

// [spec:kbdgen:req:tsf.keys.altgr+1/test]
// [spec:kbdgen:req:tsf.keys.preserved/test]
#[test]
fn altgr_chords_cover_non_passing_positions() {
    let model = fixture::model();
    let chords = crate::keys::altgr_chords(&model);
    let plain: Vec<u8> = chords.iter().filter(|c| !c.shift).map(|c| c.scan).collect();
    assert_eq!(plain, [0x0D, 0x10, 0x14, 0x1E, 0x2B, 0x56, 0x2C, 0x30]);
    let shifted: Vec<u8> = chords.iter().filter(|c| c.shift).map(|c| c.scan).collect();
    assert_eq!(
        shifted,
        [0x56],
        "only the B00 modifier key without an AltGr+Shift layer"
    );
    assert!(
        chords
            .iter()
            .all(|c| crate::keys::Chord::from_guid(c.guid()) == Some(*c))
    );
    assert_eq!(
        crate::keys::Chord::from_guid(crate::keys::ALTGR_CHORDS - 1),
        None
    );
    assert_eq!(
        crate::keys::Chord::from_guid(crate::keys::ALTGR_CHORDS + 0x200),
        None
    );
}
