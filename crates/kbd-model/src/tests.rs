//! Keyboard-level tests: invariants, NFD, `context_len`, encoding and
//! compiled layouts, over one fixture that uses every table.

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

use crate::Component::*;
use crate::*;

/// Treats `é`, `Å` and the misordered marks U+0301 U+0323 as not NFD and
/// everything else as NFD: enough to tell checked texts from unchecked
/// ones without Unicode data.
struct ToyNfd;

impl NfdCheck for ToyNfd {
    fn is_nfd(&self, text: &str) -> bool {
        !text.contains(['\u{E9}', '\u{C5}']) && !text.contains("\u{301}\u{323}")
    }
}

const NFD: Option<&dyn NfdCheck> = Some(&ToyNfd);

fn text(s: &str) -> Text {
    Text::from(s)
}

fn marker_text(marker: MarkerIndex) -> Text {
    Text(vec![TextElem::Marker(marker)])
}

fn ch(c: char) -> TreeItem {
    TreeItem::one(TreeAtom::Char(c))
}

fn pattern(items: Vec<TreeItem>) -> Pattern {
    Pattern::from_tree(false, vec![items]).unwrap()
}

fn set_of(m: &[Component]) -> ModifierSet {
    ModifierSet::Set(Modifiers::of(m))
}

/// A valid keyboard with normalization enabled that fills every table.
fn fixture() -> Keyboard {
    let mut k = Keyboard::new(
        "vro",
        45,
        Info {
            name: "Võro".to_string(),
            ..Info::default()
        },
    );
    k.host = Some(Host::Windows);
    k.locales = vec!["vro-Latn".to_string()];
    k.version = Some("1.2.3-beta.1+build.5".to_string());
    k.markers = vec!["dk_00B4".to_string()];

    let mut a = Key::new("a", text("a"));
    a.long_press = vec![5];
    a.long_press_default = Some(5);
    a.flick = Some(0);
    let mut b = Key::new("b", text("b"));
    b.multi_tap = vec![0, 0];
    let mut space = Key::new("space", text(" "));
    space.stretch = true;
    let mut shift = Key::new("shift", Text::new());
    shift.role = Some(Role::Shift);
    shift.layer_id = Some("shift".to_string());
    shift.width = 1250;
    let mut to_base = Key::new("layer-base", Text::new());
    to_base.layer_id = Some("base".to_string());
    let mut backspace = Key::gap("backspace");
    backspace.role = Some(Role::Backspace);
    k.keys = vec![
        a,
        b,
        Key::gap("gap"),
        space,
        Key::new("dk-dk_00B4", marker_text(0)),
        Key::new("u-0061-0301", text("a\u{301}")),
        shift,
        to_base,
        backspace,
    ];
    k.flicks = vec![Flick {
        id: "flick-a".to_string(),
        segments: vec![FlickSegment {
            directions: vec![Direction::S, Direction::E],
            key: 1,
        }],
    }];
    k.displays = Displays {
        entries: vec![
            Display {
                target: DisplayTarget::Output(marker_text(0)),
                display: "´".to_string(),
            },
            Display {
                target: DisplayTarget::Key(3),
                display: "vaih".to_string(),
            },
            Display {
                target: DisplayTarget::Key(3),
                display: "space".to_string(),
            },
        ],
        display_base: Some("\u{25CC}".to_string()),
        labels: Labels {
            space: Some("space".to_string()),
            r#return: Some("sisse".to_string()),
        },
    };
    k.hardware = Some(Hardware {
        form: Form {
            id: "tiny".to_string(),
            rows: vec![vec![0x10, 0x11, 0x12], vec![0x39]],
        },
        min_device_width: None,
        layers: vec![
            HardwareLayer {
                id: Some("base".to_string()),
                modifiers: vec![set_of(&[]), set_of(&[Caps])],
                rows: vec![vec![0, 1, 4], vec![3]],
            },
            HardwareLayer {
                id: None,
                modifiers: vec![set_of(&[Shift]), set_of(&[Caps, Shift])],
                rows: vec![vec![1, 0]],
            },
            HardwareLayer {
                id: None,
                modifiers: vec![set_of(&[AltR])],
                rows: vec![vec![5]],
            },
            HardwareLayer {
                id: None,
                modifiers: vec![set_of(&[Ctrl]), set_of(&[Cmd])],
                rows: vec![vec![0, 1]],
            },
            HardwareLayer {
                id: None,
                modifiers: vec![set_of(&[Ctrl, Shift])],
                rows: vec![vec![1, 0]],
            },
            HardwareLayer {
                id: None,
                modifiers: vec![set_of(&[Extra1])],
                rows: vec![vec![5]],
            },
            HardwareLayer {
                id: None,
                modifiers: vec![ModifierSet::Other],
                rows: vec![],
            },
        ],
    });
    k.touch = vec![
        TouchSet {
            name: Some("phone".to_string()),
            bottom_row: BottomRow::Host,
            min_device_width: None,
            layers: vec![
                TouchLayer {
                    id: "base".to_string(),
                    rows: vec![vec![0, 1, 2, 4], vec![6, 8]],
                },
                TouchLayer {
                    id: "shift".to_string(),
                    rows: vec![vec![1, 0], vec![7]],
                },
            ],
            base: 0,
        },
        TouchSet {
            name: Some("tablet".to_string()),
            bottom_row: BottomRow::Authored,
            min_device_width: Some(95),
            layers: vec![
                TouchLayer {
                    id: "symbols-1".to_string(),
                    rows: vec![vec![1]],
                },
                TouchLayer {
                    id: "base".to_string(),
                    rows: vec![vec![0, 1, 3]],
                },
            ],
            base: 1,
        },
    ];
    k.sets = vec![
        vec![text("a"), text("o")],
        vec![text("a\u{301}"), text("o\u{301}")],
    ];
    k.classes = vec![Class {
        ranges: vec![ClassRange { lo: 'a', hi: 'z' }],
        negated: false,
        markers: vec![0],
    }];
    k.simple = vec![
        TransformGroup::Rules(vec![
            Rule {
                from: pattern(vec![
                    TreeItem::one(TreeAtom::Marker(0)),
                    TreeItem::one(TreeAtom::Capture(vec![vec![TreeItem::one(TreeAtom::Set(
                        0,
                    ))]])),
                ]),
                to: vec![ReplacementItem::MapSet {
                    group: 1,
                    from: 0,
                    to: 1,
                }],
            },
            Rule {
                from: pattern(vec![TreeItem::one(TreeAtom::Marker(0)), ch(' ')]),
                to: vec![ReplacementItem::Text(text("´"))],
            },
            Rule {
                from: pattern(vec![
                    TreeItem::one(TreeAtom::Class(0)),
                    TreeItem {
                        atom: TreeAtom::Fixed(Fixed::Digit),
                        min: 0,
                        max: 2,
                    },
                ]),
                to: vec![
                    ReplacementItem::Group(0),
                    ReplacementItem::Text(marker_text(0)),
                ],
            },
        ]),
        TransformGroup::Reorder(vec![
            ReorderRule {
                before: vec![],
                from: vec![ReorderClass::Char('\u{301}'), ReorderClass::Char('\u{323}')],
                order: vec![10, 11],
                tertiary: vec![0, 1],
                tertiary_base: vec![false, false],
                pre_base: vec![false, false],
            },
            ReorderRule {
                before: vec![ReorderClass::Ranges(vec![ClassRange { lo: 'a', hi: 'z' }])],
                from: vec![ReorderClass::Char('\u{301}')],
                order: vec![10],
                tertiary: vec![0],
                tertiary_base: vec![false],
                pre_base: vec![false],
            },
        ]),
    ];
    k.backspace = vec![TransformGroup::Rules(vec![Rule {
        from: pattern(vec![TreeItem::one(TreeAtom::AnyMarker)]),
        to: vec![],
    }])];
    k.context_len = 4;
    k.decimal = Some(text(","));
    k.flush = BTreeMap::from([(0, "´".to_string())]);
    k.dead_key_names = BTreeMap::from([(0, "ACUTE".to_string())]);
    k.windows = Windows {
        extra_modifiers: vec![ExtraModifierKey::RightCtrl],
        shift_lock: true,
        lrm_rlm: false,
        key_names: BTreeMap::from([("Space".to_string(), "Vaih".to_string())]),
    };
    k.emoji = Emoji {
        key: Some(EmojiKey {
            scan_code: 0x39,
            modifiers: Modifiers::of(&[Extra1]),
        }),
        annotations: vec![Annotation {
            emoji: "😀".to_string(),
            name: "irvitäs".to_string(),
            keywords: vec!["nalq".to_string(), "rõõm".to_string()],
        }],
    };
    k
}

/// The invariant that `change` makes the fixture violate.
fn violation(change: impl FnOnce(&mut Keyboard)) -> Invariant {
    let mut k = fixture();
    change(&mut k);
    k.validate(NFD)
        .expect_err("the change should violate an invariant")
        .invariant
}

fn hardware(k: &mut Keyboard) -> &mut Hardware {
    k.hardware.as_mut().unwrap()
}

fn first_rules(k: &mut Keyboard) -> &mut Vec<Rule> {
    match &mut k.simple[0] {
        TransformGroup::Rules(rules) => rules,
        TransformGroup::Reorder(_) => panic!("the fixture's first group has rules"),
    }
}

fn reorder_rules(k: &mut Keyboard) -> &mut Vec<ReorderRule> {
    match &mut k.simple[1] {
        TransformGroup::Reorder(rules) => rules,
        TransformGroup::Rules(_) => panic!("the fixture's second group reorders"),
    }
}

// [spec:kbdgen:def:ldml.model.keyboard+1/test]
// [spec:kbdgen:req:ldml.model.invariants+1/test]
#[test]
fn fixture_satisfies_every_invariant() {
    let k = fixture();
    assert_eq!(k.validate(NFD), Ok(()));
    assert_eq!(k.computed_context_len(), Ok(4));
    assert_eq!(k.key_index("space"), Some(3));
    assert_eq!(k.marker_index("dk_00B4"), Some(0));
    assert_eq!(k.marker_name(0), Some("dk_00B4"));
    assert_eq!(k.key(9), None);
}

// [spec:kbdgen:def:ldml.model.keys/test]
// [spec:kbdgen:req:ldml.model.invariants+1/test]
#[test]
fn key_table_invariants_are_enforced() {
    assert_eq!(
        violation(|k| k.keys[1].id = "a".to_string()),
        Invariant::DuplicateKeyId
    );
    assert_eq!(
        violation(|k| k.keys[1].id = "a b".to_string()),
        Invariant::KeyId
    );
    assert_eq!(
        violation(|k| k.keys[0].long_press = vec![9]),
        Invariant::IndexRange
    );
    assert_eq!(
        violation(|k| k.keys[0].long_press_default = Some(1)),
        Invariant::LongPressDefault
    );
    assert_eq!(
        violation(|k| k.keys[1].multi_tap = vec![1]),
        Invariant::MultiTapSelf
    );
    assert_eq!(
        violation(|k| k.keys[1].flick = Some(1)),
        Invariant::IndexRange
    );
    assert_eq!(
        violation(|k| k.keys[0].output = marker_text(1)),
        Invariant::IndexRange
    );
    // A key may list itself in `long_press`; only `multi_tap` forbids it.
    let mut k = fixture();
    k.keys[0].long_press = vec![0];
    k.keys[0].long_press_default = Some(0);
    assert_eq!(k.validate(NFD), Ok(()));
}

// [spec:kbdgen:def:ldml.model.keys/test]
// [spec:kbdgen:req:ldml.model.invariants+1/test]
#[test]
fn gap_keys_have_no_output_or_gestures() {
    assert_eq!(
        violation(|k| k.keys[2].output = text("x")),
        Invariant::GapKey
    );
    assert_eq!(
        violation(|k| k.keys[2].layer_id = Some("base".into())),
        Invariant::GapKey
    );
    assert_eq!(
        violation(|k| k.keys[2].long_press = vec![0]),
        Invariant::GapKey
    );
    assert_eq!(
        violation(|k| k.keys[2].multi_tap = vec![0]),
        Invariant::GapKey
    );
    assert_eq!(violation(|k| k.keys[2].flick = Some(0)), Invariant::GapKey);
    let display = |k: &mut Keyboard| {
        k.displays.entries.push(Display {
            target: DisplayTarget::Key(2),
            display: "_".into(),
        })
    };
    assert_eq!(violation(display), Invariant::GapKey);
    // A gap may have a role, a width and stretch.
    let mut k = fixture();
    k.keys[2].width = 250;
    k.keys[2].stretch = true;
    k.keys[2].role = Some(Role::Tab);
    assert_eq!(k.validate(NFD), Ok(()));
}

#[test]
fn flick_and_marker_tables_are_checked() {
    assert_eq!(
        violation(|k| k.flicks[0].segments[0].key = 99),
        Invariant::IndexRange
    );
    assert_eq!(
        violation(|k| k.flicks[0].segments[0].directions.clear()),
        Invariant::FlickDirections
    );
    let dup = |k: &mut Keyboard| {
        let f = k.flicks[0].clone();
        k.flicks.push(f);
    };
    assert_eq!(violation(dup), Invariant::DuplicateFlickId);
    assert_eq!(
        violation(|k| k.flicks[0].id = String::new()),
        Invariant::FlickId
    );
    assert_eq!(
        violation(|k| k.markers.push("dk_00B4".into())),
        Invariant::DuplicateMarker
    );
    assert_eq!(
        violation(|k| k.markers[0] = "a}".into()),
        Invariant::MarkerName
    );
}

// [spec:kbdgen:def:ldml.model.displays+1/test]
#[test]
fn displays_last_entry_wins() {
    let k = fixture();
    assert_eq!(k.displays.for_key(3), Some("space"));
    assert_eq!(k.displays.for_output(&marker_text(0)), Some("´"));
    assert_eq!(k.displays.for_output(&text("q")), None);
    assert_eq!(
        violation(|k| k.displays.entries[1].target = DisplayTarget::Key(40)),
        Invariant::IndexRange
    );
    assert_eq!(
        violation(|k| k.displays.entries[0].target = DisplayTarget::Output(marker_text(3))),
        Invariant::IndexRange
    );
    // Display strings are never normalized, so they need not be NFD.
    let mut k = fixture();
    k.displays.entries[0].display = "\u{E9}".into();
    k.displays.display_base = Some("\u{C5}".into());
    assert_eq!(k.validate(NFD), Ok(()));
}

// [spec:kbdgen:def:ldml.model.hardware+1/test]
#[test]
fn hardware_positions_follow_the_form() {
    let k = fixture();
    let hw = k.hardware.as_ref().unwrap();
    assert_eq!(hw.form.position_of(0x39), Some((1, 0)));
    assert_eq!(hw.key_for_scan_code(0, 0x12), Some(4));
    assert_eq!(hw.key_for_scan_code(1, 0x12), None, "past the row's end");
    assert_eq!(hw.key_for_scan_code(1, 0x39), None, "row omitted");
    assert_eq!(hw.key_for_scan_code(0, 0x50), None, "not in the form");
    assert_eq!(hw.touch_base(), None, "base also has caps");

    assert_eq!(
        violation(|k| hardware(k).layers[0].rows[0].push(1)),
        Invariant::RowsExceedForm
    );
    assert_eq!(
        violation(|k| hardware(k).layers[0].rows.push(vec![0])),
        Invariant::RowsExceedForm
    );
    assert_eq!(
        violation(|k| hardware(k).layers[1].rows[0][0] = 77),
        Invariant::IndexRange
    );
    assert_eq!(
        violation(|k| hardware(k).form.rows[1].push(0x10)),
        Invariant::FormScanCodes
    );
    assert_eq!(
        violation(|k| hardware(k).layers[1].modifiers.clear()),
        Invariant::LayerModifiers
    );
}

// [spec:kbdgen:def:ldml.model.modifiers+1/test]
// [spec:kbdgen:req:ldml.model.invariants+1/test]
#[test]
fn layer_modifier_sets_are_canonical() {
    assert_eq!(
        violation(|k| hardware(k).layers[1].modifiers.reverse()),
        Invariant::ModifierOrder
    );
    assert_eq!(
        violation(|k| hardware(k).layers[1].modifiers.push(set_of(&[Shift]))),
        Invariant::ModifierOrder
    );
    assert_eq!(
        violation(|k| hardware(k).layers[2].modifiers = vec![set_of(&[AltL, AltR])]),
        Invariant::ModifierCombination
    );
    assert_eq!(
        violation(|k| hardware(k).layers[2].modifiers = vec![set_of(&[Alt, AltR])]),
        Invariant::ModifierCombination
    );
    let bits = |k: &mut Keyboard| {
        let encoded = postcard::to_allocvec(&0x8000u16).unwrap();
        let m: Modifiers = postcard::from_bytes(&encoded).unwrap();
        hardware(k).layers[2].modifiers = vec![ModifierSet::Set(m)];
    };
    assert_eq!(violation(bits), Invariant::ModifierBits);
}

// [spec:kbdgen:def:ldml.model.native/test]
// [spec:kbdgen:req:ldml.model.invariants+1/test]
#[test]
fn only_non_native_layers_may_not_overlap() {
    // `alt` overlaps the `altR` layer.
    assert_eq!(
        violation(|k| hardware(k).layers[5].modifiers = vec![set_of(&[Alt])]),
        Invariant::OverlappingLayers
    );
    // A second `shift` layer.
    assert_eq!(
        violation(|k| hardware(k).layers[5].modifiers = vec![set_of(&[Shift])]),
        Invariant::OverlappingLayers
    );
    // Native layers overlap each other and typing layers freely: layer 3
    // is native through `cmd`, so its `shift` set is never selected.
    let mut k = fixture();
    hardware(&mut k).layers[4].modifiers = vec![set_of(&[Ctrl])];
    hardware(&mut k).layers[3].modifiers = vec![set_of(&[Shift]), set_of(&[Cmd])];
    assert_eq!(k.validate(NFD), Ok(()));
    let hw = k.hardware.as_ref().unwrap();
    assert!(hw.layers[3].is_native());
    assert!(hw.layers[4].is_native());
    assert!(!hw.layers[2].is_native());
    assert!(!hw.layers[6].is_native());
}

// [spec:kbdgen:req:ldml.model.invariants+1/test]
#[test]
fn at_most_one_layer_has_other() {
    let second = |k: &mut Keyboard| hardware(k).layers[5].modifiers = vec![ModifierSet::Other];
    assert_eq!(violation(second), Invariant::MultipleOther);
}

// [spec:kbdgen:def:ldml.model.windows/test]
// [spec:kbdgen:req:ldml.model.invariants+1/test]
#[test]
fn extra_components_need_their_binding() {
    assert_eq!(
        violation(|k| k.windows.extra_modifiers.clear()),
        Invariant::ExtraModifierUnbound
    );
    let emoji_extra2 = |k: &mut Keyboard| {
        k.emoji.key = Some(EmojiKey {
            scan_code: 0x39,
            modifiers: Modifiers::of(&[Extra2]),
        })
    };
    assert_eq!(violation(emoji_extra2), Invariant::ExtraModifierUnbound);
    let mut k = fixture();
    k.windows.extra_modifiers.push(ExtraModifierKey::B00);
    k.emoji.key = Some(EmojiKey {
        scan_code: 0x39,
        modifiers: Modifiers::of(&[Extra2, Shift]),
    });
    assert_eq!(k.validate(NFD), Ok(()));
}

// [spec:kbdgen:def:ldml.model.windows/test]
#[test]
fn windows_options_are_checked() {
    let repeat = |k: &mut Keyboard| k.windows.extra_modifiers.push(ExtraModifierKey::RightCtrl);
    assert_eq!(violation(repeat), Invariant::ExtraModifierKeys);
    let four = |k: &mut Keyboard| {
        k.windows.extra_modifiers = vec![
            ExtraModifierKey::RightCtrl,
            ExtraModifierKey::CapsLock,
            ExtraModifierKey::B00,
            ExtraModifierKey::B00,
        ]
    };
    assert_eq!(violation(four), Invariant::ExtraModifierKeys);
    let unknown = |k: &mut Keyboard| {
        k.windows.key_names.insert("Hyper".into(), "x".into());
    };
    assert_eq!(violation(unknown), Invariant::KeyName);
    for name in ["Caps Lock", "Right Alt", "<00>", "F24"] {
        let mut k = fixture();
        k.windows.key_names.insert(name.into(), "x".into());
        assert_eq!(k.validate(NFD), Ok(()), "{name}");
    }
    assert_eq!(ExtraModifierKey::B00.name(), "B00");
}

// [spec:kbdgen:def:ldml.model.touch+1/test]
// [spec:kbdgen:req:ldml.model.invariants+1/test]
#[test]
fn touch_sets_order_widths_and_bases() {
    let k = fixture();
    assert_eq!(k.touch[1].layer_index("base"), Some(1));
    assert_eq!(violation(|k| k.touch.reverse()), Invariant::TouchWidth);
    assert_eq!(
        violation(|k| k.touch[1].min_device_width = None),
        Invariant::TouchWidth
    );
    assert_eq!(
        violation(|k| k.touch[1].min_device_width = Some(1000)),
        Invariant::TouchWidth
    );
    assert_eq!(
        violation(|k| k.touch[1].min_device_width = Some(0)),
        Invariant::TouchWidth
    );
    let same = |k: &mut Keyboard| {
        let mut third = k.touch[1].clone();
        third.name = Some("tablet-large".into());
        k.touch.push(third);
    };
    assert_eq!(violation(same), Invariant::TouchWidth);
    assert_eq!(violation(|k| k.touch[1].base = 0), Invariant::TouchBase);
    assert_eq!(violation(|k| k.touch[1].base = 7), Invariant::TouchBase);
    assert_eq!(
        violation(|k| k.touch[0].layers[1].id = "base".into()),
        Invariant::DuplicateTouchLayerId
    );
    assert_eq!(
        violation(|k| k.touch[0].layers[0].rows[0][0] = 50),
        Invariant::IndexRange
    );

    let mut k = fixture();
    k.touch.push(TouchSet {
        name: None,
        bottom_row: BottomRow::Authored,
        min_device_width: Some(190),
        layers: vec![TouchLayer {
            id: "base".into(),
            rows: vec![],
        }],
        base: 0,
    });
    assert_eq!(k.validate(NFD), Ok(()));
}

// [spec:kbdgen:def:ldml.model.touch+1/test]
// [spec:kbdgen:req:ldml.model.invariants+1/test]
#[test]
fn layer_ids_name_touch_layers() {
    assert_eq!(
        violation(|k| k.keys[6].layer_id = Some("nope".into())),
        Invariant::LayerId
    );
    // `symbols-1` exists only in the tablet set; that is enough.
    let mut k = fixture();
    k.keys[7].layer_id = Some("symbols-1".into());
    assert_eq!(k.validate(NFD), Ok(()));

    // Without touch sets the hardware set is presented as touch, with the
    // `none` layer as its base and its layer ids as switch targets.
    let mut k = fixture();
    k.touch.clear();
    assert_eq!(k.validate(NFD).unwrap_err().invariant, Invariant::LayerId);
    hardware(&mut k).layers[1].id = Some("shift".into());
    hardware(&mut k).layers[0].modifiers = vec![set_of(&[])];
    hardware(&mut k).layers[5].modifiers = vec![set_of(&[Caps])];
    assert_eq!(k.validate(NFD), Ok(()));
    assert_eq!(k.hardware.as_ref().unwrap().touch_base(), Some(0));
}

// [spec:kbdgen:def:ldml.model.transforms+1/test]
// [spec:kbdgen:req:ldml.model.invariants+1/test]
#[test]
fn rules_groups_and_patterns_are_checked() {
    assert_eq!(violation(|k| first_rules(k).clear()), Invariant::EmptyRules);
    let optional = |k: &mut Keyboard| {
        first_rules(k)[1].from = pattern(vec![TreeItem {
            atom: TreeAtom::Char('x'),
            min: 0,
            max: 3,
        }])
    };
    assert_eq!(violation(optional), Invariant::EmptyMatch);
    let anchor_only =
        |k: &mut Keyboard| first_rules(k)[1].from = Pattern::from_tree(true, vec![vec![]]).unwrap();
    assert_eq!(violation(anchor_only), Invariant::EmptyMatch);
    let empty_alt = |k: &mut Keyboard| {
        first_rules(k)[1].from = Pattern::from_tree(false, vec![vec![ch('x')], vec![]]).unwrap()
    };
    assert_eq!(violation(empty_alt), Invariant::EmptyMatch);
    let bad_class = |k: &mut Keyboard| {
        first_rules(k)[1].from = pattern(vec![TreeItem::one(TreeAtom::Class(4))])
    };
    assert_eq!(violation(bad_class), Invariant::IndexRange);
    let bad_marker = |k: &mut Keyboard| {
        first_rules(k)[1].from = pattern(vec![TreeItem::one(TreeAtom::Marker(4))])
    };
    assert_eq!(violation(bad_marker), Invariant::IndexRange);
    let in_backspace = |k: &mut Keyboard| k.backspace = vec![TransformGroup::Rules(vec![])];
    assert_eq!(violation(in_backspace), Invariant::EmptyRules);
}

// [spec:kbdgen:def:ldml.model.pattern+1/test]
// [spec:kbdgen:req:ldml.model.invariants+1/test]
#[test]
fn replacements_reference_existing_groups_and_sets() {
    assert_eq!(
        violation(|k| first_rules(k)[1].to = vec![ReplacementItem::Group(1)]),
        Invariant::ReplacementGroup
    );
    assert_eq!(
        violation(|k| first_rules(k)[0].to = vec![ReplacementItem::Group(2)]),
        Invariant::ReplacementGroup
    );
    let mapset = |group, from, to| {
        move |k: &mut Keyboard| {
            first_rules(k)[0].to = vec![ReplacementItem::MapSet { group, from, to }]
        }
    };
    assert_eq!(violation(mapset(2, 0, 1)), Invariant::ReplacementGroup);
    assert_eq!(violation(mapset(0, 0, 1)), Invariant::ReplacementGroup);
    assert_eq!(
        violation(mapset(1, 1, 0)),
        Invariant::MapSet,
        "from is not the captured set"
    );
    assert_eq!(violation(mapset(1, 0, 5)), Invariant::MapSet);
    let shorter = |k: &mut Keyboard| {
        k.sets[1].pop();
    };
    assert_eq!(violation(shorter), Invariant::MapSet);
    let two_items = |k: &mut Keyboard| {
        first_rules(k)[0].from = pattern(vec![
            TreeItem::one(TreeAtom::Marker(0)),
            TreeItem::one(TreeAtom::Capture(vec![vec![
                TreeItem::one(TreeAtom::Set(0)),
                ch('x'),
            ]])),
        ])
    };
    assert_eq!(violation(two_items), Invariant::MapSet);
    let repeated = |k: &mut Keyboard| {
        first_rules(k)[0].from = pattern(vec![TreeItem::one(TreeAtom::Capture(vec![vec![
            TreeItem {
                atom: TreeAtom::Set(0),
                min: 1,
                max: 2,
            },
        ]]))])
    };
    assert_eq!(violation(repeated), Invariant::MapSet);
    assert_eq!(
        violation(|k| k.sets[0][0] = marker_text(2)),
        Invariant::IndexRange
    );
    assert_eq!(
        violation(|k| k.classes[0].markers = vec![3]),
        Invariant::IndexRange
    );
    assert_eq!(
        violation(|k| k.classes[0].ranges = vec![ClassRange { lo: 'z', hi: 'a' }]),
        Invariant::ClassRange
    );
}

// [spec:kbdgen:def:ldml.model.transforms+1/test]
#[test]
fn reorder_rules_are_padded_and_sorted() {
    let unpadded = |k: &mut Keyboard| {
        reorder_rules(k)[0].order.pop();
    };
    assert_eq!(violation(unpadded), Invariant::ReorderShape);
    assert_eq!(
        violation(|k| reorder_rules(k)[1].pre_base.push(true)),
        Invariant::ReorderShape
    );
    let empty = |k: &mut Keyboard| {
        let r = &mut reorder_rules(k)[1];
        r.from.clear();
        r.order.clear();
        r.tertiary.clear();
        r.tertiary_base.clear();
        r.pre_base.clear();
    };
    assert_eq!(violation(empty), Invariant::ReorderShape);
    assert_eq!(
        violation(|k| reorder_rules(k).reverse()),
        Invariant::ReorderPriority
    );
    let reversed_range = |k: &mut Keyboard| {
        reorder_rules(k)[1].before =
            vec![ReorderClass::Ranges(vec![ClassRange { lo: 'z', hi: 'a' }])]
    };
    assert_eq!(violation(reversed_range), Invariant::ClassRange);
    // Equal `from` lengths sort by `before` length.
    let mut k = fixture();
    let r = &mut reorder_rules(&mut k)[0];
    r.from.truncate(1);
    r.order.truncate(1);
    r.tertiary.truncate(1);
    r.tertiary_base.truncate(1);
    r.pre_base.truncate(1);
    assert_eq!(
        k.validate(NFD).unwrap_err().invariant,
        Invariant::ReorderPriority
    );
    reorder_rules(&mut k).swap(0, 1);
    assert_eq!(k.validate(NFD), Ok(()));
    assert!(ReorderClass::Ranges(vec![ClassRange::single('q')]).contains('q'));
}

// [spec:kbdgen:req:ldml.model.context-len+1/test]
#[test]
fn reorder_rules_count_toward_context_len() {
    let mut k = fixture();
    assert_eq!(k.computed_context_len(), Ok(4));
    k.simple.push(TransformGroup::Reorder(vec![ReorderRule {
        before: vec![ReorderClass::Char('a'); 3],
        from: vec![ReorderClass::Char('b'); 4],
        order: vec![1; 4],
        tertiary: vec![0; 4],
        tertiary_base: vec![false; 4],
        pre_base: vec![false; 4],
    }]));
    assert_eq!(k.computed_context_len(), Ok(8), "before and from together");
}

// [spec:kbdgen:req:ldml.model.context-len+1/test]
#[test]
fn context_len_is_longest_match_plus_one() {
    assert_eq!(violation(|k| k.context_len = 3), Invariant::ContextLen);
    assert_eq!(violation(|k| k.context_len = 5), Invariant::ContextLen);

    // `.{9}` repeated seven times spans 63 elements: context_len 64.
    let dots = |n: usize| {
        pattern(
            (0..n)
                .map(|_| TreeItem {
                    atom: TreeAtom::Any,
                    min: 9,
                    max: 9,
                })
                .collect(),
        )
    };
    let mut k = fixture();
    first_rules(&mut k)[1].from = dots(7);
    k.context_len = 64;
    assert_eq!(k.computed_context_len(), Ok(64));
    assert_eq!(k.validate(NFD), Ok(()));

    first_rules(&mut k)[1].from = dots(8);
    assert_eq!(k.computed_context_len(), Ok(73));
    assert_eq!(
        k.validate(NFD).unwrap_err().invariant,
        Invariant::ContextTooLong
    );

    // Patterns in `backspace` count, and an empty reorder group adds
    // nothing.
    let mut k = fixture();
    k.backspace = vec![TransformGroup::Rules(vec![Rule {
        from: Pattern::from_tree(
            false,
            vec![vec![TreeItem::one(TreeAtom::Group(vec![
                vec![ch('a'), ch('b'), ch('c')],
                vec![TreeItem::one(TreeAtom::Set(1))],
            ]))]],
        )
        .unwrap(),
        to: vec![],
    }])];
    assert_eq!(k.computed_context_len(), Ok(4));
    k.simple.truncate(1);
    k.simple[0] = TransformGroup::Reorder(vec![]);
    assert_eq!(k.computed_context_len(), Ok(4));
    k.backspace.clear();
    assert_eq!(k.computed_context_len(), Ok(1));
    k.context_len = 1;
    assert_eq!(k.validate(NFD), Ok(()));
}

// [spec:kbdgen:req:ldml.model.nfd+1/test]
#[test]
fn enabled_normalization_requires_nfd_texts() {
    let nfd = |change: fn(&mut Keyboard)| violation(change);
    assert_eq!(
        nfd(|k| k.keys[0].output = text("\u{E9}")),
        Invariant::NotNfd
    );
    assert_eq!(
        nfd(|k| first_rules(k)[1].from = pattern(vec![ch('e'), ch('\u{301}'), ch('\u{323}')])),
        Invariant::NotNfd
    );
    assert_eq!(
        nfd(|k| first_rules(k)[1].from = pattern(vec![ch('\u{C5}')])),
        Invariant::NotNfd
    );
    assert_eq!(nfd(|k| k.sets[1][0] = text("\u{E9}")), Invariant::NotNfd);
    assert_eq!(
        nfd(|k| first_rules(k)[1].to = vec![ReplacementItem::Text(text("\u{C5}"))]),
        Invariant::NotNfd
    );
    assert_eq!(
        nfd(|k| k.displays.entries[0].target = DisplayTarget::Output(text("\u{E9}"))),
        Invariant::NotNfd
    );
    assert_eq!(
        nfd(|k| k.classes[0].ranges[0].hi = '\u{FF}'),
        Invariant::NotNfd
    );
    assert_eq!(
        nfd(|k| reorder_rules(k)[0].from[0] = ReorderClass::Char('\u{E9}')),
        Invariant::NotNfd
    );
    assert_eq!(
        nfd(
            |k| reorder_rules(k)[1].before = vec![ReorderClass::Ranges(vec![ClassRange {
                lo: 'a',
                hi: '\u{FF}'
            }])]
        ),
        Invariant::NotNfd
    );
    assert_eq!(nfd(|k| k.decimal = Some(text("\u{C5}"))), Invariant::NotNfd);
    assert_eq!(
        nfd(|k| {
            k.flush.insert(0, "\u{E9}".into());
        }),
        Invariant::NotNfd
    );
    // Markers are out of band: a marker between the marks still leaves a
    // misordered plain text.
    let split = |k: &mut Keyboard| {
        k.keys[0].output = Text(vec![
            TextElem::Char('e'),
            TextElem::Char('\u{301}'),
            TextElem::Marker(0),
            TextElem::Char('\u{323}'),
        ])
    };
    assert_eq!(violation(split), Invariant::NotNfd);
    // Separately quantified marks are separate runs.
    let quantified = |k: &mut Keyboard| {
        first_rules(k)[1].from = pattern(vec![
            ch('\u{301}'),
            TreeItem {
                atom: TreeAtom::Char('\u{323}'),
                min: 1,
                max: 2,
            },
        ])
    };
    let mut k = fixture();
    quantified(&mut k);
    assert_eq!(k.validate(NFD), Ok(()));
}

// [spec:kbdgen:req:ldml.model.nfd+1/test]
#[test]
fn disabled_normalization_keeps_authored_texts() {
    let mut k = fixture();
    k.normalization = Normalization::Disabled;
    k.keys[0].output = text("\u{E9}");
    k.classes[0].ranges[0].hi = '\u{FF}';
    k.flush.insert(0, "\u{C5}".into());
    assert_eq!(k.validate(NFD), Ok(()));
    assert_eq!(k.validate(None), Ok(()));

    let enabled = fixture();
    assert_eq!(
        enabled.validate(None).unwrap_err().invariant,
        Invariant::NfdUnchecked
    );
}

#[test]
fn header_fields_are_checked() {
    assert_eq!(violation(|k| k.conforms_to = 44), Invariant::ConformsTo);
    assert_eq!(violation(|k| k.conforms_to = 50), Invariant::ConformsTo);
    for bad in [
        "1.2",
        "01.2.3",
        "1.2.3-",
        "1.2.3-01",
        "1.2.3+",
        "v1.2.3",
        "1.2.3-a..b",
    ] {
        assert_eq!(
            violation(|k| k.version = Some(bad.into())),
            Invariant::Version,
            "{bad}"
        );
    }
    for good in [
        "0.0.0",
        "10.20.30",
        "1.0.0-alpha-1.0",
        "1.0.0+20130313144700",
    ] {
        let mut k = fixture();
        k.version = Some(good.into());
        assert_eq!(k.validate(NFD), Ok(()), "{good}");
    }
}

#[test]
fn extension_maps_reference_markers() {
    assert_eq!(
        violation(|k| {
            k.flush.insert(1, "x".into());
        }),
        Invariant::IndexRange
    );
    assert_eq!(
        violation(|k| {
            k.dead_key_names.insert(4, "x".into());
        }),
        Invariant::IndexRange
    );
    assert_eq!(
        violation(|k| k.decimal = Some(marker_text(1))),
        Invariant::IndexRange
    );
}

// [spec:kbdgen:def:ldml.model.emoji/test]
#[test]
fn emoji_key_is_the_preserved_key() {
    let k = fixture();
    assert_eq!(
        k.preserved_key(),
        Some(EmojiKey {
            scan_code: 0x39,
            modifiers: Modifiers::of(&[Extra1]),
        })
    );
    assert_eq!(k.emoji.annotations[0].keywords, ["nalq", "rõõm"]);
    let rejected = |k: &mut Keyboard| {
        k.emoji.key = Some(EmojiKey {
            scan_code: 0x39,
            modifiers: Modifiers::of(&[CtrlL, CtrlR]),
        })
    };
    assert_eq!(violation(rejected), Invariant::ModifierCombination);
}

// [spec:kbdgen:def:ldml.model.flush+1/test]
#[test]
fn flush_and_dead_key_names_survive_encoding() {
    let k = fixture();
    let back = Keyboard::from_bytes(&k.to_bytes().unwrap(), NFD).unwrap();
    assert_eq!(back.flush.get(&0).map(String::as_str), Some("´"));
    assert_eq!(
        back.dead_key_names.get(&0).map(String::as_str),
        Some("ACUTE")
    );
    let foreign = Keyboard {
        flush: BTreeMap::new(),
        dead_key_names: BTreeMap::new(),
        ..k
    };
    assert_eq!(foreign.validate(NFD), Ok(()));
}

// [spec:kbdgen:syn:ldml.model.encoding+1/test]
#[test]
fn encoding_starts_with_magic_and_version() {
    let bytes = fixture().to_bytes().unwrap();
    assert_eq!(&bytes[..4], b"DVKB");
    assert_eq!(&bytes[4..8], &[1, 0, 0, 0]);
    let (version, body) = read_header(&bytes).unwrap();
    assert_eq!(version, Version { major: 1, minor: 0 });
    assert_eq!(body, postcard::to_allocvec(&fixture()).unwrap().as_slice());
}

// [spec:kbdgen:syn:ldml.model.encoding+1/test]
// [spec:kbdgen:req:ldml.model.decode/test]
#[test]
fn decode_round_trips_the_fixture() {
    let k = fixture();
    let bytes = k.to_bytes().unwrap();
    assert_eq!(Keyboard::from_bytes(&bytes, NFD), Ok(k.clone()));

    let mut disabled = k;
    disabled.normalization = Normalization::Disabled;
    let bytes = disabled.to_bytes().unwrap();
    assert_eq!(Keyboard::from_bytes(&bytes, None), Ok(disabled));
}

// [spec:kbdgen:req:ldml.model.deterministic+1/test]
#[test]
fn equal_models_encode_to_identical_bytes() {
    let a = fixture().to_bytes().unwrap();
    let b = fixture().to_bytes().unwrap();
    assert_eq!(a, b);
    let decoded = Keyboard::from_bytes(&a, NFD).unwrap();
    assert_eq!(decoded.to_bytes().unwrap(), a);

    // Maps encode in key order, whatever the insertion order.
    let mut x = fixture();
    let mut y = fixture();
    for (name, value) in [("Tab", "Tabulaator"), ("Esc", "Paoq"), ("Space", "Vaih")] {
        x.windows.key_names.insert(name.into(), value.into());
    }
    for (name, value) in [("Esc", "Paoq"), ("Space", "Vaih"), ("Tab", "Tabulaator")] {
        y.windows.key_names.insert(name.into(), value.into());
    }
    assert_eq!(x.to_bytes().unwrap(), y.to_bytes().unwrap());
}

// [spec:kbdgen:req:ldml.model.deterministic+1/test]
#[test]
fn encoding_matches_a_pinned_byte_sequence() {
    // Pinned so that any platform, or a change of field order, that
    // encodes differently fails here.
    let mut k = Keyboard::new(
        "se",
        45,
        Info {
            name: "S".into(),
            ..Info::default()
        },
    );
    k.normalization = Normalization::Disabled;
    k.keys = vec![Key::new("a", text("a"))];
    let bytes = k.to_bytes().unwrap();
    let expected: &[u8] = &[
        b'D', b'V', b'K', b'B', 1, 0, 0, 0, // header
        0, // host: none
        2, b's', b'e', // locale
        0,    // locales
        45,   // conforms_to
        0,    // version: none
        1, b'S', 0, 0, 0, 0, // info
        1, // normalization: Disabled
        0, // markers
        1, // keys
        1, b'a', // id
        1, 0, 1, b'a', // output: Char('a')
        0,    // gap
        0,    // layer_id
        0xE8, 0x07, // width 1000
        0, 0, 0, 0, 0, 0, // stretch, long_press, default, multi_tap, flick, role
        0, // flicks
        0, 0, 0, 0, // displays: entries, base, labels
        0, // hardware
        0, 0, 0, 0, 0, // touch, sets, classes, simple, backspace
        1, // context_len
        0, 0, 0, // decimal, flush, dead_key_names
        0, 0, 0, 0, // windows
        0, 0, // emoji
    ];
    assert_eq!(bytes, expected);
    assert_eq!(Keyboard::from_bytes(&bytes, None), Ok(k));
}

// [spec:kbdgen:req:ldml.model.decode/test]
#[test]
fn decode_rejects_bad_magic_and_major() {
    let bytes = fixture().to_bytes().unwrap();
    let mut magic = bytes.clone();
    magic[0] = b'X';
    assert_eq!(Keyboard::from_bytes(&magic, NFD), Err(DecodeError::Magic));
    for major in [0u16, 2, 0xFFFF] {
        let mut v = bytes.clone();
        v[4..6].copy_from_slice(&major.to_le_bytes());
        assert_eq!(
            Keyboard::from_bytes(&v, NFD),
            Err(DecodeError::UnsupportedVersion(Version { major, minor: 0 }))
        );
    }
    for len in 0..HEADER_LEN {
        assert_eq!(
            Keyboard::from_bytes(&bytes[..len], NFD),
            Err(DecodeError::Truncated)
        );
    }
}

// [spec:kbdgen:req:ldml.model.decode/test]
#[test]
fn decode_accepts_higher_minor_and_trailing_bytes() {
    let k = fixture();
    let mut bytes = k.to_bytes().unwrap();
    bytes[6..8].copy_from_slice(&7u16.to_le_bytes());
    bytes.extend_from_slice(&[0xFF, 0x00, 0x42]);
    assert_eq!(Keyboard::from_bytes(&bytes, NFD), Ok(k));
}

// [spec:kbdgen:req:ldml.model.decode/test]
#[test]
fn decode_rejects_every_truncation_without_panicking() {
    let bytes = fixture().to_bytes().unwrap();
    for len in HEADER_LEN..bytes.len() {
        match Keyboard::from_bytes(&bytes[..len], NFD) {
            Err(DecodeError::Postcard(_)) => {}
            other => panic!("prefix of {len} bytes gave {other:?}"),
        }
    }
}

// [spec:kbdgen:req:ldml.model.decode/test]
#[test]
fn decode_survives_corruption_of_every_byte() {
    let bytes = fixture().to_bytes().unwrap();
    for at in HEADER_LEN..bytes.len() {
        for value in [0x00, 0x01, 0x7F, 0x80, 0xFF] {
            let mut v = bytes.clone();
            v[at] = value;
            // Either outcome is acceptable; panicking or aborting is not,
            // and anything accepted must satisfy the invariants.
            if let Ok(k) = Keyboard::from_bytes(&v, NFD) {
                assert_eq!(k.validate(NFD), Ok(()));
            }
        }
    }
}

// [spec:kbdgen:req:ldml.model.decode/test]
#[test]
fn decode_rejects_lengths_beyond_the_input() {
    let header = fixture().to_bytes().unwrap()[..HEADER_LEN].to_vec();
    // host: none, then a locale claiming 2^35 - 1 bytes.
    let mut long_string = header.clone();
    long_string.extend_from_slice(&[0, 0xFF, 0xFF, 0xFF, 0xFF, 0x7F]);
    assert!(matches!(
        Keyboard::from_bytes(&long_string, None),
        Err(DecodeError::Postcard(_))
    ));
    // host: none, locale "se", then `locales` claiming 2^32 - 1 entries,
    // which would need gigabytes if preallocated.
    let mut long_list = header.clone();
    long_list.extend_from_slice(&[0, 2, b's', b'e', 0xFF, 0xFF, 0xFF, 0xFF, 0x0F, 0]);
    assert_eq!(
        Keyboard::from_bytes(&long_list, None),
        Err(DecodeError::Postcard(
            postcard::Error::DeserializeUnexpectedEnd
        ))
    );
    // An invalid variant and invalid UTF-8.
    let mut variant = header.clone();
    variant.extend_from_slice(&[1, 9]);
    assert!(matches!(
        Keyboard::from_bytes(&variant, None),
        Err(DecodeError::Postcard(_))
    ));
    let mut utf8 = header;
    utf8.extend_from_slice(&[0, 1, 0xFF]);
    assert!(matches!(
        Keyboard::from_bytes(&utf8, None),
        Err(DecodeError::Postcard(_))
    ));
}

// [spec:kbdgen:req:ldml.model.decode/test]
// [spec:kbdgen:req:ldml.model.invariants+1/test]
#[test]
fn decode_rejects_a_violated_invariant() {
    let mut k = fixture();
    k.keys[1].multi_tap = vec![1];
    let err = Keyboard::from_bytes(&k.to_bytes().unwrap(), NFD).unwrap_err();
    assert_eq!(
        err,
        DecodeError::Invariant(InvariantError {
            invariant: Invariant::MultiTapSelf,
            site: Site::Key(1),
        })
    );
    assert_eq!(
        err.to_string(),
        "keyboard model is invalid: key lists itself in multi_tap at keys[1]"
    );
    let mut k = fixture();
    k.context_len = 65;
    assert!(matches!(
        Keyboard::from_bytes(&k.to_bytes().unwrap(), NFD),
        Err(DecodeError::Invariant(_))
    ));
}

// [spec:kbdgen:req:ldml.model.decode/test]
#[test]
fn decode_rejects_a_surrogate_scalar() {
    // `char` decodes from UTF-8, which cannot hold a surrogate.
    let mut k = Keyboard::new("se", 45, Info::default());
    k.normalization = Normalization::Disabled;
    k.keys = vec![Key::new("a", text("\u{FFFD}"))];
    let mut bytes = k.to_bytes().unwrap();
    let at = bytes
        .windows(3)
        .position(|w| w == [0xEF, 0xBF, 0xBD])
        .unwrap();
    bytes[at..at + 3].copy_from_slice(&[0xED, 0xA0, 0x80]);
    assert!(matches!(
        Keyboard::from_bytes(&bytes, None),
        Err(DecodeError::Postcard(_))
    ));
}

fn host_document(host: Host) -> Keyboard {
    let mut k = fixture();
    k.host = Some(host);
    k
}

// [spec:kbdgen:def:ldml.model.layout+1/test]
#[test]
fn equal_host_documents_share_one_keyboard() {
    let mut android = host_document(Host::Android);
    android.touch.truncate(1);
    let layout = Layout::from_host_documents(
        "vro".into(),
        BTreeMap::from([("vro".into(), "Võro".into())]),
        vec![
            host_document(Host::Windows),
            host_document(Host::MacOs),
            android,
            host_document(Host::Web),
        ],
    )
    .unwrap();
    assert_eq!(layout.keyboards.len(), 2);
    assert_eq!(layout.keyboards[0].host, None);
    assert_eq!(layout.keyboards[1].host, Some(Host::Android));
    assert_eq!(
        layout.hosts,
        BTreeMap::from([
            (Host::Windows, 0),
            (Host::MacOs, 0),
            (Host::Android, 1),
            (Host::Web, 0),
        ])
    );
    assert_eq!(layout.keyboard_for(Host::Android).unwrap().touch.len(), 1);
    assert!(layout.keyboard_for(Host::Ios).is_none());
    assert_eq!(layout.validate(NFD), Ok(()));
}

// [spec:kbdgen:def:ldml.model.layout+1/test]
#[test]
fn malformed_layouts_are_rejected() {
    let make = || {
        Layout::from_host_documents(
            "vro".into(),
            BTreeMap::new(),
            vec![host_document(Host::Windows), {
                let mut k = host_document(Host::Ios);
                k.hardware = None;
                k.windows = Windows::default();
                k.emoji.key = None;
                k
            }],
        )
        .unwrap()
    };
    assert_eq!(make().validate(NFD), Ok(()));

    let mut l = make();
    l.keyboards[1] = host_document(Host::Ios);
    assert_eq!(
        l.validate(NFD),
        Err(LayoutError::Duplicate {
            first: 0,
            second: 1
        })
    );
    let mut l = make();
    l.hosts.insert(Host::Linux, 5);
    assert_eq!(
        l.validate(NFD),
        Err(LayoutError::HostIndex { host: Host::Linux })
    );
    let mut l = make();
    l.hosts.insert(Host::Linux, 1);
    assert_eq!(
        l.validate(NFD),
        Err(LayoutError::HostMismatch { host: Host::Linux })
    );
    let mut l = make();
    l.hosts.remove(&Host::Ios);
    assert_eq!(l.validate(NFD), Err(LayoutError::Unused { index: 1 }));
    let mut l = make();
    l.keyboards[1].keys[1].multi_tap = vec![1];
    assert!(matches!(
        l.validate(NFD),
        Err(LayoutError::Keyboard { index: 1, .. })
    ));

    let mut hostless = fixture();
    hostless.host = None;
    assert_eq!(
        Layout::from_host_documents("vro".into(), BTreeMap::new(), vec![hostless]),
        Err(LayoutError::MissingHost { document: 0 })
    );
    assert_eq!(
        Layout::from_host_documents(
            "vro".into(),
            BTreeMap::new(),
            vec![host_document(Host::Web), host_document(Host::Web)]
        ),
        Err(LayoutError::DuplicateHost { host: Host::Web })
    );
}

#[test]
fn names_round_trip_for_enumerations() {
    for h in Host::ALL {
        assert_eq!(Host::from_name(h.name()), Some(h));
    }
    for r in Role::ALL {
        assert_eq!(Role::from_name(r.name()), Some(r));
    }
    for d in Direction::ALL {
        assert_eq!(Direction::from_name(d.name()), Some(d));
    }
    assert_eq!(Host::from_name("macos"), None);
}
