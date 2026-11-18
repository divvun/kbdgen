use kbd_engine::{Key, ModifierState};

use super::field::Field;
use super::fixture::{features, model, mods, vro};
use crate::keys::{
    CONTROL_MASK, Facts, Keys, LOCK_MASK, MOD1_MASK, MOD4_MASK, MOD5_MASK, RELEASE_MASK, Role,
    SHIFT_MASK, SUPER_MASK, classify,
};
use crate::keysym::{self, by_name};
use crate::planner::{Reply, Session, Signal};

fn keysym(name: &str) -> u32 {
    by_name(name).unwrap_or_else(|| panic!("no keysym {name}"))
}

fn scan(scan: u8) -> Role {
    Role::Engine(Key::Scan(scan))
}

// [spec:kbdgen:req:linux.ibus.keys/test]
#[test]
fn hardware_keys_go_by_code_not_keyval() {
    assert_eq!(classify(keysym("a"), 30), scan(0x1E));
    assert_eq!(classify(keysym("q"), 30), scan(0x1E), "a keymap's keyval");
    assert_eq!(classify(0, 41), scan(0x29));
    assert_eq!(classify(keysym("Return"), 28), scan(0x1C));
    assert_eq!(classify(keysym("Escape"), 1), scan(0x01));
    assert_eq!(classify(keysym("F12"), 88), scan(0x58));
    assert_eq!(classify(keysym("less"), 86), scan(0x56), "B00");
    assert_eq!(classify(keysym("backslash"), 89), scan(0x73), "JIS Ro");
    assert_eq!(classify(keysym("yen"), 124), scan(0x7D), "JIS Yen");
    assert_eq!(
        classify(keysym("BackSpace"), 14),
        Role::Engine(Key::Backspace)
    );
    assert_eq!(
        classify(keysym("KP_Decimal"), 83),
        Role::Engine(Key::Decimal)
    );
    assert_eq!(
        classify(keysym("KP_Separator"), 83),
        Role::Engine(Key::Decimal)
    );
    assert_eq!(
        classify(keysym("KP_Delete"), 83),
        Role::Frame,
        "Num Lock off"
    );
    for (keyval, code) in [
        ("Left", 105),
        ("Up", 103),
        ("Delete", 111),
        ("KP_Enter", 96),
        ("Home", 102),
        ("F13", 183),
        ("Menu", 127),
    ] {
        assert_eq!(classify(keysym(keyval), code), Role::Frame, "{keyval}");
    }
}

// [spec:kbdgen:req:linux.ibus.keys/test]
#[test]
fn modifiers_by_keyval_or_code() {
    for code in [29, 42, 54, 56, 58, 69, 97, 100, 125, 126] {
        assert_eq!(classify(keysym("a"), code), Role::Modifier, "{code}");
    }
    for name in [
        "Shift_L",
        "Shift_R",
        "Control_L",
        "Control_R",
        "Alt_L",
        "Alt_R",
        "Meta_L",
        "Meta_R",
        "Super_L",
        "Super_R",
        "Hyper_L",
        "Hyper_R",
        "ISO_Level3_Shift",
        "ISO_Level5_Shift",
        "Mode_switch",
        "ISO_Group_Shift",
        "ISO_Next_Group",
        "ISO_Prev_Group_Lock",
        "ISO_First_Group",
        "ISO_Last_Group_Lock",
        "ISO_Group_Latch",
        "Caps_Lock",
        "Shift_Lock",
        "Num_Lock",
    ] {
        assert!(keysym::is_modifier(keysym(name)), "{name}");
        assert_eq!(classify(keysym(name), 84), Role::Modifier, "{name}");
    }
    assert!(!keysym::is_modifier(keysym("Multi_key")));
    assert!(!keysym::is_modifier(keysym("dead_acute")));
}

// [spec:kbdgen:sem:linux.ibus.virtual-keys/test]
#[test]
fn onscreen_text_is_emitted_scalar_by_scalar() {
    let emit = |s: &str| Role::Engine(Key::Emit(s.to_owned()));
    assert_eq!(classify(keysym("a"), 0), emit("a"));
    assert_eq!(classify(keysym("eacute"), 0), emit("é"));
    assert_eq!(classify(keysym("eng"), 0), emit("ŋ"), "a legacy keysym");
    assert_eq!(classify(keysym("ccaron"), 0), emit("č"));
    assert_eq!(
        classify(0x0100_0301, 0),
        emit("\u{301}"),
        "a Unicode keysym"
    );
    assert_eq!(classify(0x0101_D56B, 0), emit("𝕫"));
    assert_eq!(
        classify(keysym("BackSpace"), 0),
        Role::Engine(Key::Backspace)
    );
    assert_eq!(
        classify(keysym("Return"), 0),
        Role::Frame,
        "a control character"
    );
    assert_eq!(classify(keysym("Tab"), 0), Role::Frame);
    assert_eq!(classify(keysym("dead_acute"), 0), Role::Frame);
    assert_eq!(classify(keysym("Left"), 0), Role::Frame);
    assert_eq!(classify(0x0100_D800, 0), Role::Frame, "a surrogate");
}

// [spec:kbdgen:sem:linux.ibus.virtual-keys/test]
#[test]
fn every_character_keysym_maps_back() {
    for c in [
        'a', 'é', 'š', 'ŋ', 'ŧ', 'ð', 'ø', 'æ', '€', 'ẞ', 'ǩ', '\u{A0}',
    ] {
        let legacy = (0x20..=0x7e).contains(&u32::from(c)) || (0xa0..=0xff).contains(&u32::from(c));
        let keyval = if legacy {
            u32::from(c)
        } else {
            (0x100..=0xffff_u32)
                .find(|k| keysym::to_char(*k) == Some(c))
                .unwrap_or(0x0100_0000 + u32::from(c))
        };
        assert_eq!(keysym::to_char(keyval), Some(c), "{c}");
    }
}

fn shift_r(keys: &mut Keys) {
    keys.observe(54, true);
}

// [spec:kbdgen:sem:linux.ibus.modifiers/test]
#[test]
fn state_bits_and_held_keys_give_modifiers() {
    let facts = Facts::of(&model("vro", &vro()));
    assert!(!facts.uses_level3, "vro has no AltGr layer on Linux");
    let mut keys = Keys::default();
    let none = ModifierState::default();
    assert_eq!(keys.modifiers(0, &facts), none);
    assert_eq!(
        keys.modifiers(SHIFT_MASK, &facts),
        ModifierState {
            shift_l: true,
            ..none
        },
        "Left when no Shift was seen"
    );
    shift_r(&mut keys);
    assert_eq!(
        keys.modifiers(SHIFT_MASK, &facts),
        ModifierState {
            shift_r: true,
            ..none
        }
    );
    keys.observe(42, true);
    let both = keys.modifiers(SHIFT_MASK, &facts);
    assert!(both.shift_l && both.shift_r);
    keys.forget();
    assert_eq!(
        keys.modifiers(CONTROL_MASK, &facts),
        ModifierState {
            ctrl_l: true,
            ..none
        }
    );
    keys.observe(97, true);
    assert_eq!(
        keys.modifiers(CONTROL_MASK, &facts),
        ModifierState {
            ctrl_r: true,
            ..none
        }
    );
    keys.forget();
    for cmd in [MOD4_MASK, SUPER_MASK] {
        assert_eq!(
            keys.modifiers(cmd, &facts),
            ModifierState { cmd: true, ..none }
        );
    }
    assert_eq!(
        keys.modifiers(LOCK_MASK, &facts),
        ModifierState { caps: true, ..none }
    );
}

// [spec:kbdgen:sem:linux.ibus.modifiers/test]
#[test]
fn right_alt_is_altgr_left_alt_is_alt() {
    let level3 = Facts::of(&model("und-x-mods", &mods()));
    assert!(level3.uses_level3);
    let plain = Facts::of(&model("vro", &vro()));
    let none = ModifierState::default();
    let altgr = ModifierState::altgr();
    let alt_l = ModifierState {
        alt_l: true,
        ..none
    };
    let mut keys = Keys::default();
    assert_eq!(keys.modifiers(MOD5_MASK, &plain), altgr, "ISO_Level3_Shift");
    assert_eq!(
        keys.modifiers(MOD1_MASK, &level3),
        alt_l,
        "no Right Alt held"
    );
    keys.observe(56, true);
    assert_eq!(keys.modifiers(MOD1_MASK, &level3), alt_l);
    keys.forget();
    keys.observe(100, true);
    assert_eq!(
        keys.modifiers(MOD1_MASK, &level3),
        altgr,
        "Right Alt as Alt beneath, AltGr for a Level3 model"
    );
    assert_eq!(keys.modifiers(0, &level3), altgr);
    assert_eq!(
        keys.modifiers(MOD1_MASK, &plain),
        ModifierState {
            alt_r: true,
            ..none
        },
        "Right Alt stays Alt without Level3"
    );
    keys.observe(100, false);
    assert_eq!(keys.modifiers(MOD1_MASK, &level3), alt_l);
}

// [spec:kbdgen:sem:linux.ibus.modifiers/test]
#[test]
fn bound_keys_held_give_extra_modifiers() {
    let facts = Facts::of(&model("und-x-mods", &mods()));
    let mut keys = Keys::default();
    keys.observe(58, true);
    assert_eq!(
        keys.modifiers(LOCK_MASK, &facts).extra,
        [false, true, false]
    );
    keys.observe(86, true);
    assert_eq!(keys.modifiers(0, &facts).extra, [false, true, true]);
    keys.observe(97, true);
    let m = keys.modifiers(CONTROL_MASK, &facts);
    assert!(
        m.ctrl_r && m.extra == [false, true, true],
        "Right Ctrl stays ctrl_r"
    );
    keys.observe(58, false);
    keys.observe(86, false);
    assert_eq!(keys.modifiers(0, &facts).extra, [false; 3]);
}

// [spec:kbdgen:sem:linux.ibus.modifiers/test]
#[test]
fn shift_lock_set_by_caps_cleared_by_shift() {
    let facts = Facts::of(&model("sme", &features()));
    assert!(facts.windows.shift_lock);
    let mut keys = Keys::default();
    assert!(
        !keys.modifiers(LOCK_MASK, &facts).caps,
        "the host's own lock"
    );
    keys.observe(58, true);
    keys.observe(58, false);
    assert!(keys.modifiers(0, &facts).caps);
    keys.observe(42, true);
    keys.observe(42, false);
    assert!(!keys.modifiers(LOCK_MASK, &facts).caps);
}

/// Presses and releases a key, the release with its press's state.
fn tap(session: &mut Session, field: &mut Field, keyval: u32, code: u32, state: u32) -> bool {
    let Reply { handled, signals } = session.process_key_event(keyval, code, state);
    field.apply(&signals);
    let release = session.process_key_event(keyval, code, state | RELEASE_MASK);
    assert_eq!(
        release.handled, handled,
        "a release returns what its press did"
    );
    assert!(release.signals.is_empty());
    handled
}

// [spec:kbdgen:req:linux.ibus.planner/test]
// [spec:kbdgen:req:linux.ibus.keys/test]
#[test]
fn dead_key_composes_with_next_key() {
    let mut session = Session::new(model("vro", &vro()));
    let mut field = Field::default();
    assert!(tap(&mut session, &mut field, keysym("plus"), 13, 0), "´");
    assert_eq!(field.preedit, "´");
    assert!(tap(&mut session, &mut field, keysym("a"), 30, 0));
    assert_eq!(field.shown(), "á");
    assert!(tap(&mut session, &mut field, keysym("plus"), 13, 0));
    assert!(
        tap(&mut session, &mut field, keysym("x"), 45, 0),
        "unmatched"
    );
    assert_eq!(field.shown(), "á´x");
}

// [spec:kbdgen:req:linux.ibus.planner/test]
// [spec:kbdgen:sem:linux.ibus.modifiers/test]
#[test]
fn left_alt_chord_passes_and_commits() {
    let mut session = Session::new(model("vro", &vro()));
    let mut field = Field::default();
    assert!(tap(&mut session, &mut field, keysym("plus"), 13, 0));
    assert!(
        !tap(&mut session, &mut field, keysym("Alt_L"), 56, 0),
        "a modifier"
    );
    assert_eq!(field.preedit, "´", "a modifier resets nothing");
    session.process_key_event(keysym("Alt_L"), 56, 0);
    assert!(!tap(&mut session, &mut field, keysym("f"), 33, MOD1_MASK));
    assert_eq!((field.before.as_str(), field.preedit.as_str()), ("´", ""));
}

// [spec:kbdgen:req:linux.ibus.planner/test]
// [spec:kbdgen:req:linux.ibus.keys/test]
#[test]
fn frame_key_commits_pending_and_passes() {
    let mut session = Session::new(model("vro", &vro()));
    let mut field = Field::default();
    tap(&mut session, &mut field, keysym("plus"), 13, 0);
    assert!(!tap(&mut session, &mut field, keysym("Left"), 105, 0));
    assert_eq!(field.shown(), "´");
    assert_eq!(field.preedit, "");
    assert!(
        !tap(&mut session, &mut field, keysym("Return"), 28, 0),
        "not in the form"
    );
    assert!(tap(&mut session, &mut field, keysym("a"), 30, 0));
    assert_eq!(field.shown(), "´a");
}

// [spec:kbdgen:req:linux.ibus.keys/test]
#[test]
fn unseen_release_and_forwarded_events_pass() {
    let mut session = Session::new(model("vro", &vro()));
    assert!(
        !session
            .process_key_event(keysym("a"), 30, RELEASE_MASK)
            .handled
    );
    let forwarded = session.process_key_event(keysym("a"), 30, 1 << 25);
    assert_eq!(forwarded, Reply::default());
    assert!(session.process_key_event(keysym("a"), 30, 0).handled);
    session.focus_out();
    assert!(
        !session
            .process_key_event(keysym("a"), 30, RELEASE_MASK)
            .handled,
        "focus forgets the press"
    );
}

// [spec:kbdgen:sem:linux.ibus.virtual-keys/test]
#[test]
fn onscreen_text_composes_with_hardware_dead_key() {
    let mut session = Session::new(model("vro", &vro()));
    let mut field = Field::default();
    tap(&mut session, &mut field, keysym("plus"), 13, 0);
    let reply = session.process_key_event(keysym("a"), 0, 0);
    assert!(reply.handled);
    assert_eq!(
        reply.signals.last(),
        Some(&Signal::CommitText("á".to_owned()))
    );
    field.apply(&reply.signals);
    let reply = session.process_key_event(keysym("Return"), 0, 0);
    assert!(!reply.handled, "GNOME Shell presses it itself");
    assert_eq!(field.shown(), "á");
}

// [spec:kbdgen:sem:linux.ibus.modifiers/test]
#[test]
fn altgr_reaches_ctrl_alt_layer() {
    let mut session = Session::new(model("und-x-mods", &mods()));
    let mut field = Field::default();
    session.process_key_event(keysym("ISO_Level3_Shift"), 100, 0);
    assert!(tap(&mut session, &mut field, keysym("a"), 30, MOD5_MASK));
    assert_eq!(field.shown(), "ç");
    session.process_key_event(keysym("ISO_Level3_Shift"), 100, MOD5_MASK | RELEASE_MASK);
    session.process_key_event(keysym("Alt_L"), 56, 0);
    assert!(
        !tap(&mut session, &mut field, keysym("a"), 30, MOD1_MASK),
        "Left Alt"
    );
    session.process_key_event(keysym("Alt_L"), 56, MOD1_MASK | RELEASE_MASK);
    session.process_key_event(keysym("less"), 86, 0);
    assert!(
        tap(&mut session, &mut field, keysym("a"), 30, 0),
        "B00 held"
    );
    assert_eq!(field.shown(), "ç3");
}
