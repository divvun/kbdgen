//! Engine tests through `Model::key`, over keyboards built with the
//! `kbd-model` API: shared fixtures here, one module per topic.

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

use kbd_model::Component::*;
use kbd_model::{
    BottomRow, Class, ClassRange, Component, Direction, EmojiKey, ExtraModifierKey, Fixed, Flick,
    FlickSegment, Form, Hardware, HardwareLayer, Host, Info, Key as ModelKey, Keyboard,
    MarkerIndex, ModifierSet, Modifiers, Normalization, Pattern, ReorderClass, ReorderRule,
    ReplacementItem, Role, Rule, Text, TextElem, TouchLayer, TouchSet, TransformGroup, TreeAtom,
    TreeItem,
};

use crate::*;

mod backspace;
mod dead_keys;
mod harness;
mod limits;
mod modifiers;
mod normalization;
mod reorder_cases;
mod touch;
mod transforms;

fn text(s: &str) -> Text {
    Text::from(s)
}

fn marker(m: MarkerIndex) -> Text {
    Text(vec![TextElem::Marker(m)])
}

fn ch(c: char) -> TreeItem {
    TreeItem::one(TreeAtom::Char(c))
}

fn chars(s: &str) -> Vec<TreeItem> {
    s.chars().map(ch).collect()
}

fn mk(m: MarkerIndex) -> TreeItem {
    TreeItem::one(TreeAtom::Marker(m))
}

fn cap(items: Vec<TreeItem>) -> TreeItem {
    TreeItem::one(TreeAtom::Capture(vec![items]))
}

fn rule(from: Vec<TreeItem>, to: Vec<ReplacementItem>) -> Rule {
    Rule {
        from: Pattern::from_tree(false, vec![from]).unwrap(),
        to,
    }
}

fn to_text(s: &str) -> ReplacementItem {
    ReplacementItem::Text(text(s))
}

fn set(components: &[Component]) -> ModifierSet {
    ModifierSet::Set(Modifiers::of(components))
}

fn keyboard(normalization: Normalization) -> Keyboard {
    let mut k = Keyboard::new(
        "und",
        46,
        Info {
            name: "Test".to_string(),
            ..Info::default()
        },
    );
    k.normalization = normalization;
    k
}

fn layer(modifiers: Vec<ModifierSet>, rows: Vec<Vec<u16>>) -> HardwareLayer {
    HardwareLayer {
        id: None,
        modifiers,
        rows,
    }
}

/// Rows of scan codes: Q W E R T, A S, B00 Z, space.
fn form() -> Form {
    Form {
        id: "iso".to_string(),
        rows: vec![
            vec![0x10, 0x11, 0x12, 0x13, 0x14],
            vec![0x1E, 0x1F],
            vec![0x56, 0x2C],
            vec![0x39],
        ],
    }
}

fn model_with(mut k: Keyboard, options: Options) -> Model {
    k.context_len = u8::try_from(k.computed_context_len().unwrap()).unwrap();
    Model::from_keyboard(k, options).unwrap()
}

fn model(k: Keyboard) -> Model {
    model_with(k, Options::default())
}

/// A host session: the document before the caret, the preedit and the
/// engine state, applied as a TSF-like host applies actions.
struct Session<'a> {
    model: &'a Model,
    state: State,
    text: String,
    preedit: String,
    at_start: bool,
}

impl<'a> Session<'a> {
    fn new(model: &'a Model) -> Self {
        Session {
            model,
            state: State::default(),
            text: String::new(),
            preedit: String::new(),
            at_start: false,
        }
    }

    fn context(&self) -> Context {
        let n = self.text.chars().count();
        let skip = n.saturating_sub(self.model.context_len());
        Context {
            text: self.text.chars().skip(skip).collect(),
            authoritative: true,
            at_start: self.at_start && skip == 0,
        }
    }

    /// Applies the action. On `Pass` the host resets the state, leaving any
    /// preedit committed, and lets the application handle the key; for
    /// Backspace that deletes one scalar value.
    fn press(&mut self, event: KeyEvent) -> Action {
        let (action, state) = self.model.key(&self.state, &self.context(), &event);
        match &action {
            Action::Pass => {
                assert_eq!(state, self.state, "pass keeps the state");
                self.text.push_str(&self.preedit);
                self.preedit.clear();
                self.state = State::default();
                if event.key == Key::Backspace {
                    self.text.pop();
                }
            }
            Action::Edit {
                delete,
                insert,
                preedit,
                ..
            } => {
                let n = self.text.chars().count();
                assert!(*delete <= n, "delete {delete} exceeds context {n}");
                self.text = self.text.chars().take(n - delete).collect();
                self.text.push_str(insert);
                self.preedit = preedit.clone();
                self.state = state;
            }
        }
        action
    }

    fn scan(&mut self, code: u8) -> Action {
        self.press(KeyEvent::new(Key::Scan(code)))
    }

    fn scan_with(&mut self, code: u8, modifiers: ModifierState) -> Action {
        self.press(KeyEvent::with(Key::Scan(code), modifiers))
    }

    fn emit(&mut self, s: &str) -> Action {
        self.press(KeyEvent::new(Key::Emit(s.to_string())))
    }

    fn id(&mut self, id: &str) -> Action {
        self.press(KeyEvent::new(Key::Id {
            id: id.to_string(),
            gesture: Gesture::Tap,
        }))
    }

    fn backspace(&mut self) -> Action {
        self.press(KeyEvent::new(Key::Backspace))
    }

    fn pending(&self) -> Vec<&str> {
        self.model.pending_markers(&self.state)
    }
}

fn edit(delete: usize, insert: &str, preedit: &str) -> Action {
    Action::Edit {
        delete,
        insert: insert.to_string(),
        preedit: preedit.to_string(),
        layer: None,
    }
}

fn with_layer(delete: usize, insert: &str, preedit: &str, layer: &str) -> Action {
    Action::Edit {
        delete,
        insert: insert.to_string(),
        preedit: preedit.to_string(),
        layer: Some(layer.to_string()),
    }
}

const ACUTE: MarkerIndex = 0;
const GRAVE: MarkerIndex = 1;

/// Dead keys as v4 lowers them: `acute` with a flush output and the
/// macOS fallback rule, `grave` without either.
fn dead_keys(normalization: Normalization, backspace_rule: bool) -> Keyboard {
    let mut k = keyboard(normalization);
    k.markers = vec!["acute".to_string(), "grave".to_string()];
    k.keys = vec![
        ModelKey::new("acute", marker(ACUTE)),
        ModelKey::new("grave", marker(GRAVE)),
        ModelKey::new("a", text("a")),
        ModelKey::new("e", text("e")),
        ModelKey::new("space", text(" ")),
        ModelKey::new("A", text("A")),
        ModelKey::new("x", text("x")),
        ModelKey::gap("gap"),
        ModelKey::new("empty", Text::new()),
    ];
    k.hardware = Some(Hardware {
        form: form(),
        min_device_width: None,
        layers: vec![
            layer(
                vec![set(&[])],
                vec![vec![0, 1, 3, 6, 7], vec![2, 8], vec![], vec![4]],
            ),
            layer(vec![set(&[Shift])], vec![vec![], vec![5]]),
        ],
    });
    let acute_then = |c: char, out: &str| rule(vec![mk(ACUTE), ch(c)], vec![to_text(out)]);
    k.simple = vec![TransformGroup::Rules(vec![
        acute_then('a', "á"),
        acute_then('e', "é"),
        acute_then('A', "Á"),
        acute_then(' ', "´"),
        rule(
            vec![mk(ACUTE), cap(vec![TreeItem::one(TreeAtom::Any)])],
            vec![to_text("´"), ReplacementItem::Group(1)],
        ),
        rule(vec![mk(GRAVE), ch('a')], vec![to_text("à")]),
    ])];
    if backspace_rule {
        k.backspace = vec![TransformGroup::Rules(vec![rule(
            vec![TreeItem::one(TreeAtom::AnyMarker)],
            vec![],
        )])];
    }
    k.flush = BTreeMap::from([(ACUTE, "´".to_string())]);
    k
}

const Q: u8 = 0x10;
const W: u8 = 0x11;
const E: u8 = 0x12;
const R: u8 = 0x13;
const T: u8 = 0x14;
const A: u8 = 0x1E;
const S: u8 = 0x1F;
const B00: u8 = 0x56;
const Z: u8 = 0x2C;
const SPACE: u8 = 0x39;

/// One key at Q per layer, each with a distinct output.
fn modifier_keyboard() -> Keyboard {
    let mut k = keyboard(Normalization::Disabled);
    let outputs = ["n", "s", "c", "x", "r", "l", "o", "^", "⌘"];
    k.keys = outputs
        .iter()
        .enumerate()
        .map(|(i, o)| ModelKey::new(alloc::format!("k{i}"), text(o)))
        .collect();
    k.hardware = Some(Hardware {
        form: form(),
        min_device_width: None,
        layers: vec![
            layer(vec![set(&[])], vec![vec![0]]),
            layer(vec![set(&[Shift])], vec![vec![1]]),
            layer(vec![set(&[Caps])], vec![vec![2]]),
            layer(vec![set(&[Caps, Shift])], vec![vec![3]]),
            layer(vec![set(&[AltR])], vec![vec![4]]),
            layer(vec![set(&[AltL])], vec![vec![5]]),
            layer(vec![ModifierSet::Other], vec![vec![6]]),
            layer(vec![set(&[Ctrl])], vec![vec![7]]),
            layer(vec![set(&[Cmd, Shift])], vec![vec![8]]),
        ],
    });
    k
}

fn out(m: &Model, modifiers: ModifierState) -> Action {
    m.key(
        &State::default(),
        &Context::new(""),
        &KeyEvent::with(Key::Scan(Q), modifiers),
    )
    .0
}

fn typed(s: &str) -> Action {
    edit(0, s, "")
}

/// Captures, `MapSet`, classes, chained groups and an anchored rule.
fn transform_keyboard() -> Keyboard {
    let mut k = keyboard(Normalization::Disabled);
    k.markers = vec!["m".to_string()];
    k.keys = vec![ModelKey::new("mark", marker(0))];
    k.sets = vec![
        vec![text("A"), text("B"), text("CC")],
        vec![text("a"), text("b"), text("c")],
    ];
    k.classes = vec![Class {
        ranges: ['a', 'e', 'i', 'o', 'u']
            .into_iter()
            .map(ClassRange::single)
            .collect(),
        negated: false,
        markers: vec![],
    }];
    k.simple = vec![
        TransformGroup::Rules(vec![
            rule(
                vec![cap(vec![TreeItem::one(TreeAtom::Set(0))])],
                vec![ReplacementItem::MapSet {
                    group: 1,
                    from: 0,
                    to: 1,
                }],
            ),
            rule(
                vec![
                    cap(vec![TreeItem::one(TreeAtom::Class(0))]),
                    cap(vec![TreeItem::one(TreeAtom::Fixed(Fixed::Digit))]),
                ],
                vec![ReplacementItem::Group(2), ReplacementItem::Group(1)],
            ),
            rule(
                vec![
                    ch('x'),
                    cap(vec![TreeItem {
                        atom: TreeAtom::Char('y'),
                        min: 0,
                        max: 1,
                    }]),
                    ch('z'),
                ],
                vec![to_text("<"), ReplacementItem::Group(1), to_text(">")],
            ),
            rule(
                vec![TreeItem::one(TreeAtom::AnyMarker), ch('b')],
                vec![ReplacementItem::Group(0), to_text("!")],
            ),
        ]),
        TransformGroup::Rules(vec![
            rule(chars("<>"), vec![to_text("∅")]),
            rule(chars("b"), vec![to_text("BB")]),
        ]),
        TransformGroup::Rules(vec![Rule {
            from: Pattern::from_tree(true, vec![chars("q")]).unwrap(),
            to: vec![to_text("Q")],
        }]),
    ];
    k
}
