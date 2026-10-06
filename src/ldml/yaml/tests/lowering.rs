//! Lowering: key ids, rows, implied layers, touch, dead keys, displays,
//! verbatim fields, hosts and determinism.

use kbd_model::{DisplayTarget, HardwareLayer, Normalization, Role, TransformGroup};

use super::*;
use crate::ldml::layouts::HostDocument;
use crate::ldml::layouts::compiled_layouts;

fn row_ids(kb: &Keyboard, rows: &[Vec<u16>]) -> Vec<String> {
    rows.iter()
        .map(|r| {
            r.iter()
                .map(|k| kb.key(*k).unwrap().id.clone())
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect()
}

fn layer<'a>(kb: &'a Keyboard, sets: &str) -> &'a HardwareLayer {
    let wanted = kbd_ldml::parse_modifiers(sets).unwrap();
    kb.hardware
        .as_ref()
        .unwrap()
        .layers
        .iter()
        .find(|l| wanted.iter().all(|w| l.modifiers.contains(w)))
        .unwrap_or_else(|| panic!("no layer {sets}"))
}

fn sets(kb: &Keyboard) -> Vec<String> {
    kb.hardware
        .as_ref()
        .unwrap()
        .layers
        .iter()
        .map(|l| kbd_ldml::encode_modifiers(&l.modifiers))
        .collect()
}

fn touch_yaml(layers: &str) -> String {
    sme(&format!(
        "touch:\n  default:\n    sizes:\n      phone:\n        layers:\n{}",
        indent(layers, 10)
    ))
}

// [spec:kbdgen:def:ldml.yaml.key-ids+1/test]
#[test]
fn key_ids_follow_the_synthesis_rules() {
    let rows = QWERTY
        .replace("` 1", "\\d{´} 1")
        .replace("[ ]", "õ a\\u{301}")
        .replace("; '", "\\m{x} a\\m{y}")
        .replace("< z", "\\u{0} \\s{gap}");
    let kb = keyboard(
        "sme",
        &sme(&format!(
            "deadKeys: {{´: {{}}}}\n{}",
            hardware(&[("none", &rows)])
        )),
        Host::Web,
    );
    assert_eq!(
        row_ids(&kb, &layer(&kb, "none").rows),
        [
            "dk-dk_00B4 1 2 3 4 5 6 7 8 9 0 u-002D u-003D",
            "q w e r t y u i o p u-00F5 u-0061-0301",
            "a s d f g h j k l o-1 o-2 u-0023",
            "gap gap x c v b n m u-002C u-002E u-002F",
            "space",
        ]
    );
    let touch = touch_yaml(
        "base: '\\s{shift} \\s{backspace:2} \\l{symbols-1} \\s{\"a\":1} \\s{space:3}'\nshift: '\\s{shift}'\nsymbols-1: '\\s{symbols}'\n",
    );
    let kb = keyboard("sme", &touch, Host::Ios);
    let set = &kb.touch[0];
    let rows = |id: &str| row_ids(&kb, &set.layers[set.layer_index(id).unwrap()].rows);
    assert_eq!(
        rows("base"),
        ["shift-shift backspace layer-symbols-1 a space-2"]
    );
    assert_eq!(rows("shift"), ["shift-base"]);
    assert_eq!(rows("symbols-1"), ["symbols-base"]);
}

// [spec:kbdgen:sem:ldml.yaml.key-ids.collisions+1/test]
#[test]
fn differing_definitions_get_numbered_suffixes() {
    let yaml = sme(&format!(
        "longPress: {{q: ö}}\n{}touch:\n  default:\n    longPress: {{q: ø}}\n    sizes:\n      tablet: {{layers: {{base: '\\s{{\"q\":2}} q'}}}}\n      phone: {{layers: {{base: '\\s{{\"q\":1.5}} q \\s{{\"q\":2}}'}}}}\n",
        hardware(&[("none", QWERTY)])
    ));
    let kb = keyboard("sme", &yaml, Host::Android);
    assert_eq!(
        row_ids(&kb, &layer(&kb, "none").rows)[1],
        "u-0071 w e r t y u i o p u-005B u-005D"
    );
    let rows = |i: usize| row_ids(&kb, &kb.touch[i].layers[0].rows);
    assert_eq!(rows(0), ["u-0071-2 u-0071-3 u-0071-4"]);
    assert_eq!(rows(1), ["u-0071-4 u-0071-3"]);
    let q = kb.key(kb.key_index("u-0071-2").unwrap()).unwrap();
    assert_eq!(q.width, 1500);
    let lp = |id: &str| {
        let key = kb.key(kb.key_index(id).unwrap()).unwrap();
        row_ids(&kb, std::slice::from_ref(&key.long_press)).remove(0)
    };
    assert_eq!(lp("u-0071"), "u-00F6");
    assert_eq!(lp("u-0071-3"), "u-00F8");
}

// [spec:kbdgen:sem:ldml.yaml.key-ids.collisions+1/test]
#[test]
fn explicit_keys_reserve_their_ids_first() {
    let rows = QWERTY
        .replace("[ ]", "\\k{u-005B} [")
        .replace("a s d", "a \\k{a} d");
    let kb = keyboard(
        "sme",
        &sme(&format!(
            "keys: {{u-005B: {{output: x}}, a: {{output: á}}}}\n{}",
            hardware(&[("none", &rows)])
        )),
        Host::Web,
    );
    let ids = row_ids(&kb, &layer(&kb, "none").rows);
    assert_eq!(ids[1], "q w e r t y u i o p u-005B u-005B-2");
    assert_eq!(ids[2], "a-2 a d f g h j k l u-003B u-0027 u-0023");
    let a = kb.key(kb.key_index("a").unwrap()).unwrap();
    assert_eq!(a.output.plain(), "á");
}

// [spec:kbdgen:req:ldml.yaml.hardware.rows+1/test]
#[test]
fn space_rows_entries_and_default_fill_space() {
    let yaml = sme(&format!(
        "{}    space: {{shift: '\\u{{A0}}'}}\n",
        hardware(&[
            ("none", &format!("{QWERTY}\\s{{space:3}}\n")),
            ("shift", SHIFTED),
            ("caps", QWERTY)
        ])
    ));
    let kb = keyboard("sme", &yaml, Host::Web);
    let space = |sets: &str| row_ids(&kb, &layer(&kb, sets).rows)[4].clone();
    assert_eq!(space("none"), "space-2");
    assert_eq!(space("shift, caps shift"), "u-00A0");
    assert_eq!(space("caps"), "space");
    let err = error(&sme(&format!(
        "{}    space: {{alt: x}}\n",
        hardware(&[("none", QWERTY)])
    )));
    assert!(
        err.contains("hardware.default.space.alt: no layer is alt"),
        "{err}"
    );
}

// [spec:kbdgen:req:ldml.yaml.hardware.rows+1/test]
#[test]
fn trailing_no_key_positions_are_left_out() {
    let rows = format!(
        "{}\n\\u{{0}} \\u{{0}} \\u{{0}} \\u{{0}} \\u{{0}} \\u{{0}} \\u{{0}} \\u{{0}} \\u{{0}} \\u{{0}} \\u{{0}} \\u{{0}}\n{}\n{}\n\\u{{0}}\n",
        "\\u{0} 1 \\u{0} 3 \\u{0} \\u{0} \\u{0} \\u{0} \\u{0} \\u{0} \\u{0} \\u{0} \\s{gap}",
        iso_rows("a", "a").lines().nth(2).unwrap(),
        "\\u{0} \\u{0} \\u{0} \\u{0} \\u{0} \\u{0} \\u{0} \\u{0} \\u{0} \\u{0} \\u{0}"
    );
    let kb = keyboard("sme", &sme(&hardware(&[("none", &rows)])), Host::Web);
    assert_eq!(
        row_ids(&kb, &layer(&kb, "none").rows),
        [
            "gap 1 gap 3 gap gap gap gap gap gap gap gap gap",
            "gap",
            "a a a a a a a a a a a a"
        ]
    );
    let out = xml("sme", &sme(&hardware(&[("none", &rows)])), Host::Web);
    assert!(out.contains("<row keys=\"gap\" />"), "{out}");
}

// [spec:kbdgen:sem:ldml.yaml.implied-layers+1/test]
// [spec:kbdgen:sem:ldml.scope.macos-rules/test]
#[test]
fn implied_rule_one_makes_caps_shift_uppercase() {
    let kb = keyboard(
        "sme",
        &sme(&hardware(&[
            ("none", QWERTY),
            ("shift", SHIFTED),
            ("caps", SHIFTED),
        ])),
        Host::Web,
    );
    assert_eq!(sets(&kb), ["none", "shift, caps shift", "caps"]);
}

// [spec:kbdgen:sem:ldml.yaml.implied-layers+1/test]
#[test]
fn implied_rule_two_extends_layers_without_shift() {
    let kb = keyboard(
        "sme",
        &sme(&hardware(&[
            ("none", QWERTY),
            ("alt", QWERTY),
            ("ctrl", QWERTY),
            ("other", QWERTY),
        ])),
        Host::Web,
    );
    assert_eq!(sets(&kb), ["none, caps", "alt, alt caps", "other", "ctrl"]);
}

// [spec:kbdgen:sem:ldml.yaml.implied-layers+1/test]
#[test]
fn implied_rule_three_creates_uppercase_caps() {
    let shifted = SHIFTED.replace("! @", "\\d{´} @");
    let yaml = sme(&format!(
        "deadKeys: {{´: {{}}}}\n{}",
        hardware(&[
            ("none", QWERTY),
            ("shift", &shifted),
            ("altR", &QWERTY.replace("q w", "@ w")),
            ("altR shift", &QWERTY.replace("q w", "@ w"))
        ])
    ));
    let kb = keyboard("sme", &yaml, Host::Web);
    assert_eq!(
        sets(&kb),
        [
            "none",
            "shift, caps shift",
            "altR, altR caps",
            "altR shift, altR caps shift",
            "caps"
        ]
    );
    assert_eq!(
        row_ids(&kb, &layer(&kb, "caps").rows),
        [
            "u-0060 1 2 3 4 5 6 7 8 9 0 u-002D u-003D",
            "Q W E R T Y U I O P u-005B u-005D",
            "A S D F G H J K L u-003B u-0027 u-0023",
            "u-003C Z X C V B N M u-002C u-002E u-002F",
            "space",
        ]
    );
    let none = keyboard(
        "sme",
        &sme(&format!(
            "{}    impliedLayers: none\n",
            hardware(&[("none", QWERTY), ("shift", SHIFTED)])
        )),
        Host::Web,
    );
    assert_eq!(sets(&none), ["none", "shift"]);
}

// [spec:kbdgen:sem:ldml.yaml.touch.roles+1/test]
#[test]
fn roles_switch_layers_or_draw_gaps() {
    let yaml = touch_yaml(
        "base: '\\s{shift} \\s{symbols} \\s{return:1.5} \\s{keyboard}'\nshift: '\\s{shift} \\s{symbols}'\nsymbols-1: '\\s{symbols} \\s{shiftSymbols}'\nsymbols-2: '\\s{shiftSymbols} \\s{tab} \\s{caps}'\n",
    );
    let kb = keyboard("sme", &yaml, Host::Web);
    let key = |id: &str| {
        kb.key(kb.key_index(id).unwrap_or_else(|| panic!("{id}")))
            .unwrap()
    };
    for (id, role, layer) in [
        ("shift-shift", Role::Shift, Some("shift")),
        ("shift-base", Role::Shift, Some("base")),
        ("symbols-symbols-1", Role::Symbols, Some("symbols-1")),
        ("symbols-base", Role::Symbols, Some("base")),
        (
            "shiftSymbols-symbols-2",
            Role::ShiftSymbols,
            Some("symbols-2"),
        ),
        (
            "shiftSymbols-symbols-1",
            Role::ShiftSymbols,
            Some("symbols-1"),
        ),
        ("return", Role::Return, None),
        ("keyboard", Role::Keyboard, None),
        ("tab", Role::Tab, None),
        ("caps", Role::Caps, None),
    ] {
        let k = key(id);
        assert_eq!(
            (k.role, k.layer_id.as_deref(), k.gap),
            (Some(role), layer, layer.is_none()),
            "{id}"
        );
    }
    assert_eq!(key("return").width, 1500);
    let out = xml("sme", &yaml, Host::Web);
    assert!(
        out.contains("<key id=\"return\" gap=\"true\" width=\"1.5\" />"),
        "{out}"
    );
    assert!(
        out.contains("<kbdgen:role keyId=\"return\" role=\"return\" />"),
        "{out}"
    );
}

// [spec:kbdgen:sem:ldml.yaml.touch.roles+1/test]
#[test]
fn switching_to_a_missing_layer_fails() {
    let err = lower_error(&touch_yaml("base: 'a \\s{shift}'\n"));
    assert!(
        err.ends_with("touch.default.sizes.phone.layers.base, row 1, token 2: shift switches to layer shift, which this size lacks"),
        "{err}"
    );
    let err = lower_error(&touch_yaml("base: '\\l{numbers}'\n"));
    assert!(err.contains("this size has no layer numbers"), "{err}");
    let rows = QWERTY.replace("[ ]", "[ \\s{shift}");
    let err = lower_error(&sme(&hardware(&[("none", &rows)])));
    assert!(
        err.contains("row 2, token 12: roles, \\l{} and \\s{\"x\":w} are touch-only tokens"),
        "{err}"
    );
    let err = lower_error(&touch_yaml("base: 'a \\u{0}'\n"));
    assert!(err.contains("use \\s{gap}"), "{err}");
}

// [spec:kbdgen:sem:ldml.yaml.touch.flicks/test]
#[test]
fn flick_rows_give_positional_flicks() {
    let yaml = touch_yaml(
        "base:\n  rows: |\n    a b \\s{gap} \\s{shift}\n  flicks:\n    s: |\n      1 \\u{0} \\s{gap} \\s{shift}\n    ne s: |\n      2 3 \\u{0} \\u{0}\nshift: '\\s{shift}'\n",
    );
    let kb = keyboard("sme", &yaml, Host::Web);
    let flick = |id: &str| {
        let key = kb.key(kb.key_index(id).unwrap()).unwrap();
        let f = &kb.flicks[usize::from(key.flick.unwrap())];
        let segs: Vec<(String, String)> = f
            .segments
            .iter()
            .map(|s| {
                let d: Vec<&str> = s.directions.iter().map(|d| d.name()).collect();
                (d.join(" "), kb.key(s.key).unwrap().id.clone())
            })
            .collect();
        (f.id.clone(), segs)
    };
    assert_eq!(
        flick("u-0061"),
        (
            "flick-u-0061".to_string(),
            vec![("s".into(), "1".into()), ("ne s".into(), "2".into())]
        )
    );
    assert_eq!(
        flick("u-0062"),
        (
            "flick-u-0062".to_string(),
            vec![("ne s".into(), "3".into())]
        )
    );
    assert_eq!(kb.flicks.len(), 2);
}

// [spec:kbdgen:sem:ldml.yaml.touch.flicks/test]
#[test]
fn misaligned_flick_rows_fail() {
    let cases = [
        (
            "base:\n  rows: 'a b'\n  flicks: {s: '1'}\n",
            "flick s row 1 has 1 tokens; the layer row has 2",
        ),
        (
            "base:\n  rows: 'a b'\n  flicks: {s: \"1 2\\n3 4\"}\n",
            "flick s has 2 rows; layer base has 1",
        ),
        (
            "base:\n  rows: '\\s{gap} b'\n  flicks: {s: '1 2'}\n",
            "only a key with output takes flicks",
        ),
        (
            "base:\n  rows: 'a \\s{backspace}'\n  flicks: {s: '1 \\s{return}'}\n",
            "a role token in a flick row equals the main row's",
        ),
        (
            "base:\n  rows: 'a'\n  flicks: {up: '1'}\n",
            "up is not a direction",
        ),
    ];
    for (layers, expected) in cases {
        let yaml = touch_yaml(layers);
        let err = match load_as("sme", &yaml) {
            Err(e) => e.to_string(),
            Ok(layout) => lower(&layout).unwrap_err().to_string(),
        };
        assert!(err.contains(expected), "{layers}: {err}");
    }
}

// [spec:kbdgen:def:ldml.yaml.long-press+1/test]
#[test]
fn long_press_reaches_every_matching_key() {
    let yaml = sme(&format!(
        "longPress: {{a: á à, e: é}}\n{}touch:\n  default:\n    longPress: {{e: ë, a: ''}}\n    sizes: {{phone: {{layers: {{base: 'a e \\k{{a}}'}}}}}}\n",
        hardware(&[("none", QWERTY)])
    ));
    let kb = keyboard("sme", &yaml, Host::Android);
    let lp = |id: &str| {
        let key = kb.key(kb.key_index(id).unwrap()).unwrap();
        row_ids(&kb, std::slice::from_ref(&key.long_press)).remove(0)
    };
    let none = row_ids(&kb, &layer(&kb, "none").rows);
    assert!(none[2].starts_with("u-0061 s d"), "{none:?}");
    assert!(none[1].starts_with("q w u-0065 r"), "{none:?}");
    assert_eq!(lp("u-0061"), "u-00E1 u-00E0");
    assert_eq!(lp("u-0065"), "u-00E9");
    assert_eq!(row_ids(&kb, &kb.touch[0].layers[0].rows), ["a u-0065-2 a"]);
    assert_eq!(lp("u-0065-2"), "u-00EB");
}

// [spec:kbdgen:sem:ldml.yaml.dead-keys.compose/test]
#[test]
fn compose_group_rules_in_spec_order() {
    let rows = QWERTY.replace("[ ]", "\\d{´} \\d{¨}");
    let yaml = sme(&format!(
        "deadKeys:\n  ´: {{compose: {{a: á, ¨: {{compose: {{u: ǘ}}}}, '.': '$'}}}}\n  ¨: {{compose: {{u: ü}}}}\n{}",
        hardware(&[("none", &rows)])
    ));
    let out = xml("sme", &yaml, Host::Web);
    let compose = [
        r#"<transform from="\m{dk_00B4}a" to="á" />"#,
        r#"<transform from="\m{dk_00B4}¨" to="\m{dk_00B4-00A8}" />"#,
        r#"<transform from="\m{dk_00B4}\m{dk_00A8}" to="\m{dk_00B4-00A8}" />"#,
        r#"<transform from="\m{dk_00B4}\." to="$$" />"#,
        r#"<transform from="\m{dk_00B4}\u{20}" to="´" />"#,
        r#"<transform from="\m{dk_00B4-00A8}u" to="ǘ" />"#,
        r#"<transform from="\m{dk_00B4-00A8}\u{20}" to="´¨" />"#,
        r#"<transform from="\m{dk_00A8}u" to="ü" />"#,
        r#"<transform from="\m{dk_00A8}\u{20}" to="¨" />"#,
        r#"<kbdgen:generated by="deadKeys-compose" />"#,
    ];
    let mut at = 0;
    for line in compose {
        let found = out[at..]
            .find(line)
            .unwrap_or_else(|| panic!("{line}\n{out}"));
        at += found + line.len();
    }
}

// [spec:kbdgen:sem:ldml.yaml.dead-keys.fallback/test]
#[test]
fn fallback_covers_the_longest_output() {
    let rows = QWERTY.replace("[ ]", "\\d{´} \\k{long}");
    let yaml = |n: usize| {
        sme(&format!(
            "deadKeys: {{´: {{standalone: ''}}}}\nkeys: {{long: {{output: {}}}}}\n{}",
            "x".repeat(n),
            hardware(&[("none", &rows)])
        ))
    };
    let out = xml("sme", &yaml(9), Host::Web);
    assert!(
        out.contains(r#"<transform from="\m{dk_00B4}\m{dk_00B4}" to="\m{dk_00B4}" />"#),
        "{out}"
    );
    assert!(
        out.contains(r#"<transform from="\m{dk_00B4}(.{1,9})" to="$1" />"#),
        "{out}"
    );
    let out = xml("sme", &yaml(19), Host::Web);
    assert!(
        out.contains(r#"from="\m{dk_00B4}(.{1,9}.{0,9}.{0,9})""#),
        "{out}"
    );
    let enabled = sme(&format!(
        "normalization: enabled\ndeadKeys: {{´: {{}}}}\nkeys: {{long: {{output: ǘǘǘǘ}}}}\n{}",
        hardware(&[("none", &rows)])
    ));
    let out = xml("sme", &enabled, Host::Web);
    assert!(out.contains(r#"from="\m{dk_00B4}(.{1,9}.{0,9})""#), "{out}");
}

// [spec:kbdgen:sem:ldml.yaml.dead-keys.keys/test]
#[test]
fn only_reachable_dead_keys_are_lowered() {
    let rows = QWERTY.replace("[ ]", "\\d{´} ]");
    let yaml = sme(&format!(
        "deadKeys:\n  ´: {{name: Acute, display: ˊ, compose: {{a: á}}}}\n  ˇ: {{compose: {{c: č}}}}\n{}touch: {{iOS: {{sizes: {{phone: {{layers: {{base: a}}}}}}}}}}\n",
        hardware(&[("none", &rows)])
    ));
    let kb = keyboard("sme", &yaml, Host::Web);
    assert_eq!(kb.markers, ["dk_00B4"]);
    assert_eq!(kb.flush.get(&0).map(String::as_str), Some("´"));
    assert_eq!(kb.dead_key_names.get(&0).map(String::as_str), Some("Acute"));
    assert_eq!(kb.displays.entries[0].display, "ˊ");
    let key = kb.key(kb.key_index("dk-dk_00B4").unwrap()).unwrap();
    assert_eq!(key.output.markers().collect::<Vec<_>>(), [0]);
    let ios = keyboard("sme", &yaml, Host::Ios);
    assert!(ios.markers.is_empty() && ios.simple.is_empty() && ios.backspace.is_empty());
    let resolved = kbd_ldml::resolve(&document("sme", &yaml, Host::Ios)).unwrap();
    assert_eq!(
        resolved.extensions.dead_keys.len(),
        2,
        "the whole table is metadata"
    );
}

// [spec:kbdgen:sem:ldml.yaml.dead-keys.backspace/test]
#[test]
fn backspace_group_follows_the_layouts_own() {
    let rows = QWERTY.replace("[ ]", "\\d{´} ]");
    let yaml = sme(&format!(
        "deadKeys: {{´: {{}}}}\nbackspace: [[{{from: x}}]]\ntransforms: [[{{from: q, to: Q}}]]\n{}",
        hardware(&[("none", &rows)])
    ));
    let resolved = kbd_ldml::resolve(&document("sme", &yaml, Host::Web)).unwrap();
    let kb = &resolved.keyboard;
    assert_eq!(kb.simple.len(), 3);
    assert_eq!(kb.backspace.len(), 2);
    let TransformGroup::Rules(last) = &kb.backspace[1] else {
        panic!("rules");
    };
    assert_eq!(last.len(), 1);
    assert!(last[0].to.is_empty());
    let marks: Vec<(kbd_model::TransformList, usize, &str)> = resolved
        .extensions
        .generated
        .iter()
        .map(|g| (g.list, g.group, g.by.as_str()))
        .collect();
    assert_eq!(
        marks,
        [
            (kbd_model::TransformList::Simple, 0, "deadKeys-compose"),
            (kbd_model::TransformList::Simple, 1, "deadKeys-fallback"),
            (kbd_model::TransformList::Backspace, 1, "deadKeys-backspace"),
        ]
    );
}

// [spec:kbdgen:sem:ldml.yaml.displays.auto/test]
#[test]
fn automatic_displays_unless_covered() {
    let rows = QWERTY
        .replace("[ ]", "\\u{301} \\u{300}\\u{302}")
        .replace("< z", "\\d{´} \\u{323}");
    let yaml = sme(&format!(
        "keyNames: {{space: rom}}\ndeadKeys: {{´: {{display: ˊ}}}}\ndisplays: [{{output: '\\u{{323}}', display: dot}}]\n{}",
        hardware(&[("none", &rows)])
    ));
    let kb = keyboard("sme", &yaml, Host::Web);
    let shown: Vec<(String, &str)> = kb
        .displays
        .entries
        .iter()
        .map(|d| {
            let target = match &d.target {
                DisplayTarget::Output(t) => t.plain(),
                DisplayTarget::Key(k) => format!("#{}", kb.key(*k).unwrap().id),
            };
            (target, d.display.as_str())
        })
        .collect();
    assert_eq!(
        shown,
        [
            ("\u{323}".to_string(), "dot"),
            (String::new(), "ˊ"),
            ("\u{301}".to_string(), "\u{25CC}\u{301}"),
            ("\u{300}\u{302}".to_string(), "\u{25CC}\u{300}\u{302}"),
            ("#space".to_string(), "rom"),
        ]
    );
    assert_eq!(kb.displays.labels.space.as_deref(), Some("rom"));
}

// [spec:kbdgen:def:ldml.yaml.verbatim+1/test]
#[test]
fn verbatim_fields_reach_the_document_as_written() {
    let rows = QWERTY.replace("[ ]", "\\k{e-acute} ]");
    let yaml = sme(&format!(
        "variables: {{strings: {{acute: '\\u{{301}}'}}, sets: {{vowel: a e}}, usets: {{u: '[a-z]'}}}}\ntransforms:\n  - [{{from: '$[vowel]${{acute}}', to: x}}]\n  - reorder: [{{from: '\\u{{301}}', order: 10}}]\nkeys: {{e-acute: {{output: 'e${{acute}}', longPress: a, flick: f}}}}\nflicks: {{f: [{{directions: n, key: a}}]}}\ndisplays: [{{keyId: e-acute, display: É}}, {{displayBase: '◌'}}]\n{}",
        hardware(&[("none", &rows)])
    ));
    let out = xml("sme", &yaml, Host::Web);
    for fragment in [
        r#"<string id="acute" value="\u{301}" />"#,
        r#"<set id="vowel" value="a e" />"#,
        r#"<uset id="u" value="[a-z]" />"#,
        r#"<transform from="$[vowel]${acute}" to="x" />"#,
        r#"<reorder from="\u{301}" order="10" />"#,
        r#"<key id="e-acute" flickId="f" output="e${acute}" longPressKeyIds="a" />"#,
        r#"<flickSegment directions="n" keyId="a" />"#,
        r#"<display keyId="e-acute" display="É" />"#,
        r#"<displayOptions baseCharacter="◌" />"#,
    ] {
        assert!(out.contains(fragment), "{fragment}\n{out}");
    }
    let kb = keyboard("sme", &yaml, Host::Web);
    assert_eq!(
        kb.key(kb.key_index("e-acute").unwrap())
            .unwrap()
            .output
            .plain(),
        "e\u{301}"
    );
}

// [spec:kbdgen:sem:ldml.yaml.normalization/test]
#[test]
fn normalization_defaults_to_disabled() {
    let rows = QWERTY.replace("[ ]", "á ]");
    let disabled = keyboard("sme", &sme(&hardware(&[("none", &rows)])), Host::Web);
    assert_eq!(disabled.normalization, Normalization::Disabled);
    let out = xml("sme", &sme(&hardware(&[("none", &rows)])), Host::Web);
    assert!(out.contains("<settings normalization=\"disabled\" />"));
    let yaml = sme(&format!(
        "normalization: enabled\n{}",
        hardware(&[("none", &rows)])
    ));
    assert!(!xml("sme", &yaml, Host::Web).contains("<settings"));
    let enabled = keyboard("sme", &yaml, Host::Web);
    assert_eq!(enabled.normalization, Normalization::Enabled);
    let key = |kb: &Keyboard| {
        kb.key(kb.key_index("u-00E1").unwrap())
            .unwrap()
            .output
            .plain()
    };
    assert_eq!(key(&disabled), "á");
    assert_eq!(key(&enabled), "a\u{301}");
}

// [spec:kbdgen:sem:ldml.yaml.hosts/test]
#[test]
fn hosts_take_the_first_variant_of_their_chains() {
    let names = |yaml: &str| -> Vec<&'static str> {
        documents("sme", yaml)
            .iter()
            .map(|(h, _)| h.name())
            .collect()
    };
    let touch = "touch: {iOS: {sizes: {phone: {layers: {base: a}}}}}\n";
    assert_eq!(
        names(&sme(&format!("{}{touch}", hardware(&[("none", QWERTY)])))),
        [
            "windows", "macOS", "chromeOS", "linux", "iOS", "android", "web"
        ]
    );
    assert_eq!(names(&sme(touch)), ["iOS"]);
    assert_eq!(
        names(&sme(
            "touch: {default: {sizes: {phone: {layers: {base: a}}}}}\n"
        )),
        ["iOS", "android", "web"]
    );
    let android = sme(&format!(
        "{}touch: {{android: {{sizes: {{phone: {{layers: {{base: a}}}}}}}}}}\n",
        hardware(&[("none", QWERTY)]).replace("default:", "android:")
    ));
    assert_eq!(names(&android), ["android"]);
    let kb = keyboard("sme", &android, Host::Android);
    assert!(kb.hardware.is_some() && kb.touch.len() == 1);
    assert_eq!(kb.host, Some(Host::Android));
}

// [spec:kbdgen:sem:ldml.yaml.hosts/test]
#[test]
fn equal_documents_share_one_model() {
    let windows = hardware(&[("altR", QWERTY)]).replace("hardware:\n  default:", "  windows:");
    let yaml = sme(&format!("{}{windows}", hardware(&[("none", QWERTY)])));
    let docs: Vec<HostDocument> = documents("sme", &yaml)
        .into_iter()
        .map(|(host, source)| HostDocument {
            tag: "sme".to_string(),
            host,
            source,
        })
        .collect();
    let layouts = compiled_layouts(&docs).unwrap();
    let layout = &layouts[0];
    assert_eq!(layout.keyboards.len(), 2);
    assert_eq!(layout.hosts[&Host::MacOs], layout.hosts[&Host::Linux]);
    assert_ne!(layout.hosts[&Host::MacOs], layout.hosts[&Host::Windows]);
    let mac = xml("sme", &yaml, Host::MacOs);
    let linux = xml("sme", &yaml, Host::Linux);
    assert_eq!(mac.replace("host=\"macOS\"", "host=\"linux\""), linux);
}

// [spec:kbdgen:req:ldml.yaml.ldml-ref+3/test]
#[test]
fn ldml_ref_keeps_the_file_and_overrides_data() {
    let dir = tempfile::tempdir().unwrap();
    let kb_xml = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<keyboard3 xmlns=\"https://schemas.unicode.org/cldr/45/keyboard3\" locale=\"sme\" conformsTo=\"45\">\n  <!-- kept -->\n  <info name=\"File\" />\n  <layers formId=\"iso\"><layer modifiers=\"none\"><row keys=\"a b\" /></layer></layers>\n</keyboard3>\n";
    std::fs::write(dir.path().join("kb.xml"), kb_xml).unwrap();
    std::fs::write(dir.path().join("ios.xml"), kb_xml.replace("a b", "b a")).unwrap();
    let path = dir.path().join("sme.yaml");
    std::fs::write(
        &path,
        sme("ldml: {default: kb.xml, iOS: ios.xml}\ndecimal: ','\nkeyNames: {return: ruovttu}\n"),
    )
    .unwrap();
    let layout = load(&path, "sme").unwrap();
    let docs = lower(&layout).unwrap();
    assert_eq!(docs.len(), 7);
    let (_, ios) = docs.iter().find(|(h, _)| *h == Host::Ios).unwrap();
    let out = kbd_ldml::write(&ios.document);
    assert!(
        out.contains("<!-- kept -->") && out.contains("keys=\"b a\""),
        "{out}"
    );
    let resolved = kbd_ldml::resolve(ios).unwrap();
    assert_eq!(resolved.keyboard.host, Some(Host::Ios));
    assert_eq!(resolved.keyboard.decimal.as_ref().unwrap().plain(), ",");
    assert_eq!(
        resolved.keyboard.displays.labels.r#return.as_deref(),
        Some("ruovttu")
    );
    assert_eq!(resolved.extensions.display_names["sme"], "Davvisámegiella");
    std::fs::write(&path, sme("ldml: {macOS: kb.xml}\n")).unwrap();
    let only: Vec<Host> = lower(&load(&path, "sme").unwrap())
        .unwrap()
        .iter()
        .map(|(h, _)| *h)
        .collect();
    assert_eq!(only, [Host::MacOs]);
}

// [spec:kbdgen:sem:ldml.yaml.lowering+1/test]
#[test]
fn lowering_is_deterministic_export_form() {
    let first: Vec<String> = documents("vro", VRO)
        .iter()
        .map(|(_, s)| kbd_ldml::write(&s.document))
        .collect();
    let second: Vec<String> = documents("vro", VRO)
        .iter()
        .map(|(_, s)| kbd_ldml::write(&s.document))
        .collect();
    assert_eq!(first, second);
    let mac = &first[1];
    let order = [
        "<info",
        "<settings",
        "<displays",
        "<keys",
        "<flicks",
        "<layers",
        "<transforms",
        "<special>\n    <kbdgen:flush",
    ];
    let positions: Vec<usize> = order.iter().filter_map(|tag| mac.find(tag)).collect();
    assert!(positions.windows(2).all(|w| w[0] < w[1]), "{mac}");
    for (host, source) in documents("vro", VRO) {
        let resolved = kbd_ldml::resolve(&source).unwrap();
        let again = kbd_ldml::export(&resolved.keyboard, &resolved.extensions);
        let back = kbd_ldml::resolve(&SourceDocument::generated("x", again)).unwrap();
        assert_eq!(back.keyboard, resolved.keyboard, "{}", host.name());
    }
}

// [spec:kbdgen:def:ldml.yaml.targets/test]
#[test]
fn targets_reach_the_model() {
    let yaml = sme(&format!(
        "targets: {{windows: {{locale: se-Latn, shiftLock: true, keyNames: {{Caps Lock: Stuorrabustávat}}}}, iOS: {{spellerPath: x.zhfst}}}}\n{}",
        hardware(&[("none", QWERTY)])
    ));
    let resolved = kbd_ldml::resolve(&document("sme", &yaml, Host::Windows)).unwrap();
    let kb = &resolved.keyboard;
    assert!(kb.windows.shift_lock && !kb.windows.lrm_rlm);
    assert_eq!(kb.windows.key_names["Caps Lock"], "Stuorrabustávat");
    let targets: Vec<(&str, &str, &str)> = resolved
        .extensions
        .targets
        .iter()
        .map(|t| (t.host.as_str(), t.name.as_str(), t.value.as_str()))
        .collect();
    assert_eq!(
        targets,
        [
            ("windows", "locale", "se-Latn"),
            ("iOS", "spellerPath", "x.zhfst")
        ]
    );
}
