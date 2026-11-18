//! The dead keysyms of a layout's dead keys, and where composing them
//! with the reference Compose table types something other than the engine
//! (`ldml.linux.dead-keys`).

use std::collections::{BTreeMap, HashSet};

use anyhow::{Result, anyhow, bail};
use kbd_engine::{
    Action, BackspacePolicy, Context, Gesture, Key, KeyEvent, Model, Options, OutputForm, State,
};
use kbd_model::{Host, Keyboard, MarkerIndex, Text, TextElem};

use super::compose::{ComposeTable, Composed, Composer, DEAD_CHARACTERS};
use super::keysym::Keysym;
use super::levels::{self, Levels, Value};

/// The longest dead-key path compared, in keys.
const MAX_DEPTH: usize = 3;

/// One dead-key path on which the reference table types something other
/// than the engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Difference {
    /// The keys pressed, as values: `\m{name}` for a dead key, else the
    /// quoted text.
    pub path: Vec<String>,
    pub engine: String,
    pub xkb: String,
}

impl Difference {
    pub fn line(&self) -> String {
        format!(
            "{}: the engine types {}, the XKB layout {}",
            self.path.join(" then "),
            self.engine,
            self.xkb
        )
    }
}

/// The dead keys of a layout's XKB layout.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeadKeys {
    pub keysyms: BTreeMap<MarkerIndex, Keysym>,
    /// The dead keys without a dead keysym, in marker order, which type as
    /// their standalone.
    pub without: Vec<MarkerIndex>,
    pub differences: Vec<Difference>,
}

/// The text a marker's dead keysym is matched by: its flush output, else
/// the display of `\m{m}`.
fn identities(keyboard: &Keyboard, marker: MarkerIndex) -> Vec<String> {
    let flush = keyboard
        .flush
        .get(&marker)
        .filter(|flush| !flush.is_empty())
        .cloned();
    let display = keyboard
        .displays
        .for_output(&Text(vec![TextElem::Marker(marker)]))
        .filter(|display| !display.is_empty())
        .map(str::to_owned);
    flush.into_iter().chain(display).collect()
}

/// The dead keysym of each dead key of the written states, in marker
/// order: the first of `ibus_keysym_to_unicode`'s table whose isolated or
/// combining character is the marker's flush output, else its display,
/// and that no earlier marker got.
// [spec:kbdgen:sem:ldml.linux.dead-keys+2]
pub fn assign(
    keyboard: &Keyboard,
    levels: &Levels,
) -> (BTreeMap<MarkerIndex, Keysym>, Vec<MarkerIndex>) {
    let mut markers: Vec<MarkerIndex> = levels
        .keys
        .iter()
        .flat_map(|key| key.values.iter())
        .filter_map(|value| match value {
            Value::Dead(marker) => Some(*marker),
            _ => None,
        })
        .collect();
    markers.sort_unstable();
    markers.dedup();
    let mut keysyms = BTreeMap::new();
    let mut without = Vec::new();
    for marker in markers {
        let identities = identities(keyboard, marker);
        let found = identities.iter().find_map(|identity| {
            DEAD_CHARACTERS
                .iter()
                .find_map(|(name, combining, isolated)| {
                    let keysym = Keysym::from_name(name)?;
                    let matches = [combining, isolated]
                        .iter()
                        .any(|c| identity.chars().eq(std::iter::once(**c)));
                    (matches && !keysyms.values().any(|got| *got == keysym)).then_some(keysym)
                })
        });
        match found {
            Some(keysym) => {
                keysyms.insert(marker, keysym);
            }
            None => without.push(marker),
        }
    }
    (keysyms, without)
}

/// How a [`Difference`] path shows the numpad decimal key.
pub(crate) const DECIMAL_SHOWN: &str = "the decimal key";

/// A distinct value of the written states with the first key giving it,
/// or the decimal key, and how a path shows it.
struct Pressable {
    value: Value,
    key: Key,
    keysym: Keysym,
    shown: String,
}

/// A value as a [`Difference`] path shows it.
pub(crate) fn show(keyboard: &Keyboard, value: &Value) -> String {
    match value {
        Value::Dead(marker) => format!("\\m{{{}}}", keyboard.marker_name(*marker).unwrap_or("?")),
        Value::Char(c) => format!("{:?}", c.to_string()),
        Value::Long(text) => format!("{text:?}"),
        Value::Mixed | Value::Empty => "a key mixing text and markers".to_owned(),
    }
}

struct Prober<'a> {
    keyboard: &'a Keyboard,
    model: Model,
    table: &'a ComposeTable,
    dead: &'a BTreeMap<MarkerIndex, Keysym>,
}

/// What the engine leaves after a press: committed text and the pending
/// markers; none when the key passes.
struct EngineStep {
    committed: String,
    pending: Vec<MarkerIndex>,
    state: State,
}

impl Prober<'_> {
    fn engine(&self, state: &State, key: &Key) -> Option<EngineStep> {
        let event = KeyEvent::new(key.clone());
        let (action, next) = self.model.key(state, &Context::default(), &event);
        let committed = match action {
            Action::Edit { insert, .. } => insert,
            Action::Pass => return None,
        };
        let pending = self
            .model
            .pending_markers(&next)
            .into_iter()
            .filter_map(|name| self.keyboard.marker_index(name))
            .collect();
        Some(EngineStep {
            committed,
            pending,
            state: next,
        })
    }

    fn marker_names(&self, markers: &[MarkerIndex]) -> String {
        markers
            .iter()
            .map(|marker| {
                format!(
                    "\\m{{{}}}",
                    self.keyboard.marker_name(*marker).unwrap_or("?")
                )
            })
            .collect::<Vec<_>>()
            .join("")
    }

    fn keysym_names(keysyms: &[Keysym]) -> String {
        keysyms
            .iter()
            .map(|k| k.name())
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// Runs every dead-key path of up to three keys through the engine and
/// through the reference table, from each dead key with a dead keysym.
/// Pressing a dead key from the reset state MUST commit nothing and leave
/// exactly its marker pending.
// [spec:kbdgen:sem:ldml.linux.dead-keys+2]
pub fn derive(keyboard: &Keyboard, levels: &Levels, layout: &str) -> Result<DeadKeys> {
    let (keysyms, without) = assign(keyboard, levels);
    let options = Options {
        output_form: OutputForm::Nfc,
        backspace: BackspacePolicy::CancelOrPass,
        host: Some(Host::Linux),
    };
    let model = Model::from_keyboard(keyboard.clone(), options).map_err(|error| {
        anyhow!("{layout}: the linux keyboard does not load in the engine: {error}")
    })?;
    let mut pressables: Vec<Pressable> = Vec::new();
    for key in &levels.keys {
        for (value, index) in key.values.iter().zip(key.keys) {
            let Some(index) = index else { continue };
            if *value == Value::Empty || pressables.iter().any(|p| p.value == *value) {
                continue;
            }
            let Some(key) = keyboard.key(index) else {
                continue;
            };
            pressables.push(Pressable {
                value: value.clone(),
                key: Key::Id {
                    id: key.id.clone(),
                    gesture: Gesture::Tap,
                },
                keysym: levels::keysym(keyboard, &keysyms, value),
                shown: show(keyboard, value),
            });
        }
    }
    if let Some(decimal) = levels::decimal(keyboard) {
        pressables.push(Pressable {
            value: Value::Empty,
            key: Key::Decimal,
            keysym: decimal.keysym,
            shown: DECIMAL_SHOWN.to_owned(),
        });
    }
    let prober = Prober {
        keyboard,
        model,
        table: ComposeTable::reference(),
        dead: &keysyms,
    };
    let mut differences = Vec::new();
    let mut explored: HashSet<Vec<Keysym>> = HashSet::new();
    let mut open: Vec<(Vec<String>, State, Composer)> = Vec::new();
    for pressable in &pressables {
        let Value::Dead(marker) = pressable.value else {
            continue;
        };
        let Some(keysym) = keysyms.get(&marker) else {
            continue;
        };
        let step = prober.engine(&State::default(), &pressable.key);
        let Some(step) = step.filter(|s| s.committed.is_empty() && s.pending == [marker]) else {
            bail!(
                "{layout}: dead key {}: pressing it does not leave exactly its marker pending",
                pressable.shown
            );
        };
        let composer = Composer {
            pending: vec![*keysym],
        };
        if explored.insert(composer.pending.clone()) {
            open.push((vec![pressable.shown.clone()], step.state, composer));
        }
    }
    while let Some((path, state, composer)) = open.pop() {
        for pressable in &pressables {
            let Some(step) = prober.engine(&state, &pressable.key) else {
                continue;
            };
            let mut next = composer.clone();
            let xkb_text = match next.press(prober.table, pressable.keysym) {
                Composed::Passed => pressable
                    .keysym
                    .to_char()
                    .map(String::from)
                    .unwrap_or_default(),
                Composed::Committed(text) => text,
                Composed::Pending | Composed::Dropped => String::new(),
            };
            let engine_pending: Option<Vec<Keysym>> = step
                .pending
                .iter()
                .map(|marker| prober.dead.get(marker).copied())
                .collect();
            let mut path = path.clone();
            path.push(pressable.shown.clone());
            let same = step.committed == xkb_text && engine_pending.as_ref() == Some(&next.pending);
            if !same {
                let engine = if step.pending.is_empty() {
                    format!("{:?}", step.committed)
                } else {
                    format!(
                        "{:?} and leaves {} pending",
                        step.committed,
                        prober.marker_names(&step.pending)
                    )
                };
                let xkb = if next.pending.is_empty() {
                    format!("types {xkb_text:?}")
                } else {
                    format!(
                        "types {xkb_text:?} and leaves {} pending",
                        Prober::keysym_names(&next.pending)
                    )
                };
                differences.push(Difference { path, engine, xkb });
            } else if !next.pending.is_empty()
                && path.len() < MAX_DEPTH
                && explored.insert(next.pending.clone())
            {
                open.push((path, step.state, next));
            }
        }
    }
    Ok(DeadKeys {
        keysyms,
        without,
        differences,
    })
}
