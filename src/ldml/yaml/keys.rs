//! Keys from tokens (`ldml.yaml.key-ids`): each distinct key definition
//! gets one id, synthesized from what it does, with a `-2`, `-3`, …
//! suffix when another definition already has that id.

use kbd_ldml::escape::Piece;
use kbd_model::{DEFAULT_WIDTH, Direction, Role};

use super::error::{At, Result};
use super::schema::{DeadNode, ExplicitKey, LongPressEntry};
use super::text::hex_name;
use super::tokens::Token;

/// What a token-made key does. Long press and flicks are kept as the
/// tokens that name their keys, so equal definitions compare equal before
/// any id exists.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KeyDef {
    pub output: Vec<Piece>,
    pub gap: bool,
    pub layer: Option<String>,
    pub width: u32,
    pub stretch: bool,
    pub long_press: Vec<Token>,
    pub flicks: Vec<(Vec<Direction>, Token)>,
    pub role: Option<Role>,
}

impl KeyDef {
    fn plain(output: Vec<Piece>) -> KeyDef {
        KeyDef {
            output,
            width: DEFAULT_WIDTH,
            ..KeyDef::default()
        }
    }

    fn shape(&self) -> Shape<'_> {
        Shape {
            output: &self.output,
            gap: self.gap,
            layer: self.layer.as_deref(),
            stretch: self.stretch,
            role: self.role,
            plain: *self == KeyDef::plain(self.output.clone()),
        }
    }
}

/// What the id of a key depends on. `plain` says the key has nothing but
/// its output: default width, no gestures, no layer, role or stretch.
pub struct Shape<'a> {
    pub output: &'a [Piece],
    pub gap: bool,
    pub layer: Option<&'a str>,
    pub stretch: bool,
    pub role: Option<Role>,
    pub plain: bool,
}

pub enum BaseId {
    Id(String),
    /// `o-<n>`, numbered by the order outputs with markers are first used.
    MarkerOutput,
}

// [spec:kbdgen:def:ldml.yaml.key-ids]
/// The id a key gets before collisions. `dead` says whether a marker is a
/// top-level dead key's.
pub fn base_id(shape: &Shape, dead: &dyn Fn(&str) -> bool) -> BaseId {
    if let Some(role) = shape.role {
        return BaseId::Id(match shape.layer {
            Some(layer) => format!("{}-{layer}", role.name()),
            None => role.name().to_string(),
        });
    }
    if shape.gap {
        return BaseId::Id("gap".to_string());
    }
    if let Some(layer) = shape.layer {
        return BaseId::Id(format!("layer-{layer}"));
    }
    if let [Piece::Marker(m)] = shape.output
        && dead(m)
    {
        return BaseId::Id(format!("dk-{m}"));
    }
    if shape.output == [Piece::Char(' ')] && shape.stretch {
        return BaseId::Id("space".to_string());
    }
    if let [Piece::Char(c)] = shape.output
        && c.is_ascii_alphanumeric()
        && shape.plain
    {
        return BaseId::Id(c.to_string());
    }
    if shape.output.iter().any(|p| matches!(p, Piece::Marker(_))) {
        return BaseId::MarkerOutput;
    }
    let chars = shape.output.iter().filter_map(|p| match p {
        Piece::Char(c) => Some(*c),
        Piece::Marker(_) => None,
    });
    BaseId::Id(format!("u-{}", hex_name(chars, "-")))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Slot {
    /// A `keys` entry; its definition is LDML as written.
    Explicit,
    /// An implied key, which the document does not write.
    Implied(KeyDef),
    /// A key made from tokens, with its long-press and flick keys' ids.
    Made {
        def: KeyDef,
        long_press: Vec<String>,
        flicks: Vec<(Vec<Direction>, String)>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub id: String,
    pub slot: Slot,
    /// The decoded output, for implied layers, displays and lengths.
    pub output: Vec<Piece>,
}

/// Where a token is: the long-press entries in force and, in touch rows,
/// the layer and the ids of its size's layers.
pub struct Ctx<'a> {
    pub long_press: &'a [LongPressEntry],
    pub touch: Option<(&'a str, Vec<&'a str>)>,
}

/// The key table of one host document, in document order: `keys`
/// entries, implied keys, then made keys in order of first use.
pub struct KeyTable<'a> {
    pub entries: Vec<Entry>,
    dead: &'a [DeadNode],
    /// Identities of the top-level dead keys a `\d{}` token reached, in
    /// order of first use.
    pub reached: Vec<String>,
    marker_outputs: Vec<Vec<Piece>>,
}

// [spec:kbdgen:sem:ldml.yaml.touch.roles]
/// The layer a switching role goes to from layer `from`
/// (`ldml.yaml.touch.roles`); `None` for roles the host draws as gaps.
pub fn role_target(role: Role, from: &str) -> Option<&'static str> {
    match role {
        Role::Shift => Some(if from == "base" { "shift" } else { "base" }),
        Role::Symbols => Some(if from == "base" || from == "shift" {
            "symbols-1"
        } else {
            "base"
        }),
        Role::ShiftSymbols => Some(if from == "symbols-2" {
            "symbols-1"
        } else {
            "symbols-2"
        }),
        _ => None,
    }
}

impl<'a> KeyTable<'a> {
    pub fn new(
        explicit: &[ExplicitKey],
        implied: Vec<kbd_model::Key>,
        dead: &'a [DeadNode],
    ) -> Self {
        let mut entries: Vec<Entry> = explicit
            .iter()
            .map(|k| Entry {
                id: k.id.clone(),
                slot: Slot::Explicit,
                output: k.decoded.clone(),
            })
            .collect();
        for key in implied {
            if entries.iter().any(|e| e.id == key.id) {
                continue;
            }
            let output: Vec<Piece> = key.output.chars().map(Piece::Char).collect();
            let def = KeyDef {
                output: output.clone(),
                gap: key.gap,
                width: key.width,
                stretch: key.stretch,
                ..KeyDef::default()
            };
            entries.push(Entry {
                id: key.id,
                slot: Slot::Implied(def),
                output,
            });
        }
        KeyTable {
            entries,
            dead,
            reached: Vec::new(),
            marker_outputs: Vec::new(),
        }
    }

    pub fn output_of(&self, id: &str) -> Option<&[Piece]> {
        self.entries
            .iter()
            .find(|e| e.id == id)
            .map(|e| e.output.as_slice())
    }

    fn taken(&self, id: &str) -> bool {
        self.entries.iter().any(|e| e.id == id)
    }

    fn find(&self, def: &KeyDef) -> Option<&str> {
        self.entries.iter().find_map(|e| match &e.slot {
            Slot::Implied(d) | Slot::Made { def: d, .. } if d == def => Some(e.id.as_str()),
            _ => None,
        })
    }

    fn base_id(&mut self, def: &KeyDef) -> String {
        let dead = |m: &str| self.dead.iter().any(|d| d.marker == m);
        match base_id(&def.shape(), &dead) {
            BaseId::Id(id) => id,
            BaseId::MarkerOutput => {
                let n = match self.marker_outputs.iter().position(|o| *o == def.output) {
                    Some(i) => i + 1,
                    None => {
                        self.marker_outputs.push(def.output.clone());
                        self.marker_outputs.len()
                    }
                };
                format!("o-{n}")
            }
        }
    }

    fn dead_marker(&mut self, identity: &str, at: &At) -> Result<String> {
        let node = self
            .dead
            .iter()
            .find(|d| d.identity == identity)
            .ok_or_else(|| at.error(format!("no dead key has the identity {identity:?}")))?;
        if !self.reached.iter().any(|r| r == identity) {
            self.reached.push(identity.to_string());
        }
        Ok(node.marker.clone())
    }

    fn def(&mut self, token: &Token, ctx: &Ctx, at: &At) -> Result<KeyDef> {
        let width = |w: &Option<u32>| w.unwrap_or(DEFAULT_WIDTH);
        let mut def = match token {
            Token::Gap(w) => KeyDef {
                gap: true,
                width: width(w),
                ..KeyDef::default()
            },
            Token::Space(w) => KeyDef {
                output: vec![Piece::Char(' ')],
                stretch: true,
                width: width(w),
                ..KeyDef::default()
            },
            Token::Role(role, w) => {
                let (from, layers) = ctx
                    .touch
                    .as_ref()
                    .ok_or_else(|| at.error("roles are touch-only tokens"))?;
                let layer = match role_target(*role, from) {
                    Some(target) if layers.contains(&target) => Some(target.to_string()),
                    Some(target) => {
                        return Err(at.error(format!(
                            "{} switches to layer {target}, which this size lacks",
                            role.name()
                        )));
                    }
                    None => None,
                };
                KeyDef {
                    gap: layer.is_none(),
                    layer,
                    role: Some(*role),
                    width: width(w),
                    ..KeyDef::default()
                }
            }
            Token::Layer(layer, w) => {
                let (_, layers) = ctx
                    .touch
                    .as_ref()
                    .ok_or_else(|| at.error("\\l{} is a touch-only token"))?;
                if !layers.contains(&layer.as_str()) {
                    return Err(at.error(format!("this size has no layer {layer}")));
                }
                KeyDef {
                    layer: Some(layer.clone()),
                    width: width(w),
                    ..KeyDef::default()
                }
            }
            Token::Sized(output, w) => KeyDef {
                output: output.clone(),
                width: *w,
                ..KeyDef::default()
            },
            Token::Dead(identity) => {
                KeyDef::plain(vec![Piece::Marker(self.dead_marker(identity, at)?)])
            }
            Token::Output(output) => KeyDef::plain(output.clone()),
            Token::NoKey | Token::KeyRef(_) => {
                return Err(at.error("this token names no key definition"));
            }
        };
        if !def.output.is_empty()
            && let Some(entry) = ctx.long_press.iter().find(|e| e.output == def.output)
        {
            def.long_press = entry.candidates.clone();
        }
        Ok(def)
    }

    // [spec:kbdgen:sem:ldml.yaml.key-ids.collisions]
    /// The id of the key a token makes or names, registering it on first
    /// use. A new definition whose id is taken by a different one gets the
    /// first free suffix `-2`, `-3`, …. Its long-press keys, then its flick
    /// keys, are registered right after it, which is their place in
    /// document order. `flicks` are the flick targets of this position,
    /// each with where it was written.
    pub fn key(
        &mut self,
        token: &Token,
        ctx: &Ctx,
        flicks: &[(Vec<Direction>, Token, At)],
        at: &At,
    ) -> Result<Option<String>> {
        if let Token::KeyRef(id) = token {
            if !flicks.is_empty() {
                return Err(at.error("a \\k{} key takes flicks from its keys entry"));
            }
            return if self.taken(id) {
                Ok(Some(id.clone()))
            } else {
                Err(at.error(format!("no key has the id {id}")))
            };
        }
        if *token == Token::NoKey {
            return Ok(None);
        }
        let mut def = self.def(token, ctx, at)?;
        if !flicks.is_empty() && (def.gap || def.output.is_empty()) {
            return Err(at.error("only a key with output takes flicks"));
        }
        def.flicks = flicks
            .iter()
            .map(|(d, t, _)| (d.clone(), t.clone()))
            .collect();
        if let Some(id) = self.find(&def) {
            return Ok(Some(id.to_string()));
        }
        let base = self.base_id(&def);
        let mut id = base.clone();
        let mut n = 2;
        while self.taken(&id) {
            id = format!("{base}-{n}");
            n += 1;
        }
        let index = self.entries.len();
        self.entries.push(Entry {
            id: id.clone(),
            slot: Slot::Made {
                def: def.clone(),
                long_press: Vec::new(),
                flicks: Vec::new(),
            },
            output: def.output.clone(),
        });
        let mut long_press = Vec::new();
        if let Some(entry) = ctx.long_press.iter().find(|e| e.output == def.output)
            && !def.output.is_empty()
        {
            for (i, candidate) in entry.candidates.iter().enumerate() {
                let cat = entry.at.row(0).token(i);
                if matches!(
                    candidate,
                    Token::NoKey | Token::Gap(_) | Token::Role(..) | Token::Layer(..)
                ) {
                    return Err(cat.error("a long-press candidate is a key with output"));
                }
                if let Some(cid) = self.key(candidate, ctx, &[], &cat)? {
                    long_press.push(cid);
                }
            }
        }
        let mut flick_ids = Vec::new();
        for (directions, target, fat) in flicks {
            if let Some(fid) = self.key(target, ctx, &[], fat)? {
                flick_ids.push((directions.clone(), fid));
            }
        }
        if let Some(Entry {
            slot:
                Slot::Made {
                    long_press: lp,
                    flicks: f,
                    ..
                },
            ..
        }) = self.entries.get_mut(index)
        {
            *lp = long_press;
            *f = flick_ids;
        }
        Ok(Some(id))
    }
}
