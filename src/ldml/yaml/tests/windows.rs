//! Windows extensions of v4: native-only layers, `extraModifiers` and
//! `targets`.

use kbd_model::{Component, ExtraModifierKey, ModifierSet, Modifiers};

use super::*;

fn with_extra(extra: &str, layers: &[(&str, &str)]) -> String {
    sme(&hardware(layers).replace(
        "  default:\n",
        &format!("  default:\n    extraModifiers: {extra}\n"),
    ))
}

const NO_B00: &str = "` 1 2 3 4 5 6 7 8 9 0 - =\nq w e r t y u i o p [ ]\na s d f g h j k l ; ' #\n\\u{0} z x c v b n m , . /\n";

// [spec:kbdgen:def:ldml.yaml.native/test]
#[test]
fn extra_modifiers_bind_extra_keys_in_order() {
    let yaml = with_extra(
        "[capsLock, rightCtrl]",
        &[
            ("none", QWERTY),
            ("extra1", SHIFTED),
            ("extra2 shift", QWERTY),
        ],
    );
    let kb = keyboard("sme", &yaml, Host::Windows);
    assert_eq!(
        kb.windows.extra_modifiers,
        [ExtraModifierKey::CapsLock, ExtraModifierKey::RightCtrl]
    );
    let xml = xml("sme", &yaml, Host::Windows);
    assert!(
        xml.contains(r#"<kbdgen:extraModifier key="capsLock" />"#),
        "{xml}"
    );
    let err = error(&with_extra("[rightCtrl, rightCtrl]", &[("none", QWERTY)]));
    assert!(
        err.ends_with("hardware.default.extraModifiers[2]: an extra modifier key occurs twice"),
        "{err}"
    );
    let err = error(&with_extra("[shiftLock]", &[("none", QWERTY)]));
    assert!(
        err.ends_with("shiftLock is not rightCtrl, capsLock or B00"),
        "{err}"
    );
}

// [spec:kbdgen:def:ldml.yaml.native/test]
#[test]
fn extra_layers_need_their_binding() {
    let err = error(&with_extra(
        "[rightCtrl]",
        &[("none", QWERTY), ("extra2", SHIFTED)],
    ));
    assert!(
        err.ends_with(
            "sme.yaml: hardware.default.layers.extra2: layer extra2 uses extra2, but extraModifiers binds 1 key(s), so nothing binds extra2"
        ),
        "{err}"
    );
    let err = error(&sme(&hardware(&[
        ("none", QWERTY),
        ("extra1 shift", SHIFTED),
    ])));
    assert!(
        err.contains("layers.extra1 shift: layer extra1 shift uses extra1"),
        "{err}"
    );
    let inherited = format!(
        "{}  windows:\n    inherits: default\n    extraModifiers: [rightCtrl]\n    layers:\n      extra1: |\n{}",
        hardware(&[("none", QWERTY)]),
        indent(SHIFTED, 8)
    );
    let kb = keyboard("sme", &sme(&inherited), Host::Windows);
    assert_eq!(kb.windows.extra_modifiers, [ExtraModifierKey::RightCtrl]);
}

// [spec:kbdgen:def:ldml.yaml.native/test]
#[test]
fn bound_b00_positions_must_be_empty() {
    let err = lower_error(&with_extra("[B00]", &[("none", QWERTY)]));
    assert!(
        err.ends_with(
            "sme.yaml: hardware.default.layers.none, row 4, token 1: extraModifiers binds B00, so every B00 position must be \\u{0}"
        ),
        "{err}"
    );
    let err = lower_error(&with_extra("[B00]", &[("none", NO_B00), ("ctrl", QWERTY)]));
    assert!(err.contains("layers.ctrl, row 4, token 1"), "{err}");
    let kb = keyboard(
        "sme",
        &with_extra("[B00]", &[("none", NO_B00), ("extra1", NO_B00)]),
        Host::Windows,
    );
    assert_eq!(kb.windows.extra_modifiers, [ExtraModifierKey::B00]);
    for layer in &kb.hardware.as_ref().unwrap().layers {
        assert_eq!(kb.key(layer.rows[3][0]).unwrap().id, "gap");
    }
    keyboard("sme", &sme(&hardware(&[("none", QWERTY)])), Host::Windows);
}

// [spec:kbdgen:def:ldml.yaml.native/test]
#[test]
fn native_layers_take_ordinary_tokens() {
    let cmd = QWERTY.replace("` 1", "\\d{´} 1");
    let yaml = sme(&format!(
        "deadKeys: {{´: {{compose: {{a: á}}}}}}\n{}",
        hardware(&[
            ("none", QWERTY),
            ("cmd", &cmd),
            ("cmd alt", SHIFTED),
            ("cmd altR shift", QWERTY),
            ("ctrl", SHIFTED),
            ("ctrlL", QWERTY),
        ])
    ));
    let kb = keyboard("sme", &yaml, Host::Windows);
    let hw = kb.hardware.as_ref().unwrap();
    let cmd_layer = hw
        .layers
        .iter()
        .find(|l| l.modifiers == [ModifierSet::Set(Modifiers::of(&[Component::Cmd]))])
        .unwrap();
    assert!(cmd_layer.modifiers.iter().all(|s| s.is_native()));
    let dead = kb.key(cmd_layer.rows[0][0]).unwrap();
    assert_eq!(dead.id, "dk-dk_00B4");
    let xml = xml("sme", &yaml, Host::Windows);
    assert!(xml.contains(r#"<kbdgen:layer modifiers="cmd">"#), "{xml}");
    assert!(!xml.contains(r#"<layer modifiers="cmd">"#), "{xml}");
    let err = error(&sme(&hardware(&[
        ("ctrl alt", QWERTY),
        ("altR ctrl", SHIFTED),
    ])));
    assert!(err.contains("overlaps"), "{err}");
}

// [spec:kbdgen:def:ldml.yaml.targets/test]
#[test]
fn targets_reach_the_kbdgen_namespace() {
    let yaml = sme(&format!(
        "targets:\n  windows: {{locale: se-Latn-NO, id: sme-x, shiftLock: true, lrmRlm: true, keyNames: {{Right Alt: AltGr}}}}\n  chromeOS: {{locale: se, xkbLayout: no}}\n  android: {{spellerPackageKey: k, spellerPath: s.zhfst}}\n{}",
        hardware(&[("none", QWERTY)])
    ));
    let xml = xml("sme", &yaml, Host::Windows);
    for needle in [
        r#"<kbdgen:windows shiftLock="true" lrmRlm="true" />"#,
        r#"<kbdgen:windowsKeyName key="Right Alt" name="AltGr" />"#,
        r#"<kbdgen:target host="windows" name="id" value="sme-x" />"#,
        r#"<kbdgen:target host="chromeOS" name="xkbLayout" value="no" />"#,
    ] {
        assert!(xml.contains(needle), "{needle}: {xml}");
    }
    let resolved = kbd_ldml::resolve(&document("sme", &yaml, Host::Windows)).unwrap();
    let kb = &resolved.keyboard;
    assert!(kb.windows.shift_lock && kb.windows.lrm_rlm);
    assert_eq!(kb.windows.key_names["Right Alt"], "AltGr");
    assert_eq!(resolved.extensions.targets.len(), 6);
    let err = error(&sme("targets: {windows: {keyNames: {Hyper: H}}}\n"));
    assert!(
        err.ends_with("targets.windows.keyNames.Hyper: Hyper is not a Windows key name"),
        "{err}"
    );
    let err = error(&sme("targets: {chromeOS: {id: x}}\n"));
    assert!(
        err.contains("targets.chromeOS.id: unknown field id"),
        "{err}"
    );
}
