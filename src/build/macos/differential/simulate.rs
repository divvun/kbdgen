//! A `.keylayout` as macOS runs it (`ldml.macos.semantics`): key-map
//! selection by modifier terms, then key outputs, actions, dead-key states
//! and terminators.

use anyhow::{Result, bail};

use crate::build::macos::input::{Binding, KeyMap, KeylayoutInput, NONE_STATE, When};

/// The modifier keys held, by side, and the Caps Lock state.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Mods {
    pub shift_l: bool,
    pub shift_r: bool,
    pub caps: bool,
    pub option_l: bool,
    pub option_r: bool,
    pub control_l: bool,
    pub control_r: bool,
    pub command: bool,
}

/// How one modifier term token constrains its key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Need {
    Absent,
    Required,
    Optional,
}

/// A modifier term: for Shift, Option and Control the `Need` of the any,
/// left and right names, then Caps Lock and Command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Term {
    pairs: [[Need; 3]; 3],
    caps: Need,
    command: Need,
}

const NAMES: [[&str; 3]; 3] = [
    ["anyShift", "shift", "rightShift"],
    ["anyOption", "option", "rightOption"],
    ["anyControl", "control", "rightControl"],
];

fn parse_term(keys: &str) -> Result<Term> {
    let mut term = Term {
        pairs: [[Need::Absent; 3]; 3],
        caps: Need::Absent,
        command: Need::Absent,
    };
    for token in keys.split_whitespace() {
        let (name, need) = match token.strip_suffix('?') {
            Some(name) => (name, Need::Optional),
            None => (token, Need::Required),
        };
        let slot =
            match name {
                "caps" => &mut term.caps,
                "command" => &mut term.command,
                _ => {
                    let Some((class, side)) = NAMES.iter().enumerate().find_map(|(c, names)| {
                        names.iter().position(|n| *n == name).map(|s| (c, s))
                    }) else {
                        bail!("unknown modifier key {token:?}");
                    };
                    &mut term.pairs[class][side]
                }
            };
        *slot = need;
    }
    Ok(term)
}

fn single(need: Need, down: bool) -> bool {
    match need {
        Need::Absent => !down,
        Need::Required => down,
        Need::Optional => true,
    }
}

/// Whether a side pair accepts (left, right): `any` needs either key, a
/// side name needs its key, and a key no name mentions must be up.
fn pair([any, left, right]: [Need; 3], l: bool, r: bool) -> bool {
    match any {
        Need::Required => l || r,
        Need::Optional => true,
        Need::Absent => single(left, l) && single(right, r),
    }
}

fn matches(term: &Term, m: Mods) -> bool {
    pair(term.pairs[0], m.shift_l, m.shift_r)
        && pair(term.pairs[1], m.option_l, m.option_r)
        && pair(term.pairs[2], m.control_l, m.control_r)
        && single(term.caps, m.caps)
        && single(term.command, m.command)
}

/// A keylayout being typed on: its key maps with parsed terms, and the
/// pending dead-key state.
pub struct Keylayout<'a> {
    input: &'a KeylayoutInput,
    terms: Vec<Vec<Term>>,
    /// The pending dead-key state, if any.
    pub state: Option<String>,
}

impl<'a> Keylayout<'a> {
    pub fn new(input: &'a KeylayoutInput) -> Result<Self> {
        let terms = input
            .maps
            .iter()
            .map(|map| map.modifiers.iter().map(|keys| parse_term(keys)).collect())
            .collect::<Result<_>>()?;
        Ok(Keylayout {
            input,
            terms,
            state: None,
        })
    }

    /// The key map `m` selects: the first whose terms match, else the
    /// default index.
    // [spec:kbdgen:sem:ldml.macos.semantics]
    pub fn select(&self, m: Mods) -> Option<&'a KeyMap> {
        let index = self
            .terms
            .iter()
            .position(|terms| terms.iter().any(|term| matches(term, m)))
            .unwrap_or(self.input.default_index);
        self.input.maps.get(index)
    }

    /// How many key maps `m` matches, which is at most one for every state
    /// the engine types in.
    pub fn match_count(&self, m: Mods) -> usize {
        self.terms
            .iter()
            .filter(|terms| terms.iter().any(|term| matches(term, m)))
            .count()
    }

    fn terminator(&self, state: &str) -> &str {
        self.input
            .terminators
            .iter()
            .find(|when| when.state == state)
            .and_then(|when| when.output.as_deref())
            .unwrap_or("")
    }

    /// The pending state's terminator, if a state is pending.
    pub fn pending(&self) -> Option<String> {
        self.state
            .as_deref()
            .map(|state| self.terminator(state).to_owned())
    }

    fn apply(&mut self, when: &When, out: &mut String) {
        out.push_str(when.output.as_deref().unwrap_or(""));
        self.state = when.next.clone().filter(|next| next != NONE_STATE);
    }

    /// Presses key `code` with `m`; returns the text it types. A key the
    /// selected map lacks types nothing and keeps the state. Otherwise a
    /// pending state with no `when` of its own types its terminator first,
    /// and the key then acts as in state `none`.
    // [spec:kbdgen:sem:ldml.macos.semantics]
    pub fn press(&mut self, code: u16, m: Mods) -> String {
        let mut out = String::new();
        let Some(key) = self
            .select(m)
            .and_then(|map| map.keys.iter().find(|k| k.code == code))
        else {
            return out;
        };
        let current = self.state.clone().unwrap_or_else(|| NONE_STATE.to_owned());
        let empty = Vec::new();
        let (plain, whens) = match &key.binding {
            Binding::Output(output) => (Some(output), &empty),
            Binding::Action { whens, .. } => (None, whens),
        };
        if let Some(when) = whens.iter().find(|w| w.state == current) {
            let when = when.clone();
            self.apply(&when, &mut out);
            return out;
        }
        if self.state.is_some() {
            out.push_str(self.terminator(&current));
            self.state = None;
        }
        match (plain, whens.iter().find(|w| w.state == NONE_STATE)) {
            (Some(output), _) => out.push_str(output),
            (None, Some(when)) => {
                let when = when.clone();
                self.apply(&when, &mut out);
            }
            (None, None) => {}
        }
        out
    }
}
