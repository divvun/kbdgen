//! The dead-key tree of `kbdl.input`, derived by running the engine over
//! each dead key followed by every value of the layers, so the DLL's dead
//! keys compose exactly as the engine does wherever the tables can.

use anyhow::{Result, anyhow, bail};
use indexmap::IndexMap;
use kbd_engine::{
    Action, BackspacePolicy, Context, Gesture, Key, KeyEvent, Model, Options, OutputForm, State,
};
use kbd_model::{Host, KeyIndex, Keyboard, MarkerIndex};

use super::super::{
    diag::Diagnostics,
    input::{DeadKeyNode, STANDALONE},
};
use super::layers::DerivedLayers;

/// What the engine did with one key in a dead-key state.
struct Outcome {
    committed: String,
    pending: Vec<MarkerIndex>,
    state: State,
}

/// Runs the engine for the dead-key states of one keyboard.
struct Deriver<'a> {
    model: Model,
    keyboard: &'a Keyboard,
    layers: &'a DerivedLayers,
}

/// The tree, keyed by each dead key's identity, in first-occurrence order.
// [spec:kbdgen:sem:ldml.kbdl.dead-tree+1]
pub fn derive(
    keyboard: &Keyboard,
    layers: &DerivedLayers,
    diag: &mut Diagnostics,
) -> Result<IndexMap<String, DeadKeyNode>> {
    let options = Options {
        output_form: OutputForm::Nfc,
        backspace: BackspacePolicy::CancelOrPass,
        host: Some(Host::Windows),
    };
    let model = Model::from_keyboard(keyboard.clone(), options).map_err(|error| {
        anyhow!(
            "{}: the windows keyboard does not load in the engine: {error}",
            diag.layout()
        )
    })?;
    let deriver = Deriver {
        model,
        keyboard,
        layers,
    };
    let mut tree = IndexMap::new();
    for dead in &layers.dead_keys {
        let pressed = deriver.press(&State::default(), dead.key);
        if pressed.pending != [dead.marker] || !pressed.committed.is_empty() {
            bail!(
                "{}: dead key {:?}: pressing it does not leave exactly its marker \\m{{{}}} pending",
                diag.layout(),
                dead.identity,
                keyboard.marker_name(dead.marker).unwrap_or("?")
            );
        }
        let path = vec![dead.marker];
        let branch = deriver.branch(&pressed.state, &path, &dead.identity, diag)?;
        tree.insert(dead.identity.clone(), DeadKeyNode::Branch(branch));
    }
    Ok(tree)
}

impl Deriver<'_> {
    /// The outcome of `key` pressed by id from `state` with an empty
    /// context.
    fn press(&self, state: &State, key: KeyIndex) -> Outcome {
        let id = self
            .keyboard
            .key(key)
            .map(|key| key.id.clone())
            .unwrap_or_default();
        self.run(
            state,
            Key::Id {
                id,
                gesture: Gesture::Tap,
            },
        )
    }

    fn run(&self, state: &State, key: Key) -> Outcome {
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

    /// The children of the dead-key state `state`, reached through the
    /// markers of `path`, the last of which is pending.
    // [spec:kbdgen:sem:ldml.kbdl.dead-tree+1]
    fn branch(
        &self,
        state: &State,
        path: &[MarkerIndex],
        shown: &str,
        diag: &mut Diagnostics,
    ) -> Result<IndexMap<String, DeadKeyNode>> {
        let marker = *path.last().ok_or_else(|| anyhow!("empty dead-key path"))?;
        let flush = self.keyboard.flush.get(&marker).map_or("", String::as_str);
        let mut children = IndexMap::new();
        for value in &self.layers.values {
            let input = &value.value.text;
            if input == STANDALONE || children.contains_key(input) {
                continue;
            }
            let outcome = self.press(state, value.key);
            let unmatched = if value.value.dead {
                flush.to_owned()
            } else {
                format!("{flush}{input}")
            };
            let child_path = format!("{shown} {input}");
            let chained = match outcome.pending[..] {
                [next] if outcome.committed.is_empty() => Some(next),
                _ => None,
            };
            let node = match chained {
                Some(next) if !path.contains(&next) => {
                    let mut deeper = path.to_vec();
                    deeper.push(next);
                    let branch = self.branch(&outcome.state, &deeper, &child_path, diag)?;
                    Some(DeadKeyNode::Branch(branch))
                }
                _ if outcome.committed == unmatched => None,
                Some(next) => bail!(
                    "{}: dead key {child_path:?} returns to the pending dead key \\m{{{}}}, a cycle",
                    diag.layout(),
                    self.keyboard.marker_name(next).unwrap_or("?")
                ),
                None if outcome.pending.is_empty() => Some(DeadKeyNode::Leaf(outcome.committed)),
                None => {
                    diag.warn(format!(
                        "dead key {child_path:?}: the engine types {:?} and keeps {} marker(s) pending, which the DLL cannot express; omitted",
                        outcome.committed,
                        outcome.pending.len()
                    ));
                    None
                }
            };
            if let Some(node) = node {
                children.insert(input.clone(), node);
            }
        }
        let standalone = self.run(state, Key::Emit(STANDALONE.to_owned()));
        if !standalone.pending.is_empty() {
            diag.warn(format!(
                "dead key {shown:?}: space leaves a marker pending; its standalone output is {:?}",
                standalone.committed
            ));
        }
        children.insert(
            STANDALONE.to_owned(),
            DeadKeyNode::Leaf(standalone.committed),
        );
        Ok(children)
    }
}
