//! The cases of `ldml.linux.test.typing` and what each side makes of them.

use std::collections::{BTreeMap, HashSet};
use std::fmt;

use kbd_engine::{Action, Key, KeyEvent, Model, ModifierState, harness::Harness};

use super::Subject;
use super::exceptions::{Exception, explain};
use crate::build::linux::xkb::compose::{ComposeTable, Composed, Composer};
use crate::build::linux::xkb::keycodes::{DECIMAL, KEYS};
use crate::build::linux::xkb::keysym::Keysym;
use crate::build::linux::xkb::levels::modifiers;
use crate::build::linux::xkb::types::{LEVEL3, NUM_LOCK, XkbState};

/// The longest dead-key path compared, in keys, before the space bar ends
/// it.
pub const MAX_DEPTH: usize = 3;

/// One key press: a written key in one of its eight states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Press {
    pub name: &'static str,
    pub scan: u8,
    pub state: XkbState,
}

impl Press {
    pub fn is_decimal(self) -> bool {
        self.name == DECIMAL.name
    }

    /// What the IBus engine sends the engine (`linux.ibus.keys`,
    /// `linux.ibus.modifiers`): the scan code, or `Decimal` with Num Lock
    /// on; Right Alt as AltGr when the model uses Level3, else as Right
    /// Alt.
    pub fn event(self, uses_level3: bool) -> KeyEvent {
        let mut m: ModifierState = modifiers(self.state);
        if !uses_level3 {
            m.altgr = false;
        }
        let key = if self.is_decimal() {
            Key::Decimal
        } else {
            Key::Scan(self.scan)
        };
        KeyEvent::with(key, m)
    }
}

impl fmt::Display for Press {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.state.mods() == 0 {
            write!(f, "<{}>", self.name)
        } else {
            write!(f, "<{}> with {}", self.name, self.state)
        }
    }
}

/// Every press: each written key in each of the eight states, or the four
/// without Level3 when the model does not use it, and `<KPDL>` with Num
/// Lock on in those without Shift. xkeyboard-config's `KEYPAD` gave
/// Shift+Num Lock Level1 until it dropped Shift from the type (2.42 still
/// does, 2.48 not), so that state depends on the installed version.
pub fn presses(subject: &Subject) -> Vec<Press> {
    let uses_level3 = subject.adapted.levels.uses_level3;
    let mut keys: Vec<(&'static str, u8)> = KEYS
        .iter()
        .filter(|key| subject.parsed.key(key.name).is_some())
        .map(|key| (key.name, key.scan))
        .collect();
    if subject.parsed.key(DECIMAL.name).is_some() {
        keys.push((DECIMAL.name, DECIMAL.scan));
    }
    XkbState::ALL
        .into_iter()
        .filter(|state| uses_level3 || !state.level3())
        .flat_map(|state| {
            keys.iter()
                .map(move |&(name, scan)| Press { name, scan, state })
        })
        .filter(|press| !(press.is_decimal() && press.state.shift()))
        .collect()
}

/// The engine's committed text and pending markers, or `None` when a press
/// passes.
pub fn engine_run(
    model: &Model,
    path: &[Press],
    uses_level3: bool,
) -> Option<(String, Vec<String>)> {
    let mut harness = Harness::new(model);
    for press in path {
        if *harness.send(&press.event(uses_level3)) == Action::Pass {
            return None;
        }
    }
    let markers = model
        .pending_markers(harness.state())
        .into_iter()
        .map(str::to_owned)
        .collect();
    Some((harness.document().to_owned(), markers))
}

/// The keysym the read-back XKB layout gives a press, as libxkbcommon
/// computes it (`ldml.linux.semantics`); none when Right Alt is Alt and the
/// press holds it, which types no text.
pub fn keysym(subject: &Subject, press: Press) -> Option<Keysym> {
    let key = subject.parsed.key(press.name)?;
    if press.state.level3() && !subject.parsed.level3 {
        return None;
    }
    let mut mods = press.state.mods();
    if press.is_decimal() {
        mods |= NUM_LOCK;
    }
    if !subject.parsed.level3 {
        mods &= !LEVEL3;
    }
    Some(key.key_type.resolve(&key.levels, mods))
}

/// What the XKB layout types on `path` through IBus's simple engine, and
/// the sequence left pending.
pub fn xkb_run(subject: &Subject, path: &[Press]) -> (String, Vec<Keysym>) {
    let table = ComposeTable::reference();
    let mut composer = Composer::default();
    let mut text = String::new();
    for press in path {
        let Some(keysym) = keysym(subject, *press) else {
            continue;
        };
        match composer.press(table, keysym) {
            Composed::Passed => text.extend(keysym.to_char().filter(|c| !c.is_control())),
            Composed::Committed(committed) => text.push_str(&committed),
            Composed::Pending | Composed::Dropped => {}
        }
    }
    (text, composer.pending)
}

/// One compared sequence.
#[derive(Debug, Clone)]
pub struct Case {
    pub path: Vec<Press>,
    pub engine: String,
    pub xkb: String,
    states: (Vec<String>, Vec<Keysym>),
}

impl Case {
    pub fn new(subject: &Subject, path: Vec<Press>) -> Option<Case> {
        let uses_level3 = subject.adapted.levels.uses_level3;
        let (engine, markers) = engine_run(&subject.model, &path, uses_level3)?;
        let (xkb, pending) = xkb_run(subject, &path);
        Some(Case {
            path,
            engine,
            xkb,
            states: (markers, pending),
        })
    }

    fn pending(&self) -> bool {
        !self.states.0.is_empty() || !self.states.1.is_empty()
    }

    pub fn name(&self) -> String {
        let keys: Vec<String> = self.path.iter().map(Press::to_string).collect();
        keys.join(", then ")
    }
}

/// Every single press the engine handles, then every dead-key path: each
/// case on which both sides agree and leave something pending, continued
/// with one press of each distinct kind, up to [`MAX_DEPTH`] keys, and
/// each such path also ended by the space bar.
pub fn cases(subject: &Subject) -> Vec<Case> {
    let singles: Vec<Case> = presses(subject)
        .into_iter()
        .filter_map(|press| Case::new(subject, vec![press]))
        .collect();
    let mut kinds = HashSet::new();
    let continuations: Vec<Press> = singles
        .iter()
        .filter(|case| case.pending() || !case.engine.is_empty() || !case.xkb.is_empty())
        .filter(|case| {
            let press = case.path[0];
            kinds.insert((
                keysym(subject, press),
                case.engine.clone(),
                case.states.0.clone(),
            ))
        })
        .map(|case| case.path[0])
        .collect();
    let space = Press {
        name: "SPCE",
        scan: 0x39,
        state: XkbState::ALL[0],
    };
    let mut explored = HashSet::new();
    let mut open: Vec<Vec<Press>> = singles
        .iter()
        .filter(|case| {
            case.pending() && case.engine == case.xkb && explored.insert(case.states.clone())
        })
        .map(|case| case.path.clone())
        .collect();
    let mut cases = singles;
    while let Some(prefix) = open.pop() {
        let ended = [prefix.as_slice(), &[space]].concat();
        if let Some(case) = Case::new(subject, ended) {
            cases.push(case);
        }
        if prefix.len() >= MAX_DEPTH {
            continue;
        }
        for press in &continuations {
            let path = [prefix.as_slice(), &[*press]].concat();
            let Some(case) = Case::new(subject, path) else {
                continue;
            };
            if case.pending() && case.engine == case.xkb && explored.insert(case.states.clone()) {
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

/// Compares the engine with the simulated XKB layout on every case.
// [spec:kbdgen:req:ldml.linux.test.typing+1]
pub fn compare(subject: &Subject) -> Report {
    let mut report = Report::default();
    for case in cases(subject) {
        report.cases += 1;
        if case.engine == case.xkb {
            report.matched += 1;
            continue;
        }
        let line = format!(
            "{}: {}: engine {:?}, XKB {:?}",
            subject.label,
            case.name(),
            case.engine,
            case.xkb
        );
        match explain(subject, &case) {
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
