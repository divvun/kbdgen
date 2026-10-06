//! The dead-key states of a `.keylayout`, derived by running the engine
//! from each pending marker over every value of the key maps, so that the
//! states compose exactly as the engine does wherever a keylayout can.

use anyhow::{Result, anyhow, bail};
use indexmap::IndexMap;
use kbd_engine::{
    Action, BackspacePolicy, Context, Gesture, Key, KeyEvent, Model, Options, OutputForm, State,
};
use kbd_model::{Atom, Host, Keyboard, MarkerIndex, TransformGroup};

use super::layers::{Derived, Press, Value};
use crate::build::macos::input::{NONE_STATE, When};
use crate::build::windows::kbdl::diag::Diagnostics;

/// A dead-key state: its marker, id and terminator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeadState {
    pub marker: MarkerIndex,
    pub id: String,
    pub terminator: String,
}

/// The states, and per value the `when`s it adds for them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeadKeys {
    pub states: Vec<DeadState>,
    /// Indexed like `Derived::values`, each in state order.
    pub whens: Vec<Vec<When>>,
    /// One line per result a keylayout cannot express.
    pub omitted: Vec<String>,
}

impl DeadKeys {
    pub fn id(&self, marker: MarkerIndex) -> &str {
        self.states
            .iter()
            .find(|state| state.marker == marker)
            .map_or(NONE_STATE, |state| state.id.as_str())
    }
}

/// What the engine did with one press.
struct Outcome {
    committed: String,
    pending: Vec<MarkerIndex>,
    state: State,
}

/// A result for a state and a value that differs from the keylayout's
/// default.
enum Transition {
    Output(String),
    Next(MarkerIndex),
}

struct Prober<'a> {
    model: Model,
    keyboard: &'a Keyboard,
}

impl Prober<'_> {
    fn press(&self, state: &State, press: Press) -> Outcome {
        let key = match press {
            Press::Key(index) => Key::Id {
                id: self
                    .keyboard
                    .key(index)
                    .map(|key| key.id.clone())
                    .unwrap_or_default(),
                gesture: Gesture::Tap,
            },
            Press::Decimal => Key::Decimal,
        };
        let (action, next) = self
            .model
            .key(state, &Context::default(), &KeyEvent::new(key));
        let committed = match action {
            Action::Edit { insert, .. } => insert,
            Action::Pass => String::new(),
        };
        let pending = self
            .model
            .pending_markers(&next)
            .into_iter()
            .filter_map(|name| self.keyboard.marker_index(name))
            .collect();
        Outcome {
            committed,
            pending,
            state: next,
        }
    }

    fn marker(&self, marker: MarkerIndex) -> String {
        format!(
            "\\m{{{}}}",
            self.keyboard.marker_name(marker).unwrap_or("?")
        )
    }
}

/// The text a state's terminator types: the marker's flush output.
fn terminator(keyboard: &Keyboard, marker: MarkerIndex) -> String {
    keyboard.flush.get(&marker).cloned().unwrap_or_default()
}

/// The markers in the order they first start a `from` pattern of the
/// `simple` groups (for v4, `deadKeys` order), then the rest in marker
/// order.
// [spec:kbdgen:sem:ldml.macos.dead-keys]
fn marker_order(keyboard: &Keyboard) -> Vec<MarkerIndex> {
    let mut order: Vec<MarkerIndex> = Vec::new();
    for group in &keyboard.simple {
        let TransformGroup::Rules(rules) = group else {
            continue;
        };
        for rule in rules {
            let lead = rule.from.nodes.first().and_then(|node| {
                node.alternatives
                    .first()
                    .and_then(|sequence| sequence.first())
            });
            if let Some(Atom::Marker(marker)) = lead.map(|item| item.atom)
                && !order.contains(&marker)
            {
                order.push(marker);
            }
        }
    }
    let rest = (0..keyboard.markers.len()).filter_map(|i| MarkerIndex::try_from(i).ok());
    let rest: Vec<MarkerIndex> = rest.filter(|m| !order.contains(m)).collect();
    order.extend(rest);
    order
}

/// The transition from the state of `marker` on `value`, if it differs
/// from the keylayout's default: the terminator then the value's own
/// `none` result. A result with committed text and a marker pending, or
/// several markers pending, is omitted with a warning.
// [spec:kbdgen:sem:ldml.macos.dead-keys]
fn transition(
    prober: &Prober,
    marker: MarkerIndex,
    value: &Value,
    outcome: &Outcome,
    out: &mut DeadKeys,
    diag: &mut Diagnostics,
) -> Option<Transition> {
    let flush = terminator(prober.keyboard, marker);
    let (text, pending) = match value {
        Value::Text(text) => (format!("{flush}{text}"), vec![]),
        Value::Dead(dead) => (flush, vec![*dead]),
    };
    if outcome.committed == text && outcome.pending == pending {
        return None;
    }
    match outcome.pending[..] {
        [] => Some(Transition::Output(outcome.committed.clone())),
        [next] if outcome.committed.is_empty() => Some(Transition::Next(next)),
        _ => {
            let shown = match value {
                Value::Text(text) => format!("{text:?}"),
                Value::Dead(dead) => prober.marker(*dead),
            };
            let line = format!(
                "dead key {} then {shown}: the engine types {:?} and leaves {} marker(s) pending",
                prober.marker(marker),
                outcome.committed,
                outcome.pending.len()
            );
            diag.warn(format!(
                "{line}, which a .keylayout cannot express; omitted"
            ));
            out.omitted.push(line);
            None
        }
    }
}

/// Every dead-key state reachable from the dead keys of the key maps, and
/// the `when`s of every value. Pressing a dead key from the reset state
/// MUST commit nothing and leave exactly its marker pending.
// [spec:kbdgen:sem:ldml.macos.dead-keys]
pub fn derive(keyboard: &Keyboard, derived: &Derived, diag: &mut Diagnostics) -> Result<DeadKeys> {
    let options = Options {
        output_form: OutputForm::Nfc,
        backspace: BackspacePolicy::CancelOrPass,
        host: Some(Host::MacOs),
    };
    let model = Model::from_keyboard(keyboard.clone(), options).map_err(|error| {
        anyhow!(
            "{}: the macOS keyboard does not load in the engine: {error}",
            diag.layout()
        )
    })?;
    let prober = Prober { model, keyboard };
    let mut found: IndexMap<MarkerIndex, State> = IndexMap::new();
    for entry in &derived.values {
        let Value::Dead(marker) = entry.value else {
            continue;
        };
        let pressed = prober.press(&State::default(), entry.press);
        if pressed.pending != [marker] || !pressed.committed.is_empty() {
            bail!(
                "{}: dead key {}: pressing it does not leave exactly its marker pending",
                diag.layout(),
                prober.marker(marker)
            );
        }
        found.entry(marker).or_insert(pressed.state);
    }
    let mut out = DeadKeys {
        whens: vec![Vec::new(); derived.values.len()],
        ..DeadKeys::default()
    };
    let mut raw: Vec<Vec<(MarkerIndex, Transition)>> =
        (0..derived.values.len()).map(|_| Vec::new()).collect();
    let mut index = 0;
    while let Some((marker, state)) = found.get_index(index).map(|(m, s)| (*m, s.clone())) {
        for (v, entry) in derived.values.iter().enumerate() {
            let outcome = prober.press(&state, entry.press);
            let Some(step) = transition(&prober, marker, &entry.value, &outcome, &mut out, diag)
            else {
                continue;
            };
            if let Transition::Next(next) = step {
                found.entry(next).or_insert(outcome.state);
            }
            raw[v].push((marker, step));
        }
        index += 1;
    }
    let order = marker_order(keyboard);
    let mut markers: Vec<MarkerIndex> = found.keys().copied().collect();
    markers.sort_by_key(|m| order.iter().position(|o| o == m));
    out.states = markers
        .iter()
        .enumerate()
        .map(|(i, marker)| DeadState {
            marker: *marker,
            id: format!("dead_key{i:03}"),
            terminator: terminator(keyboard, *marker),
        })
        .collect();
    for (v, mut steps) in raw.into_iter().enumerate() {
        steps.sort_by_key(|(m, _)| markers.iter().position(|o| o == m));
        out.whens[v] = steps
            .into_iter()
            .map(|(m, step)| match step {
                Transition::Output(text) => When::output(out.id(m), text),
                Transition::Next(next) => When::next(out.id(m), out.id(next)),
            })
            .collect();
    }
    Ok(out)
}
