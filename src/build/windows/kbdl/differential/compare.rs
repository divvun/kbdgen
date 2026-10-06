//! The cases of `tsf.test.differential` and what each side makes of them.

use std::collections::{BTreeMap, HashSet};
use std::fmt;

use kbd_engine::{Action, Key, KeyEvent, Model, ModifierState, harness::Harness};

use super::super::input::{Layer, POSITION_COUNT, POSITION_NAMES};
use super::super::tables::POSITION_KEYS;
use super::Subject;
use super::exceptions::{Exception, explain};
use super::simulate::{Dll, Typed, WinKeys};

/// The space bar's scan code; its row is fixed by `kbdl.vk-chars`.
pub const SPACE_SCAN: u8 = 0x39;

/// The longest dead-key path compared, in keys.
pub const MAX_DEPTH: usize = 4;

/// One key press: an ISO position, or the space bar for `None`, in a
/// `kbdl.layers` layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Press {
    pub position: Option<usize>,
    pub layer: Layer,
}

impl Press {
    pub fn scan(self) -> u8 {
        self.position
            .map_or(SPACE_SCAN, |position| POSITION_KEYS[position].0)
    }

    /// What a TSF host sends the engine for this press (`tsf.keys.identity`,
    /// `ldml.kbdl.layers`): AltGr as `alt_r` with `altgr`, Ctrl as `ctrl_l`
    /// and the *i*-th extra modifier as `extra[i]`.
    pub fn event(self) -> KeyEvent {
        let altgr = ModifierState::altgr();
        let mut modifiers = match self.layer {
            Layer::Default | Layer::Extra(_) => ModifierState::default(),
            Layer::Shift | Layer::ExtraShift(_) => ModifierState::shift(),
            Layer::Ctrl => ModifierState {
                ctrl_l: true,
                ..ModifierState::default()
            },
            Layer::Alt => altgr,
            Layer::AltShift => ModifierState {
                shift_l: true,
                ..altgr
            },
            Layer::Caps => ModifierState {
                caps: true,
                ..ModifierState::default()
            },
            Layer::CapsShift => ModifierState {
                caps: true,
                ..ModifierState::shift()
            },
            Layer::AltCaps => ModifierState {
                caps: true,
                ..altgr
            },
        };
        if let Layer::Extra(i) | Layer::ExtraShift(i) = self.layer {
            modifiers.extra[usize::from(i)] = true;
        }
        KeyEvent::with(Key::Scan(self.scan()), modifiers)
    }

    /// The keyboard state `ToUnicodeEx` gets for this press.
    pub fn keys(self) -> WinKeys {
        let shift = matches!(
            self.layer,
            Layer::Shift | Layer::AltShift | Layer::CapsShift | Layer::ExtraShift(_)
        );
        let altgr = matches!(self.layer, Layer::Alt | Layer::AltShift | Layer::AltCaps);
        let mut extra = [false; 3];
        if let Layer::Extra(i) | Layer::ExtraShift(i) = self.layer {
            extra[usize::from(i)] = true;
        }
        WinKeys {
            shift,
            ctrl: altgr || self.layer == Layer::Ctrl,
            alt: altgr,
            extra,
            caps: matches!(self.layer, Layer::Caps | Layer::CapsShift | Layer::AltCaps),
        }
    }
}

impl fmt::Display for Press {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let key = self.position.map_or("space", |p| POSITION_NAMES[p]);
        write!(f, "{key} in {}", self.layer)
    }
}

/// The `kbdl.layers` layers of a layout with `extra` extra modifiers.
pub fn layers(extra: usize) -> Vec<Layer> {
    let mut layers = vec![
        Layer::Default,
        Layer::Shift,
        Layer::Ctrl,
        Layer::Alt,
        Layer::AltShift,
        Layer::Caps,
        Layer::CapsShift,
        Layer::AltCaps,
    ];
    for i in 0..extra.min(3) as u8 {
        layers.extend([Layer::Extra(i), Layer::ExtraShift(i)]);
    }
    layers
}

/// Every press of every layer: the 49 positions, then the space bar.
pub fn presses(extra: usize) -> Vec<Press> {
    let positions: Vec<Option<usize>> = (0..POSITION_COUNT).map(Some).chain([None]).collect();
    layers(extra)
        .into_iter()
        .flat_map(|layer| {
            positions
                .iter()
                .map(move |&position| Press { position, layer })
        })
        .collect()
}

/// What a sequence of presses leaves: the committed text, and the dead key
/// left pending, shown as the engine's preedit or the DLL's dead
/// character, which `kbdl.dead-keys` makes the standalone output.
#[derive(Debug, Clone, PartialEq, Eq)]
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

/// The engine's outcome as a TSF host applies it (`tsf.edit.ops`), and the
/// markers left pending: edits change the document, and a passed key first commits
/// what is pending (`tsf.edit.reset`) and then reaches the application,
/// not this text.
pub fn engine_run(model: &Model, path: &[Press]) -> (Outcome, Vec<String>) {
    let mut harness = Harness::new(model);
    for press in path {
        if *harness.send(&press.event()) == Action::Pass {
            harness.send(&KeyEvent::new(Key::Commit));
            harness.reset();
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
    (outcome, markers)
}

/// The simulated DLL's outcome: the units every call types, and whether
/// the kernel holds a dead character afterwards.
pub fn dll_outcome(dll: &mut Dll, path: &[Press]) -> Outcome {
    let mut units = Vec::new();
    for press in path {
        if let Typed::Text(typed) = dll.press(dll.vk(press.scan()), press.keys()) {
            units.extend(typed);
        }
    }
    Outcome {
        text: String::from_utf16_lossy(&units),
        pending: dll.pending().map(|id| String::from_utf16_lossy(&[id])),
    }
}

/// One compared sequence, with the state each side is left in.
#[derive(Debug, Clone)]
pub struct Case {
    pub path: Vec<Press>,
    pub engine: Outcome,
    pub dll: Outcome,
    states: (Option<u16>, Vec<String>),
}

impl Case {
    fn new(subject: &Subject, path: Vec<Press>) -> Case {
        let (engine, markers) = engine_run(&subject.model, &path);
        let mut dll = Dll::new(&subject.tables);
        let dll_outcome = dll_outcome(&mut dll, &path);
        Case {
            path,
            engine,
            dll: dll_outcome,
            states: (dll.pending(), markers),
        }
    }

    fn pending(&self) -> bool {
        self.engine.pending.is_some() || self.dll.pending.is_some()
    }

    pub fn name(&self) -> String {
        let keys: Vec<String> = self.path.iter().map(Press::to_string).collect();
        keys.join(", then ")
    }
}

/// Every single press from the reset state, then every dead-key path:
/// each case on which both sides agree and leave a dead key pending,
/// continued with
/// every press that types or leaves something pending alone, except Ctrl
/// presses, which the engine passes. A pair of pending dead states, the
/// DLL's dead character and the engine's pending markers, is continued from
/// its first path only: dead-key tables see nothing else of the text.
pub fn cases(subject: &Subject) -> Vec<Case> {
    let singles: Vec<Case> = presses(subject.input.extra_modifiers.len())
        .into_iter()
        .map(|press| Case::new(subject, vec![press]))
        .collect();
    let continuations: Vec<Press> = singles
        .iter()
        .filter(|case| case.path[0].layer != Layer::Ctrl)
        .filter(|case| case.pending() || !case.engine.text.is_empty() || !case.dll.text.is_empty())
        .map(|case| case.path[0])
        .collect();
    let mut explored = HashSet::new();
    let mut open: Vec<Vec<Press>> = Vec::new();
    for case in &singles {
        if case.pending() && case.engine == case.dll && explored.insert(case.states.clone()) {
            open.push(case.path.clone());
        }
    }
    let mut cases = singles;
    while let Some(prefix) = open.pop() {
        for press in &continuations {
            let path = [prefix.as_slice(), &[*press]].concat();
            let case = Case::new(subject, path);
            if case.pending()
                && case.engine == case.dll
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
    /// Records `case` with `dll` as the DLL's outcome.
    pub fn judge(&mut self, subject: &Subject, case: &Case, dll: &Outcome) {
        self.cases += 1;
        if case.engine == *dll {
            self.matched += 1;
            return;
        }
        let line = format!(
            "{}: {}: engine {}, DLL {dll}",
            subject.name,
            case.name(),
            case.engine
        );
        match explain(subject, &case.path, dll) {
            Some(exception) => {
                let (count, examples) = self.excepted.entry(exception).or_default();
                *count += 1;
                if examples.len() < 5 {
                    examples.push(line);
                }
            }
            None => self.unexplained.push(line),
        }
    }

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

/// Compares the engine with the simulated DLL on every case.
// [spec:kbdgen:req:tsf.test.differential+1]
pub fn compare(subject: &Subject) -> Report {
    let mut report = Report::default();
    for case in cases(subject) {
        report.judge(subject, &case, &case.dll);
    }
    report
}
