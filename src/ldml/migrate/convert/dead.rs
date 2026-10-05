//! `transforms` → `deadKeys` (`ldml.migrate.dead-keys`): each top-level
//! entry is a dead key with the same identity, its `' '` child the
//! standalone output and its other children, in order, the compose
//! entries; deeper branches become nested nodes.

use super::super::defect::{Code, Defects};
use super::super::out::{ComposeOut, DeadOut};
use super::super::source::{Desktop, Node3, Source3, Touch};
use super::super::text::plain;
use super::{Ctx, decode_at};

/// A v3 string and its decoded text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Leaf {
    pub raw: String,
    pub text: String,
}

/// A dead key of the migrated layout: its decoded identity, how v4
/// spells it, the v3 transform key and path, its standalone output, and
/// its compose inputs with their outputs (none for a nested node).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeadDef {
    pub identity: String,
    pub spelling: String,
    pub raw: String,
    pub path: String,
    pub standalone: Leaf,
    pub compose: Vec<(Leaf, Option<Leaf>)>,
}

impl DeadDef {
    /// Whether a transform string holds a `\u{…}` escape.
    fn escaped(&self) -> bool {
        let has = |s: &str| s.contains("\\u{");
        has(&self.raw)
            || has(&self.standalone.raw)
            || self
                .compose
                .iter()
                .any(|(i, o)| has(&i.raw) || o.as_ref().is_some_and(|o| has(&o.raw)))
    }
}

struct Node {
    out: DeadOut,
    standalone: Option<Leaf>,
    compose: Vec<(Leaf, Option<Leaf>)>,
}

fn node(defects: &mut Defects, children: &[(String, Node3)], identity: &str, path: &str) -> Node {
    let mut out = DeadOut::default();
    let mut standalone: Option<Leaf> = None;
    let mut compose: Vec<(Leaf, Option<Leaf>)> = Vec::new();
    for (raw, child) in children {
        let at = format!("{path}.{raw}");
        let Some((fixed, input)) = decode_at(defects, raw, &at, None) else {
            continue;
        };
        if input == " " {
            match (child, &standalone) {
                (Node3::Leaf(v), None) => {
                    if let Some((vf, text)) = decode_at(defects, v, &at, None) {
                        if text != identity {
                            out.standalone = Some(plain(&vf, &text));
                        }
                        standalone = Some(Leaf {
                            raw: v.clone(),
                            text,
                        });
                    }
                }
                _ => defects.add(
                    Code::M10,
                    &at,
                    "a second or branching ' ' entry is not carried over; standalone is one output",
                ),
            }
            continue;
        }
        if input.is_empty() || compose.iter().any(|(i, _)| i.text == input) {
            defects.add(
                Code::M10,
                &at,
                "an empty input, or one that decodes like an earlier input, is not carried over",
            );
            continue;
        }
        let leaf = Leaf {
            raw: raw.clone(),
            text: input.clone(),
        };
        let spelled = plain(&fixed, &input);
        match child {
            Node3::Leaf(v) => {
                let Some((vf, text)) = decode_at(defects, v, &at, None) else {
                    continue;
                };
                out.compose
                    .push((spelled, ComposeOut::Output(plain(&vf, &text))));
                compose.push((
                    leaf,
                    Some(Leaf {
                        raw: v.clone(),
                        text,
                    }),
                ));
            }
            Node3::Branch(grand) => {
                let nested = node(defects, grand, &format!("{identity}{input}"), &at);
                out.compose.push((spelled, ComposeOut::Node(nested.out)));
                compose.push((leaf, None));
            }
        }
    }
    Node {
        out,
        standalone,
        compose,
    }
}

// [spec:kbdgen:sem:ldml.migrate.dead-keys+1]
/// The dead keys of the `transforms` tree in order, and the `deadKeys`
/// table written for them. A top-level leaf, which v3 ignored, is not
/// carried over; nor is a root that decodes like an earlier one.
pub fn table(src: &Source3, defects: &mut Defects) -> (Vec<DeadDef>, Vec<(String, DeadOut)>) {
    let mut defs: Vec<DeadDef> = Vec::new();
    let mut table = Vec::new();
    for (raw, root) in &src.transforms {
        let path = format!("transforms.{raw}");
        let Some((fixed, identity)) = decode_at(defects, raw, &path, None) else {
            continue;
        };
        let children = match root {
            Node3::Branch(children) => children,
            Node3::Leaf(_) => {
                defects.add(
                    Code::M10,
                    &path,
                    "a top-level transform that is a single output is not carried over; v3 ignored it",
                );
                continue;
            }
        };
        if identity.is_empty() || defs.iter().any(|d| d.identity == identity) {
            defects.add(
                Code::M10,
                &path,
                "an empty root, or one that decodes like an earlier root, is not carried over",
            );
            continue;
        }
        let spelling = plain(&fixed, &identity);
        let built = node(defects, children, &identity, &path);
        let standalone = built.standalone.unwrap_or_else(|| Leaf {
            raw: raw.clone(),
            text: identity.clone(),
        });
        if standalone.text != identity {
            defects.add(
                Code::M16,
                &path,
                format!(
                    "the standalone output {:?} differs from the root {identity:?}; standalone holds the ' ' entry",
                    standalone.text
                ),
            );
        }
        table.push((spelling.clone(), built.out));
        defs.push(DeadDef {
            identity,
            spelling,
            raw: raw.clone(),
            path,
            standalone,
            compose: built.compose,
        });
    }
    (defs, table)
}

/// Reports, per platform that showed transform strings as written
/// (Windows, ChromeOS, iOS), the dead keys whose strings hold escapes,
/// which v4 decodes (M14).
pub fn escaped_leaves(ctx: &mut Ctx) {
    let raw_platforms = [
        Desktop::Windows.name(),
        Desktop::ChromeOs.name(),
        Touch::Ios.name(),
    ];
    for def in ctx.dead {
        if !def.escaped() {
            continue;
        }
        let platforms: Vec<&str> = raw_platforms
            .iter()
            .copied()
            .filter(|p| ctx.used.contains(&(*p, def.identity.clone())))
            .collect();
        if !platforms.is_empty() {
            ctx.defects.add(
                Code::M14,
                &def.path,
                format!(
                    "{} showed this dead key's \\u{{…}} strings as written; they are now decoded",
                    platforms.join(", ")
                ),
            );
        }
    }
}
