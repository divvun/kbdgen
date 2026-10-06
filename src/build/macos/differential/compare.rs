//! The cases of `ldml.macos.test.typing` and what each side makes of them.

use std::collections::{BTreeMap, HashSet};
use std::fmt;

use kbd_engine::{Action, Key, KeyEvent, Model, ModifierState, harness::Harness};
use kbd_model::ExtraModifierKey;

use super::exceptions::{Exception, explain};
use super::{Keylayout, Mods, Subject};
use crate::build::macos::adapter::{SPACE, positions};
use crate::build::macos::input::Binding;

/// The longest dead-key path compared, in keys.
pub const MAX_DEPTH: usize = 3;

/// One key press: a key and the modifiers held.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Press {
    pub name: &'static str,
    pub scan: u8,
    pub code: u16,
    pub mods: Mods,
}

impl Press {
    /// What a macOS host sends the engine: each modifier by side, Option
    /// as Alt, no AltGr.
    pub fn event(self) -> KeyEvent {
        let m = self.mods;
        let modifiers = ModifierState {
            shift_l: m.shift_l,
            shift_r: m.shift_r,
            caps: m.caps,
            ctrl_l: m.control_l,
            ctrl_r: m.control_r,
            alt_l: m.option_l,
            alt_r: m.option_r,
            cmd: m.command,
            ..ModifierState::default()
        };
        KeyEvent::with(Key::Scan(self.scan), modifiers)
    }
}

impl fmt::Display for Press {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let m = self.mods;
        let held: Vec<&str> = [
            (m.shift_l, "shiftL"),
            (m.shift_r, "shiftR"),
            (m.caps, "caps"),
            (m.option_l, "optionL"),
            (m.option_r, "optionR"),
            (m.control_l, "controlL"),
            (m.control_r, "controlR"),
        ]
        .into_iter()
        .filter_map(|(on, name)| on.then_some(name))
        .collect();
        write!(f, "{} with [{}]", self.name, held.join(" "))
    }
}

/// Every modifier state compared: Shift, Caps Lock, Option and Control by
/// side, never Command, whose presses the engine passes. Right Control is
/// left out when it is bound as an extra modifier.
pub fn modifier_states(subject: &Subject) -> Vec<Mods> {
    let right_ctrl = !subject
        .keyboard
        .windows
        .extra_modifiers
        .contains(&ExtraModifierKey::RightCtrl);
    let sides = [(false, false), (true, false), (false, true)];
    let mut states = Vec::new();
    for (shift_l, shift_r) in sides {
        for caps in [false, true] {
            for (option_l, option_r) in sides.into_iter().chain([(true, true)]) {
                for (control_l, control_r) in sides {
                    if control_r && !right_ctrl {
                        continue;
                    }
                    states.push(Mods {
                        shift_l,
                        shift_r,
                        caps,
                        option_l,
                        option_r,
                        control_l,
                        control_r,
                        command: false,
                    });
                }
            }
        }
    }
    states
}

/// Every press: each modifier state at each ISO position and the space bar.
pub fn presses(subject: &Subject) -> Vec<Press> {
    let keys: Vec<(&'static str, u8, u16)> =
        positions().chain([("space", SPACE.0, SPACE.1)]).collect();
    modifier_states(subject)
        .into_iter()
        .flat_map(|mods| {
            keys.iter().map(move |&(name, scan, code)| Press {
                name,
                scan,
                code,
                mods,
            })
        })
        .collect()
}

/// What a sequence of presses leaves: the committed text and the pending
/// dead key's shown text.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Outcome {
    pub text: String,
    pub pending: Option<String>,
}

impl fmt::Display for Outcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.text)?;
        if let Some(pending) = &self.pending {
            write!(f, " with {pending:?} pending")?;
        }
        Ok(())
    }
}

/// The engine's outcome and pending markers, or `None` when a press
/// passes: macOS then handles the key itself, which is the `.keylayout`.
pub fn engine_run(model: &Model, path: &[Press]) -> Option<(Outcome, Vec<String>)> {
    let mut harness = Harness::new(model);
    for press in path {
        if *harness.send(&press.event()) == Action::Pass {
            return None;
        }
    }
    let markers: Vec<String> = model
        .pending_markers(harness.state())
        .into_iter()
        .map(str::to_owned)
        .collect();
    let outcome = Outcome {
        text: harness.document().to_owned(),
        pending: (!markers.is_empty()).then(|| harness.preedit().to_owned()),
    };
    Some((outcome, markers))
}

/// The simulated `.keylayout`'s outcome and its pending state.
pub fn keylayout_run(subject: &Subject, path: &[Press]) -> (Outcome, Option<String>) {
    let mut keylayout = Keylayout::new(&subject.parsed).unwrap();
    let mut text = String::new();
    for press in path {
        text.push_str(&keylayout.press(press.code, press.mods));
    }
    let pending = keylayout.pending();
    let state = keylayout.state.clone();
    (Outcome { text, pending }, state)
}

/// One compared sequence, with the state each side is left in.
#[derive(Debug, Clone)]
pub struct Case {
    pub path: Vec<Press>,
    pub engine: Outcome,
    pub keylayout: Outcome,
    states: (Option<String>, Vec<String>),
}

impl Case {
    fn new(subject: &Subject, path: Vec<Press>) -> Option<Case> {
        let (engine, markers) = engine_run(&subject.model, &path)?;
        let (keylayout, state) = keylayout_run(subject, &path);
        Some(Case {
            path,
            engine,
            keylayout,
            states: (state, markers),
        })
    }

    fn pending(&self) -> bool {
        self.engine.pending.is_some() || self.keylayout.pending.is_some()
    }

    pub fn name(&self) -> String {
        let keys: Vec<String> = self.path.iter().map(Press::to_string).collect();
        keys.join(", then ")
    }
}

/// What a press does from the reset state on each side, which decides
/// how it continues a dead-key path.
fn signature(subject: &Subject, case: &Case) -> (Option<Binding>, Outcome) {
    let press = case.path[0];
    let keylayout = Keylayout::new(&subject.parsed).unwrap();
    let binding = keylayout
        .select(press.mods)
        .and_then(|map| map.keys.iter().find(|k| k.code == press.code))
        .map(|key| key.binding.clone());
    (binding, case.engine.clone())
}

/// Every single press the engine types, then every dead-key path: each
/// case on which both sides agree and leave a dead key pending, continued
/// with one press of each distinct kind, up to [`MAX_DEPTH`] keys.
pub fn cases(subject: &Subject) -> Vec<Case> {
    let singles: Vec<Case> = presses(subject)
        .into_iter()
        .filter_map(|press| Case::new(subject, vec![press]))
        .collect();
    let mut kinds = HashSet::new();
    let continuations: Vec<Press> = singles
        .iter()
        .filter(|case| {
            case.pending() || !case.engine.text.is_empty() || !case.keylayout.text.is_empty()
        })
        .filter(|case| kinds.insert(signature(subject, case)))
        .map(|case| case.path[0])
        .collect();
    let mut explored = HashSet::new();
    let mut open: Vec<Vec<Press>> = singles
        .iter()
        .filter(|case| {
            case.pending() && case.engine == case.keylayout && explored.insert(case.states.clone())
        })
        .map(|case| case.path.clone())
        .collect();
    let mut cases = singles;
    while let Some(prefix) = open.pop() {
        for press in &continuations {
            let path = [prefix.as_slice(), &[*press]].concat();
            let Some(case) = Case::new(subject, path) else {
                continue;
            };
            if case.pending()
                && case.engine == case.keylayout
                && case.path.len() < MAX_DEPTH
                && explored.insert(case.states.clone())
            {
                open.push(case.path.clone());
            }
            cases.push(case);
        }
    }
    cases
}

/// The result of comparing one layout.
#[derive(Debug, Default)]
pub struct Report {
    pub cases: usize,
    pub matched: usize,
    /// Cases each exception explains, with up to five examples.
    pub excepted: BTreeMap<Exception, (usize, Vec<String>)>,
    pub unexplained: Vec<String>,
}

impl Report {
    pub fn summary(&self) -> String {
        let mut lines = vec![format!(
            "{} cases, {} equal, {} unexplained",
            self.cases,
            self.matched,
            self.unexplained.len()
        )];
        for (exception, (count, examples)) in &self.excepted {
            lines.push(format!("  {count} × {exception:?}, e.g. {}", examples[0]));
        }
        lines.join("\n")
    }
}

/// Compares the engine with the simulated `.keylayout` on every case. A
/// typing state that selects more than one key map is reported too.
// [spec:kbdgen:req:ldml.macos.test.typing]
pub fn compare(subject: &Subject) -> Report {
    let mut report = Report::default();
    let keylayout = Keylayout::new(&subject.parsed).unwrap();
    let typing = |m: &Mods| m.option_l || m.option_r || !(m.control_l || m.control_r);
    for mods in modifier_states(subject).into_iter().filter(typing) {
        if keylayout.match_count(mods) > 1 {
            report.unexplained.push(format!(
                "{}: {mods:?} selects {} key maps",
                subject.label,
                keylayout.match_count(mods)
            ));
        }
    }
    for case in cases(subject) {
        report.cases += 1;
        if case.engine == case.keylayout {
            report.matched += 1;
            continue;
        }
        let line = format!(
            "{}: {}: engine {}, keylayout {}",
            subject.label,
            case.name(),
            case.engine,
            case.keylayout
        );
        match explain(subject, &case.path, &case.keylayout) {
            Some(exception) => {
                let (count, examples) = report.excepted.entry(exception).or_default();
                *count += 1;
                if examples.len() < 5 {
                    examples.push(line);
                }
            }
            None => report.unexplained.push(line),
        }
    }
    report
}
