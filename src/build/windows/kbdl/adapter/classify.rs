//! What a layout's DLL does not reproduce: the model features that only the
//! text service provides, reported once per layout.

use indexmap::IndexMap;
use kbd_model::{
    Atom, Fixed, Item, Keyboard, Normalization, Pattern, ReplacementItem, Rule, TransformGroup,
};

use super::super::{
    diag::Diagnostics,
    input::{DeadKeyNode, STANDALONE},
};

/// The most examples a category lists.
const MAX_EXAMPLES: usize = 10;

/// One category of what the DLL does not reproduce.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Category {
    pub count: usize,
    pub examples: Vec<String>,
}

impl Category {
    fn add(&mut self, example: impl FnOnce() -> String) {
        if self.examples.len() < MAX_EXAMPLES {
            self.examples.push(example());
        }
        self.count += 1;
    }
}

/// Everything of `ldml.kbdl.classify` for one layout.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Classification {
    pub text_transforms: Category,
    pub reorder_groups: Category,
    pub backspace_rules: Category,
    pub long_leaves: Category,
    pub mixed_outputs: Category,
    pub normalization: bool,
}

/// Whether every alternative of `pattern` must start with a marker, as a
/// dead-key transform does.
pub fn starts_with_marker(pattern: &Pattern) -> bool {
    pattern.nodes.first().is_some_and(|top| {
        top.alternatives.iter().all(|sequence| {
            sequence.first().is_some_and(|item| {
                item.min >= 1 && matches!(item.atom, Atom::Marker(_) | Atom::AnyMarker)
            })
        })
    })
}

/// A pattern in LDML's syntax, close enough to recognise in a message.
fn show_items(keyboard: &Keyboard, pattern: &Pattern, node: usize, out: &mut String) {
    let Some(alternation) = pattern.nodes.get(node) else {
        return;
    };
    for (i, sequence) in alternation.alternatives.iter().enumerate() {
        if i > 0 {
            out.push('|');
        }
        for item in sequence {
            show_item(keyboard, pattern, item, out);
        }
    }
}

fn show_item(keyboard: &Keyboard, pattern: &Pattern, item: &Item, out: &mut String) {
    match item.atom {
        Atom::Char(c) if c.is_alphanumeric() || c.is_ascii_punctuation() => out.push(c),
        Atom::Char(c) => out.push_str(&format!("\\u{{{:x}}}", u32::from(c))),
        Atom::Any => out.push('.'),
        Atom::Class(_) => out.push_str("[…]"),
        Atom::Fixed(fixed) => out.push_str(fixed_escape(fixed)),
        Atom::Marker(m) => {
            let name = keyboard.marker_name(m).unwrap_or("?");
            out.push_str(&format!("\\m{{{name}}}"));
        }
        Atom::AnyMarker => out.push_str("\\m{.}"),
        Atom::Set(_) => out.push_str("$[…]"),
        Atom::Group(node) => {
            out.push_str("(?:");
            show_items(keyboard, pattern, usize::from(node), out);
            out.push(')');
        }
        Atom::Capture { node, .. } => {
            out.push('(');
            show_items(keyboard, pattern, usize::from(node), out);
            out.push(')');
        }
    }
    match (item.min, item.max) {
        (1, 1) => {}
        (0, 1) => out.push('?'),
        (min, max) => out.push_str(&format!("{{{min},{max}}}")),
    }
}

fn fixed_escape(fixed: Fixed) -> &'static str {
    match fixed {
        Fixed::Space => "\\s",
        Fixed::NotSpace => "\\S",
        Fixed::Digit => "\\d",
        Fixed::NotDigit => "\\D",
        Fixed::Word => "\\w",
        Fixed::NotWord => "\\W",
        Fixed::Tab => "\\t",
        Fixed::CarriageReturn => "\\r",
        Fixed::LineFeed => "\\n",
        Fixed::FormFeed => "\\f",
        Fixed::VerticalTab => "\\v",
    }
}

fn show_pattern(keyboard: &Keyboard, pattern: &Pattern) -> String {
    let mut out = String::new();
    if pattern.anchored {
        out.push('^');
    }
    show_items(keyboard, pattern, 0, &mut out);
    out
}

/// The backspace rule `ldml.yaml.dead-keys.backspace` generates: `\m{.}`
/// with no replacement.
pub fn is_generated_backspace(rule: &Rule) -> bool {
    let only_any_marker = rule.from.nodes.len() == 1
        && rule.from.nodes[0].alternatives.len() == 1
        && rule.from.nodes[0].alternatives[0] == [Item::one(Atom::AnyMarker)];
    !rule.from.anchored
        && only_any_marker
        && rule
            .to
            .iter()
            .all(|item| matches!(item, ReplacementItem::Text(text) if text.is_empty()))
}

/// Collects leaves of more than one UTF-16 unit, which `kbdl` omits, with
/// their input paths.
fn long_leaves(path: &str, node: &DeadKeyNode, category: &mut Category) {
    let DeadKeyNode::Branch(children) = node else {
        return;
    };
    for (input, child) in children {
        let shown = if input == STANDALONE { "space" } else { input };
        let child_path = format!("{path} {shown}");
        match child {
            DeadKeyNode::Leaf(output) if output.encode_utf16().count() > 1 => {
                category.add(|| format!("{child_path} → {output}"));
            }
            DeadKeyNode::Leaf(_) => {}
            DeadKeyNode::Branch(_) => long_leaves(&child_path, child, category),
        }
    }
}

/// Classifies a keyboard, its derived dead-key tree and its mixed outputs.
// [spec:kbdgen:req:ldml.kbdl.classify]
pub fn classify(
    keyboard: &Keyboard,
    tree: &IndexMap<String, DeadKeyNode>,
    mixed: &[String],
) -> Classification {
    let mut result = Classification {
        normalization: keyboard.normalization == Normalization::Enabled,
        ..Classification::default()
    };
    for (g, group) in keyboard.simple.iter().enumerate() {
        match group {
            TransformGroup::Rules(rules) => {
                for (r, rule) in rules.iter().enumerate() {
                    if !starts_with_marker(&rule.from) {
                        result.text_transforms.add(|| {
                            format!(
                                "group {} rule {}: {}",
                                g + 1,
                                r + 1,
                                show_pattern(keyboard, &rule.from)
                            )
                        });
                    }
                }
            }
            TransformGroup::Reorder(rules) => result
                .reorder_groups
                .add(|| format!("group {} ({} rules)", g + 1, rules.len())),
        }
    }
    for (g, group) in keyboard.backspace.iter().enumerate() {
        match group {
            TransformGroup::Rules(rules) => {
                for (r, rule) in rules.iter().enumerate() {
                    if !is_generated_backspace(rule) {
                        result.backspace_rules.add(|| {
                            format!(
                                "group {} rule {}: {}",
                                g + 1,
                                r + 1,
                                show_pattern(keyboard, &rule.from)
                            )
                        });
                    }
                }
            }
            TransformGroup::Reorder(rules) => result
                .backspace_rules
                .add(|| format!("group {} (reorder, {} rules)", g + 1, rules.len())),
        }
    }
    for (identity, node) in tree {
        long_leaves(identity, node, &mut result.long_leaves);
    }
    for example in mixed {
        result.mixed_outputs.add(|| example.clone());
    }
    result
}

fn describe(label: &str, category: &Category, parts: &mut Vec<String>) {
    if category.count == 0 {
        return;
    }
    parts.push(format!(
        "{} {label} (e.g. {})",
        category.count,
        category.examples.join("; ")
    ));
}

/// Reports `classification` as one information message.
// [spec:kbdgen:req:ldml.kbdl.classify]
pub fn report(classification: &Classification, diag: &mut Diagnostics) {
    let mut parts = Vec::new();
    describe(
        "transform rule(s) that do not start with a marker",
        &classification.text_transforms,
        &mut parts,
    );
    describe(
        "reorder group(s)",
        &classification.reorder_groups,
        &mut parts,
    );
    describe(
        "backspace rule(s) besides the generated one",
        &classification.backspace_rules,
        &mut parts,
    );
    describe(
        "dead-key output(s) of more than one UTF-16 unit",
        &classification.long_leaves,
        &mut parts,
    );
    describe(
        "output(s) mixing text and markers",
        &classification.mixed_outputs,
        &mut parts,
    );
    if classification.normalization {
        parts.push("normalization is enabled".to_owned());
    }
    if parts.is_empty() {
        diag.inform("the layout DLL reproduces the whole keyboard; the text service adds nothing");
    } else {
        diag.inform(format!(
            "without the text service, for example at the sign-in screen, users do not get: {}",
            parts.join("; ")
        ));
    }
}
