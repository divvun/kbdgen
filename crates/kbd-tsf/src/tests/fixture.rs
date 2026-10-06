//! A Windows keyboard exercising what the text service handles: a dead key
//! with a flush output and a multi-unit composition, transforms that
//! delete committed text (one through a surrogate pair), a backspace rule,
//! an AltGr layer with a multi-unit output and a key with no output, a
//! `ctrl alt` layer, `B00` bound as an extra modifier, and a decimal key.

use kbd_engine::{Model, Options};
use kbd_model::Component::{Alt, AltR, Ctrl, Extra1, Shift};
use kbd_model::{
    ExtraModifierKey, Form, Hardware, HardwareLayer, Host, Info, Key, KeyIndex, Keyboard,
    ModifierSet, Modifiers, Normalization, Pattern, ReplacementItem, Rule, Text, TextElem,
    TransformGroup, TreeAtom, TreeItem,
};

pub const ACUTE: u8 = 0x0D;
pub const Q: u8 = 0x10;
pub const T: u8 = 0x14;
pub const A: u8 = 0x1E;
pub const APOSTROPHE: u8 = 0x2B;
pub const B00: u8 = 0x56;
pub const Z: u8 = 0x2C;
pub const B: u8 = 0x30;
pub const SPACE: u8 = 0x39;

const GAP: KeyIndex = 12;
const EMPTY: KeyIndex = 13;

fn ch(c: char) -> TreeItem {
    TreeItem::one(TreeAtom::Char(c))
}

fn acute() -> TreeItem {
    TreeItem::one(TreeAtom::Marker(0))
}

fn rule(from: Vec<TreeItem>, to: Vec<ReplacementItem>) -> Rule {
    Rule {
        from: Pattern::from_tree(false, vec![from]).unwrap(),
        to,
    }
}

fn to(s: &str) -> Vec<ReplacementItem> {
    vec![ReplacementItem::Text(Text::from(s))]
}

fn layer(components: &[kbd_model::Component], rows: Vec<Vec<KeyIndex>>) -> HardwareLayer {
    HardwareLayer {
        id: None,
        modifiers: vec![ModifierSet::Set(Modifiers::of(components))],
        rows,
    }
}

fn keys() -> Vec<Key> {
    vec![
        Key::new("acute", Text(vec![TextElem::Marker(0)])),
        Key::new("q", Text::from("q")),
        Key::new("t", Text::from("t")),
        Key::new("a", Text::from("a")),
        Key::new("apostrophe", Text::from("'")),
        Key::new("z", Text::from("z")),
        Key::new("b", Text::from("b")),
        Key::new("space", Text::from(" ")),
        Key::new("Q", Text::from("Q")),
        Key::new("A", Text::from("A")),
        Key::new("t-acute", Text::from("t\u{301}")),
        Key::new("one", Text::from("1")),
        Key::gap("gap"),
        Key::new("empty", Text::new()),
        Key::new("a-diaeresis", Text::from("ä")),
    ]
}

fn hardware() -> Hardware {
    Hardware {
        form: Form {
            id: "iso".to_owned(),
            rows: vec![
                vec![ACUTE, Q, T],
                vec![A, APOSTROPHE],
                vec![B00, Z, B],
                vec![SPACE],
            ],
        },
        min_device_width: None,
        layers: vec![
            layer(
                &[],
                vec![vec![0, 1, 2], vec![3, 4], vec![GAP, 5, 6], vec![7]],
            ),
            layer(&[Shift], vec![vec![GAP, 8, GAP], vec![9, 4]]),
            layer(&[AltR], vec![vec![0, GAP, 10], vec![EMPTY]]),
            layer(&[Ctrl, Alt], vec![vec![], vec![14]]),
            layer(&[Extra1], vec![vec![], vec![11]]),
        ],
    }
}

fn simple() -> Vec<TransformGroup> {
    let any = TreeItem::one(TreeAtom::Capture(vec![vec![TreeItem::one(TreeAtom::Any)]]));
    vec![
        TransformGroup::Rules(vec![
            rule(vec![acute(), ch('a')], to("á")),
            rule(vec![acute(), ch('b')], to("b\u{301}")),
            rule(vec![acute(), ch(' ')], to("´")),
        ]),
        TransformGroup::Rules(vec![rule(
            vec![acute(), any],
            vec![
                ReplacementItem::Text(Text::from("´")),
                ReplacementItem::Group(1),
            ],
        )]),
        TransformGroup::Rules(vec![
            rule(vec![ch('q'), ch('\'')], to("ʠ")),
            rule(vec![ch('z'), ch('\'')], to("𝕫")),
            rule(vec![ch('𝕫'), ch('\'')], to("ʐ")),
        ]),
    ]
}

/// The keyboard, as a layout DLL's resource carries it.
pub fn keyboard() -> Keyboard {
    let mut k = Keyboard::new(
        "und",
        46,
        Info {
            name: "Fixture".to_owned(),
            ..Info::default()
        },
    );
    k.host = Some(Host::Windows);
    k.normalization = Normalization::Disabled;
    k.markers = vec!["acute".to_owned()];
    k.keys = keys();
    k.hardware = Some(hardware());
    k.simple = simple();
    k.backspace = vec![TransformGroup::Rules(vec![rule(vec![ch('ʠ')], to("q"))])];
    k.flush.insert(0, "´".to_owned());
    k.decimal = Some(Text::from(","));
    k.windows.extra_modifiers = vec![ExtraModifierKey::B00];
    k.context_len = u8::try_from(k.computed_context_len().unwrap()).unwrap();
    k
}

pub fn model() -> Model {
    Model::from_keyboard(keyboard(), Options::default()).unwrap()
}
