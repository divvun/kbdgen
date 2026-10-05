//! Loading v4 files: detection, the strict schema and its addressing,
//! escapes, rows and tokens, and the checks of whole variants.

use kbd_ldml::escape::Piece;
use kbd_model::{BottomRow, ModifierSet, Role};
use serde_yaml::Value;

use super::*;
use crate::ldml::yaml::schema::{ComposeTo, FormSpec};
use crate::ldml::yaml::text::Strings;
use crate::ldml::yaml::tokens::{Token, rows, token};
use crate::ldml::yaml::{At, LayoutFormat, detect};

fn parse(text: &str) -> Value {
    serde_yaml::from_str(text).unwrap()
}

fn tok(raw: &str) -> Result<Token, String> {
    token(raw, &Strings::default(), &At::file("t")).map_err(|e| e.to_string())
}

fn chars(s: &str) -> Vec<Piece> {
    s.chars().map(Piece::Char).collect()
}

// [spec:kbdgen:def:ldml.yaml.detect/test]
#[test]
fn format_four_is_v4_and_others_fail() {
    assert_eq!(detect(&parse("format: 4\n")), Ok(LayoutFormat::V4));
    assert_eq!(detect(&parse("displayNames: {}\n")), Ok(LayoutFormat::V3));
    assert_eq!(detect(&parse("format: '4'\n")), Err("'4'".to_string()));
    assert_eq!(detect(&parse("format: 4.0\n")), Err("4.0".to_string()));
    assert_eq!(detect(&parse("format: 3\n")), Err("3".to_string()));
    let (_dir, path) = write("sme", "format: v4\n");
    let err = load(&path, "sme").unwrap_err().to_string();
    assert!(
        err.contains("sme.yaml") && err.contains("format v4"),
        "{err}"
    );
}

// [spec:kbdgen:def:ldml.yaml.detect/test]
// [spec:kbdgen:def:ldml.yaml.schema/test]
#[test]
fn autonym_and_display_names_are_required() {
    let err = error("format: 4\n");
    assert!(err.contains("displayNames is required"), "{err}");
    let err = error("format: 4\ndisplayNames: {en: Northern Sami}\n");
    assert!(
        err.contains("displayNames: displayNames has no autonym"),
        "{err}"
    );
    let layout = load_as("se-FI", "format: 4\ndisplayNames: {se: Davvisámegiella}\n").unwrap();
    assert_eq!(layout.tag, "se-FI");
}

// [spec:kbdgen:req:ldml.yaml.strict/test]
// [spec:kbdgen:def:ldml.yaml.schema/test]
#[test]
fn unknown_fields_fail_with_their_path() {
    let cases = [
        ("colour: red\n", "colour: unknown field colour"),
        ("info: {nom: x}\n", "info.nom: unknown field nom"),
        (
            "deadKeys: {´: {compse: {a: á}}}\n",
            "deadKeys.´.compse: unknown field compse",
        ),
        (
            "hardware: {default: {layrs: {}}}\n",
            "hardware.default.layrs: unknown field layrs",
        ),
        (
            "touch: {iOS: {sizes: {phone: {layers: {base: {rows: a, flick: {}}}}}}}\n",
            "touch.iOS.sizes.phone.layers.base.flick: unknown field flick",
        ),
        (
            "transforms: [[{from: a, too: b}]]\n",
            "transforms[1][1].too: unknown field too",
        ),
        (
            "targets: {windows: {languageName: Võro}}\n",
            "targets.windows.languageName: unknown field languageName",
        ),
        (
            "emoji: {key: {position: C01, modifiers: none, mods: x}}\n",
            "emoji.key.mods",
        ),
    ];
    for (body, expected) in cases {
        let err = error(&sme(body));
        assert!(err.contains(expected), "{body}: {err}");
        assert!(err.contains("sme.yaml: "), "{err}");
    }
}

// [spec:kbdgen:req:ldml.yaml.strict/test]
#[test]
fn row_errors_name_row_and_token() {
    let rows = QWERTY.replace("[ ]", "[ \\d{´");
    let err = error(&sme(&hardware(&[("none", &rows)])));
    assert!(
        err.ends_with(
            "sme.yaml: hardware.default.layers.none, row 2, token 12: \\d{´ does not end with }"
        ),
        "{err}"
    );
    let rows = QWERTY.replace("[ ]", "[ \\d{´}");
    let err = lower_error(&sme(&hardware(&[("none", &rows)])));
    assert!(
        err.ends_with(
            "hardware.default.layers.none, row 2, token 12: no dead key has the identity \"´\""
        ),
        "{err}"
    );
}

// [spec:kbdgen:req:ldml.yaml.strict/test]
#[test]
fn odd_yaml_is_an_error_not_a_panic() {
    for body in [
        "- a\n- b\n",
        "format: 4\ndisplayNames: [sme]\n",
        "format: 4\ndisplayNames: {sme: 1}\n",
        "format: 4\n1: x\ndisplayNames: {sme: x}\n",
        "format: 4\ndisplayNames: {sme: x}\nhardware: !tag {}\n",
        "format: 4\ndisplayNames: {sme: x}\ndeadKeys: {´: {compose: {1: ¹}}}\n",
        "format: 4\ndisplayNames: {sme: x}\nhardware: {default: {layers: {none: 5}}}\n",
        "format: 4\ndisplayNames: {sme: x}\ntouch: {iOS: {sizes: {phone: {layers: {}}}}}\n",
        "format: 4\ndisplayNames: {sme: x}\nkeys: {a: {width: wide}}\n",
        "format: 4\ndisplayNames: {sme: x}\nlongPress: {a: [b]}\n",
        "format: 4\ndisplayNames: {sme: x}\nvariables: {strings: {a: '${a}'}}\n",
    ] {
        let result = std::panic::catch_unwind(|| load_as("sme", body));
        assert!(matches!(result, Ok(Err(_))), "{body}");
    }
    let err = error(&sme("deadKeys: {´: {compose: {1: ¹}}}\n"));
    assert!(err.contains("quote it"), "{err}");
}

// [spec:kbdgen:syn:ldml.yaml.escape/test]
#[test]
fn bare_backslash_u_is_an_error() {
    for body in [
        "decimal: '\\u00A0'\n",
        "keyNames: {space: '\\u0020'}\n",
        "deadKeys: {'\\u00B4': {}}\n",
        "transforms: [[{from: '\\u0300', to: x}]]\n",
        "variables: {sets: {s: 'a \\u0300'}}\n",
        "displays: [{output: a, display: '\\u0301'}]\n",
    ] {
        let err = error(&sme(body));
        assert!(err.contains("is not followed by {"), "{body}: {err}");
    }
    let rows = QWERTY.replace("[ ]", "[ \\u0301");
    let err = error(&sme(&hardware(&[("none", &rows)])));
    assert!(err.contains("row 2, token 12: \\u at character 1"), "{err}");
    load_as(
        "sme",
        &sme("transforms: [[{from: 'a\\\\u', to: '\\\\u'}]]\n"),
    )
    .unwrap();
}

// [spec:kbdgen:syn:ldml.yaml.escape/test]
#[test]
fn escapes_are_decoded_once_for_comparison() {
    let err = error(&sme("deadKeys: {´: {}, '\\u{B4}': {}}\n"));
    assert!(err.contains("the identity \"´\" occurs twice"), "{err}");
    let err = error(&sme("longPress: {a: b, '\\u{61}': c}\n"));
    assert!(err.contains("occurs twice"), "{err}");
    let layout = load_as(
        "sme",
        &sme("decimal: '\\u{2C}'\nkeyNames: {space: '\\u{A0}x'}\n"),
    )
    .unwrap();
    assert_eq!(layout.decimal, Some(chars(",")));
    assert_eq!(layout.space_label.as_deref(), Some("\u{A0}x"));
    assert_eq!(layout.display_names["sme"], "Davvisámegiella");
}

// [spec:kbdgen:syn:ldml.yaml.rows/test]
#[test]
fn rows_split_on_ascii_blanks_only() {
    let parsed = rows(
        "\n  a\tb  \n\n   \nc\u{A0}d \\u{20}\n",
        &Strings::default(),
        &At::file("t"),
    )
    .unwrap();
    assert_eq!(
        parsed,
        vec![
            vec![Token::Output(chars("a")), Token::Output(chars("b"))],
            vec![Token::Output(chars("c\u{A0}d")), Token::Output(chars(" "))],
        ]
    );
}

// [spec:kbdgen:syn:ldml.yaml.tokens/test]
#[test]
fn tokens_are_tried_in_table_order() {
    let w = |n: u32| Some(n);
    assert_eq!(tok("\\u{0}"), Ok(Token::NoKey));
    assert_eq!(tok("\\s{gap}"), Ok(Token::Gap(None)));
    assert_eq!(tok("\\s{gap:1.5}"), Ok(Token::Gap(w(1500))));
    assert_eq!(tok("\\s{spacer:0.25}"), Ok(Token::Gap(w(250))));
    assert_eq!(tok("\\s{space:4}"), Ok(Token::Space(w(4000))));
    assert_eq!(
        tok("\\s{shift:1.25}"),
        Ok(Token::Role(Role::Shift, w(1250)))
    );
    assert_eq!(
        tok("\\s{shiftSymbols}"),
        Ok(Token::Role(Role::ShiftSymbols, None))
    );
    assert_eq!(tok("\\s{\"x:y\":2}"), Ok(Token::Sized(chars("x:y"), 2000)));
    assert_eq!(tok("\\d{\\u{B4}}"), Ok(Token::Dead("´".to_string())));
    assert_eq!(
        tok("\\k{e-acute}"),
        Ok(Token::KeyRef("e-acute".to_string()))
    );
    assert_eq!(
        tok("\\l{symbols-1:2}"),
        Ok(Token::Layer("symbols-1".to_string(), w(2000)))
    );
    assert_eq!(tok("\\"), Ok(Token::Output(chars("\\"))));
    assert_eq!(tok("\\x"), Ok(Token::Output(chars("\\x"))));
    assert_eq!(
        tok("a\\m{acute}"),
        Ok(Token::Output(vec![
            Piece::Char('a'),
            Piece::Marker("acute".to_string())
        ]))
    );
    assert!(
        tok("\\s{hyper}")
            .unwrap_err()
            .contains("hyper is not a role")
    );
    assert!(
        tok("\\s{gap:0}")
            .unwrap_err()
            .contains("not a width greater than 0")
    );
    assert!(tok("\\s{gap:1.2345}").is_err());
    assert!(tok("\\k{}").is_err());
    assert!(tok("a\\u{0}").unwrap_err().contains("only valid alone"));
}

// [spec:kbdgen:syn:ldml.yaml.modifier-names/test]
#[test]
fn modifier_names_are_sets_in_any_order() {
    let err = error(&sme(&hardware(&[
        ("caps shift", QWERTY),
        ("shift caps", SHIFTED),
    ])));
    assert!(
        err.contains("layers.shift caps: caps shift overlaps caps shift"),
        "{err}"
    );
    let err = error(&sme(&hardware(&[("alt", QWERTY), ("altR", SHIFTED)])));
    assert!(err.contains("altR overlaps alt of layer alt"), "{err}");
    let err = error(&sme(&hardware(&[
        ("alt, caps", QWERTY),
        ("none, alt", SHIFTED),
    ])));
    assert!(err.contains("overlaps"), "{err}");
    let err = error(&sme(&hardware(&[("hyper", QWERTY)])));
    assert!(err.contains("unknown modifier component hyper"), "{err}");
    let layout = load_as(
        "sme",
        &sme(&hardware(&[
            ("ctrl", QWERTY),
            ("ctrlL", SHIFTED),
            ("none", QWERTY),
        ])),
    )
    .unwrap();
    let keys: Vec<&str> = layout.hardware[0]
        .layers
        .iter()
        .map(|l| l.key.as_str())
        .collect();
    assert_eq!(keys, ["ctrl", "ctrlL", "none"]);
}

// [spec:kbdgen:def:ldml.yaml.hardware/test]
#[test]
fn inherits_replaces_layers_key_by_key() {
    let yaml = sme(&format!(
        "{}  windows:\n    inherits: default\n    impliedLayers: none\n    form: abnt2\n    layers:\n      caps shift: |\n{}      altR: |\n{}    space: {{none: '\\u{{A0}}'}}\n",
        hardware(&[("none", QWERTY), ("shift caps", SHIFTED)]),
        indent(QWERTY, 8),
        indent(SHIFTED, 8),
    ));
    let layout = load_as("sme", &yaml).unwrap();
    let windows = &layout.hardware[1];
    assert_eq!(windows.name, "windows");
    assert_eq!(windows.form, FormSpec::Implied("abnt2".to_string()));
    assert!(!windows.implied && layout.hardware[0].implied);
    let keys: Vec<&str> = windows.layers.iter().map(|l| l.key.as_str()).collect();
    assert_eq!(keys, ["none", "caps shift", "altR"]);
    assert_eq!(windows.layers[1].rows[1][0], Token::Output(chars("q")));
    assert_eq!(windows.space.len(), 1);
    assert_eq!(layout.hardware[0].space.len(), 0);
}

// [spec:kbdgen:def:ldml.yaml.hardware/test]
#[test]
fn inheritance_cycles_and_unknown_parents_fail() {
    let err = error(&sme(
        "hardware:\n  macOS: {inherits: windows}\n  windows: {inherits: macOS}\n",
    ));
    assert!(
        err.contains("hardware.windows.inherits: inherits macOS forms a cycle"),
        "{err}"
    );
    let err = error(&sme("hardware: {macOS: {inherits: linux}}\n"));
    assert!(err.contains("there is no hardware variant linux"), "{err}");
    let err = error(&sme("touch: {iOS: {inherits: iOS, sizes: {}}}\n"));
    assert!(err.contains("forms a cycle"), "{err}");
    let err = error(&sme("hardware: {amiga: {}}\n"));
    assert!(err.contains("amiga is not a hardware variant"), "{err}");
}

// [spec:kbdgen:req:ldml.yaml.hardware.rows/test]
#[test]
fn a_49_token_row_is_caught() {
    let rows = QWERTY.replace("< z", "< > z");
    let err = lower_error(&sme(&hardware(&[("none", &rows)])));
    assert!(
        err.ends_with(
            "hardware.default.layers.none, row 4: layer none row 4 has 12 tokens; form iso row 4 has 11 scan codes"
        ),
        "{err}"
    );
    let abnt2 = sme(&format!(
        "{}    form: abnt2\n",
        hardware(&[("none", &rows)])
    ));
    keyboard("sme", &abnt2, Host::Web);
    let three = QWERTY.lines().take(3).collect::<Vec<_>>().join("\n");
    let err = lower_error(&sme(&hardware(&[("none", &three)])));
    assert!(
        err.contains("layer none has 3 rows; form iso has 4 character rows"),
        "{err}"
    );
    let six = format!("{QWERTY}x\ny\n");
    assert!(lower_error(&sme(&hardware(&[("none", &six)]))).contains("has 6 rows"));
    let wide_space = format!("{QWERTY}x y\n");
    let err = lower_error(&sme(&hardware(&[("none", &wide_space)])));
    assert!(
        err.contains("row 5: the space row of layer none has 2 tokens"),
        "{err}"
    );
}

// [spec:kbdgen:def:ldml.yaml.touch/test]
#[test]
fn sizes_have_widths_base_layers_and_distinct_widths() {
    let yaml = sme(
        "touch:\n  default:\n    sizes:\n      phone: {layers: {base: a}}\n      tablet: {bottomRow: authored, layers: {base: a}}\n      tablet-large: {layers: {base: a}}\n      watch: {minDeviceWidth: 30, layers: {base: a}}\n",
    );
    let layout = load_as("sme", &yaml).unwrap();
    let sizes: Vec<(&str, Option<u16>, BottomRow)> = layout.touch[0]
        .sizes
        .iter()
        .map(|s| (s.name.as_str(), s.min_device_width, s.bottom_row))
        .collect();
    assert_eq!(
        sizes,
        [
            ("phone", None, BottomRow::Host),
            ("tablet", Some(95), BottomRow::Authored),
            ("tablet-large", Some(190), BottomRow::Host),
            ("watch", Some(30), BottomRow::Host),
        ]
    );
    let err = error(&sme(
        "touch: {iOS: {sizes: {watch: {layers: {base: a}}}}}\n",
    ));
    assert!(err.contains("size watch needs minDeviceWidth"), "{err}");
    let err = error(&sme(
        "touch: {iOS: {sizes: {phone: {layers: {shift: a}}}}}\n",
    ));
    assert!(err.contains("every size has a base layer"), "{err}");
    let err = error(&sme(
        "touch: {iOS: {sizes: {tablet: {layers: {base: a}}, big: {minDeviceWidth: 95, layers: {base: a}}}}}\n",
    ));
    assert!(
        err.contains("sizes tablet and big have the same minDeviceWidth"),
        "{err}"
    );
    let err = error(&sme(
        "touch: {iOS: {sizes: {phone: {minDeviceWidth: 1000, layers: {base: a}}}}}\n",
    ));
    assert!(err.contains("not a width from 1 to 999"), "{err}");
}

// [spec:kbdgen:def:ldml.yaml.touch/test]
#[test]
fn touch_inherits_sizes_and_long_press() {
    let yaml = sme(
        "longPress: {a: á}\ntouch:\n  default:\n    longPress: {e: é, o: ó}\n    sizes: {phone: {layers: {base: a e}}, tablet: {layers: {base: a}}}\n  iOS:\n    inherits: default\n    longPress: {e: ë}\n    sizes: {tablet: {layers: {base: e}}}\n",
    );
    let layout = load_as("sme", &yaml).unwrap();
    let ios = &layout.touch[1];
    let lp: Vec<(Vec<Piece>, Vec<Token>)> = ios
        .long_press
        .iter()
        .map(|e| (e.output.clone(), e.candidates.clone()))
        .collect();
    assert_eq!(
        lp,
        [
            (chars("e"), vec![Token::Output(chars("ë"))]),
            (chars("o"), vec![Token::Output(chars("ó"))]),
        ]
    );
    assert_eq!(ios.sizes.len(), 2);
    assert_eq!(
        ios.sizes[1].layers[0].rows,
        vec![vec![Token::Output(chars("e"))]]
    );
}

// [spec:kbdgen:def:ldml.yaml.dead-keys/test]
// [spec:kbdgen:sem:ldml.yaml.dead-keys.keys/test]
#[test]
fn dead_keys_default_markers_and_strings() {
    let yaml = sme(
        "deadKeys:\n  ´:\n    compose: {a: á, ¨: {compose: {u: ǘ}}}\n  '\\u{A0}\\u{330}': {standalone: '', display: x, name: Tilde below}\n  ˇ: {marker: caron}\n",
    );
    let layout = load_as("sme", &yaml).unwrap();
    let acute = &layout.dead_keys[0];
    assert_eq!(
        (
            acute.marker.as_str(),
            acute.display.as_str(),
            acute.standalone.as_str()
        ),
        ("dk_00B4", "´", "´")
    );
    let ComposeTo::Node(nested) = &acute.compose[1].value else {
        panic!("nested");
    };
    assert_eq!(
        (
            nested.identity.as_str(),
            nested.marker.as_str(),
            nested.standalone.as_str()
        ),
        ("´¨", "dk_00B4-00A8", "´¨")
    );
    let tilde = &layout.dead_keys[1];
    assert_eq!(tilde.marker, "dk_00A0_0330");
    assert_eq!(
        (tilde.standalone.as_str(), tilde.display.as_str()),
        ("", "x")
    );
    assert_eq!(tilde.name.as_deref(), Some("Tilde below"));
    assert_eq!(layout.dead_keys[2].marker, "caron");
}

// [spec:kbdgen:def:ldml.yaml.dead-keys/test]
#[test]
fn dead_key_errors_are_addressed() {
    let cases = [
        (
            "deadKeys: {´: {compose: {' ': ´}}}\n",
            "deadKeys.´.compose. : the input \" \" is what standalone is for",
        ),
        (
            "deadKeys: {´: {marker: m}, ˇ: {marker: m}}\n",
            "the marker m is used twice",
        ),
        (
            "deadKeys: {´: {marker: 'a b'}}\n",
            "deadKeys.´.marker: marker \"a b\" must be letters",
        ),
        (
            "deadKeys: {´: {compose: {a: {name: x}}}}\n",
            "only a top-level dead key has a name",
        ),
        (
            "deadKeys: {´: {compose: {a: á, '\\u{61}': à}}}\n",
            "the input \"a\" occurs twice",
        ),
    ];
    for (body, expected) in cases {
        let err = error(&sme(body));
        assert!(err.contains(expected), "{body}: {err}");
    }
}

// [spec:kbdgen:req:ldml.yaml.ldml-ref/test]
#[test]
fn ldml_ref_excludes_what_the_file_defines() {
    for field in [
        "hardware: {}",
        "deadKeys: {}",
        "normalization: enabled",
        "info: {}",
        "keys: {}",
    ] {
        let err = error(&sme(&format!("ldml: kb.xml\n{field}\n")));
        assert!(
            err.contains("a layout with ldml: may not have"),
            "{field}: {err}"
        );
    }
    let err = error(&sme("ldml: {amiga: kb.xml}\n"));
    assert!(
        err.contains("ldml.amiga: amiga is neither a host nor default"),
        "{err}"
    );
    load_as(
        "sme",
        &sme("ldml: {default: kb.xml, iOS: ios.xml}\ndecimal: ','\nkeyNames: {space: x}\ntargets: {windows: {locale: se}}\n"),
    )
    .unwrap();
}

// [spec:kbdgen:def:ldml.yaml.verbatim/test]
#[test]
fn verbatim_fields_parse_strictly() {
    let yaml = sme(
        "variables: {strings: {s: x}, sets: {v: a b}, usets: {u: '[a-z]'}}\ntransforms:\n  - [{from: a, to: b}, {from: c}]\n  - reorder: [{from: '\\u{1A60}', order: 55, tertiaryBase: true}]\nbackspace: [[{from: x}]]\nkeys: {e-acute: {output: é, width: 1.5, longPress: a b, role: shift, gap: false}}\nflicks: {f: [{directions: n s, key: a}]}\ndisplays: [{output: a, display: A}, {keyId: e-acute, display: É}, {displayBase: x}]\n",
    );
    let layout = load_as("sme", &yaml).unwrap();
    assert_eq!(layout.variables.len(), 3);
    assert_eq!(layout.transforms.len(), 2);
    assert_eq!(layout.keys[0].width.as_deref(), Some("1.5"));
    assert_eq!(layout.keys[0].role, Some(Role::Shift));
    assert_eq!(
        layout.flicks[0].segments,
        [("n s".to_string(), "a".to_string())]
    );
    assert_eq!(layout.displays.len(), 2);
    assert_eq!(layout.display_base.as_deref(), Some("x"));
    for (body, expected) in [
        (
            "keys: {a: {colour: red}}\n",
            "keys.a.colour: unknown field colour",
        ),
        ("keys: {a: {role: hyper}}\n", "hyper is not a role"),
        ("transforms: [[]]\n", "at least one transform"),
        (
            "displays: [{output: a, keyId: b, display: c}]\n",
            "exactly one of output and keyId",
        ),
        (
            "displays: [{displayBase: a}, {displayBase: b}]\n",
            "displayBase occurs twice",
        ),
    ] {
        let err = error(&sme(body));
        assert!(err.contains(expected), "{body}: {err}");
    }
}

// [spec:kbdgen:def:ldml.yaml.long-press/test]
#[test]
fn long_press_candidates_are_tokens() {
    let layout = load_as(
        "sme",
        &sme("longPress: {a: 'á \\d{´} \\k{x}\n  \\u{E5}'}\n"),
    )
    .unwrap();
    assert_eq!(
        layout.long_press[0].candidates,
        [
            Token::Output(chars("á")),
            Token::Dead("´".to_string()),
            Token::KeyRef("x".to_string()),
            Token::Output(chars("å")),
        ]
    );
}

// [spec:kbdgen:def:ldml.yaml.emoji/test]
#[test]
fn emoji_reads_key_position_and_annotations() {
    let (dir, path) = write(
        "sme",
        &sme("emoji: {key: {position: B00, modifiers: ctrl shift}, annotations: ann.xml}\n"),
    );
    std::fs::write(
        dir.path().join("ann.xml"),
        "<?xml version=\"1.0\" encoding=\"UTF-8\" ?>\n<!DOCTYPE ldml SYSTEM \"../../common/dtd/ldml.dtd\">\n<ldml><identity><language type=\"se\"/></identity><annotations>\n<annotation cp=\"😀\">ilu | nirvu</annotation>\n<annotation cp=\"😀\" type=\"tts\">nirvvas ámadadju</annotation>\n<annotation cp=\"🐟\" type=\"tts\">guolli</annotation>\n</annotations></ldml>\n",
    )
    .unwrap();
    let layout = load(&path, "sme").unwrap();
    let key = layout.emoji.key.unwrap();
    assert_eq!(key.scan_code, 0x56);
    assert_eq!(
        kbd_ldml::encode_modifiers(&[ModifierSet::Set(key.modifiers)]),
        "ctrl shift"
    );
    let names: Vec<(&str, &str, Vec<&str>)> = layout
        .emoji
        .annotations
        .iter()
        .map(|a| {
            (
                a.emoji.as_str(),
                a.name.as_str(),
                a.keywords.iter().map(String::as_str).collect(),
            )
        })
        .collect();
    assert_eq!(
        names,
        [
            ("😀", "nirvvas ámadadju", vec!["ilu", "nirvu"]),
            ("🐟", "guolli", vec![]),
        ]
    );
    let err = error(&sme("emoji: {key: {position: Z99, modifiers: none}}\n"));
    assert!(
        err.contains("emoji.key.position: Z99 is not an ISO position"),
        "{err}"
    );
    let err = error(&sme("emoji: {annotations: missing.xml}\n"));
    assert!(err.contains("emoji.annotations: cannot read"), "{err}");
}

// [spec:kbdgen:req:ldml.yaml.strict/test]
#[test]
fn unused_dead_keys_warn_with_their_path() {
    let layout = load_as("sme", &sme("deadKeys: {´: {}}\n")).unwrap();
    let warnings: Vec<String> = layout.warnings.iter().map(ToString::to_string).collect();
    assert_eq!(warnings.len(), 1);
    assert!(
        warnings[0].ends_with(
            "sme.yaml: deadKeys.´: no \\d{´} token uses this dead key, so no host gets it"
        ),
        "{warnings:?}"
    );
}
