//! The engine on lowered layouts: the Võro example and the macOS rules
//! that lowering realises (`ldml.scope.macos-rules`).

use kbd_engine::OutputForm;
use kbd_model::Direction;

use super::*;

const ACUTE: u8 = 0x0D;
const CARON: u8 = 0x29;
const A: u8 = 0x1E;
const C: u8 = 0x2E;
const J: u8 = 0x24;
const O: u8 = 0x18;
const SPACE: u8 = 0x39;

fn vro(host: Host) -> Typist {
    Typist::new(keyboard("vro", VRO, host))
}

fn caps_shift() -> ModifierState {
    ModifierState {
        caps: true,
        ..ModifierState::shift()
    }
}

// [spec:kbdgen:sem:ldml.yaml.dead-keys.compose/test]
// [spec:kbdgen:sem:ldml.scope.macos-rules/test]
#[test]
fn acute_then_a_gives_a_acute() {
    let mut t = vro(Host::MacOs);
    t.scan(ACUTE);
    assert_eq!((t.text.as_str(), t.preedit.as_str()), ("", "´"));
    t.scan(A);
    assert_eq!((t.text.as_str(), t.preedit.as_str()), ("á", ""));
    t.scan(ACUTE).scan(SPACE);
    assert_eq!(t.text, "á´");
}

// [spec:kbdgen:sem:ldml.yaml.dead-keys.fallback/test]
// [spec:kbdgen:sem:ldml.scope.macos-rules/test]
#[test]
fn unmatched_dead_key_emits_standalone_then_key() {
    let mut t = vro(Host::MacOs);
    t.scan(ACUTE).scan(J);
    assert_eq!((t.text.as_str(), t.preedit.as_str()), ("´j", ""));
    let mut t = vro(Host::MacOs);
    t.scan(ACUTE).scan(CARON);
    assert_eq!((t.text.as_str(), t.preedit.as_str()), ("´", "ˇ"));
    t.scan(C);
    assert_eq!(t.text, "´č");
}

// [spec:kbdgen:sem:ldml.yaml.dead-keys.backspace/test]
// [spec:kbdgen:sem:ldml.scope.macos-rules/test]
#[test]
fn backspace_cancels_only_the_pending_dead_key() {
    let mut t = vro(Host::MacOs);
    t.scan(J).scan(ACUTE).press(KeyEvent::new(Key::Backspace));
    assert_eq!((t.text.as_str(), t.preedit.as_str()), ("j", ""));
    t.scan(A);
    assert_eq!(t.text, "ja");
}

// [spec:kbdgen:sem:ldml.yaml.implied-layers/test]
// [spec:kbdgen:sem:ldml.scope.macos-rules/test]
#[test]
fn caps_shift_letter_gives_uppercase() {
    let mut t = vro(Host::MacOs);
    t.scan_with(A, caps_shift());
    assert_eq!(t.text, "A");
    t.scan_with(
        A,
        ModifierState {
            caps: true,
            ..ModifierState::default()
        },
    );
    assert_eq!(t.text, "AA");
    t.scan_with(
        ACUTE,
        ModifierState {
            caps: true,
            ..ModifierState::default()
        },
    );
    assert_eq!(t.preedit, "ę");
    t.scan(A);
    assert_eq!(t.text, "AAá");
    let alt_caps = ModifierState {
        alt_l: true,
        caps: true,
        ..ModifierState::default()
    };
    t.scan_with(O, alt_caps);
    assert_eq!(t.text, "AAáØ");
}

// [spec:kbdgen:def:ldml.yaml.long-press/test]
#[test]
fn long_press_offers_the_candidates() {
    let mut t = vro(Host::MacOs);
    t.id("u-0061", Gesture::LongPress(1));
    assert_eq!(t.text, "á");
    let mut t = vro(Host::Ios);
    t.touch(0, 0, 1, 0, Gesture::LongPress(2));
    assert_eq!(t.text, "ä");
    t.touch(0, 0, 0, 8, Gesture::LongPress(5));
    assert_eq!(t.text, "äõ\u{32D}");
}

// [spec:kbdgen:sem:ldml.yaml.touch.roles/test]
#[test]
fn role_keys_pass_and_layer_keys_switch() {
    let mut t = vro(Host::Ios);
    let shift = KeyEvent::new(Key::Touch {
        set: 0,
        layer: 0,
        row: 2,
        col: 0,
        gesture: Gesture::Tap,
    });
    let (action, _) = t.model.key(&t.state, &Context::new(""), &shift);
    assert_eq!(action, Action::Pass);
    t.touch(0, 1, 0, 0, Gesture::Tap);
    assert_eq!(t.text, "Q");
    let yaml = sme(
        "touch: {default: {sizes: {phone: {layers: {base: 'a \\l{shift}', shift: 'A \\l{base}'}}}}}\n",
    );
    let mut t = Typist::new(keyboard("sme", &yaml, Host::Web));
    t.touch(0, 0, 0, 1, Gesture::Tap);
    assert_eq!(t.layer.as_deref(), Some("shift"));
    t.touch(0, 1, 0, 0, Gesture::Tap)
        .touch(0, 1, 0, 1, Gesture::Tap);
    assert_eq!((t.text.as_str(), t.layer.as_deref()), ("A", Some("base")));
}

// [spec:kbdgen:sem:ldml.yaml.touch.flicks/test]
#[test]
fn tablet_flick_down_gives_the_digit() {
    let mut t = vro(Host::Ios);
    assert_eq!(t.model.touch_set_by_name("tablet"), Some(1));
    t.touch(1, 0, 0, 0, Gesture::Flick(vec![Direction::S]));
    t.touch(1, 0, 2, 3, Gesture::Flick(vec![Direction::S]));
    assert_eq!(t.text, "1-");
}

// [spec:kbdgen:def:ldml.yaml.dead-keys/test]
#[test]
fn chained_dead_keys_compose_through_nested_nodes() {
    let rows = QWERTY.replace("[ ]", "\\d{´} \\d{¨}");
    let yaml = sme(&format!(
        "deadKeys:\n  ´: {{compose: {{a: á, ¨: {{compose: {{u: ǘ}}}}}}}}\n  ¨: {{compose: {{u: ü}}}}\n{}",
        hardware(&[("none", &rows)])
    ));
    let (left, right) = (0x1A, 0x1B);
    let mut t = Typist::new(keyboard("sme", &yaml, Host::Web));
    t.scan(left).scan(right);
    assert_eq!(t.preedit, "´¨");
    t.scan(0x16);
    assert_eq!(t.text, "ǘ");
    t.scan(left).scan(right).scan(SPACE);
    assert_eq!(t.text, "ǘ´¨");
    t.scan(right).scan(0x16);
    assert_eq!(t.text, "ǘ´¨ü");
}

// [spec:kbdgen:sem:ldml.yaml.dead-keys.fallback/test]
#[test]
fn empty_standalone_drops_the_dead_key() {
    let rows = QWERTY.replace("[ ]", "\\d{´} ]");
    let yaml = sme(&format!(
        "deadKeys: {{´: {{standalone: '', compose: {{a: á}}}}}}\n{}",
        hardware(&[("none", &rows)])
    ));
    let mut t = Typist::new(keyboard("sme", &yaml, Host::Web));
    t.scan(0x1A).scan(J).scan(0x1A).scan(A);
    assert_eq!(t.text, "já");
}

// [spec:kbdgen:sem:ldml.yaml.normalization/test]
// [spec:kbdgen:sem:ldml.scope.macos-rules/test]
#[test]
fn disabled_normalization_emits_authored_scalars() {
    let rows = QWERTY.replace("[ ]", "\\d{´} ]");
    let body = format!(
        "deadKeys: {{´: {{compose: {{a: á}}}}}}\n{}",
        hardware(&[("none", &rows)])
    );
    let nfd = |yaml: &str| {
        let model = Model::from_keyboard(
            keyboard("sme", yaml, Host::Web),
            Options {
                output_form: OutputForm::Nfd,
                ..Options::default()
            },
        )
        .unwrap();
        let mut t = Typist {
            model,
            state: State::default(),
            text: String::new(),
            preedit: String::new(),
            layer: None,
        };
        t.scan(0x1A).scan(A);
        t.text
    };
    assert_eq!(nfd(&sme(&body)), "á");
    assert_eq!(
        nfd(&sme(&format!("normalization: enabled\n{body}"))),
        "a\u{301}"
    );
}

// [spec:kbdgen:sem:ldml.yaml.dead-keys.keys/test]
#[test]
fn pending_dead_key_commits_its_standalone() {
    let mut t = vro(Host::Windows);
    t.scan(ACUTE).press(KeyEvent::new(Key::Commit));
    assert_eq!((t.text.as_str(), t.preedit.as_str()), ("´", ""));
    t.scan_with(CARON, ModifierState::shift());
    assert_eq!(t.preedit, "~");
    t.scan(O);
    assert_eq!(t.text, "´õ");
}
