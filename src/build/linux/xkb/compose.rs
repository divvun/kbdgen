//! The reference Compose table, `data/Compose`, and composition as IBus's
//! simple engine does it (`ldml.linux.compose`), which GNOME runs for every
//! XKB source.

use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use icu_normalizer::ComposingNormalizerBorrowed;

use super::keysym::Keysym;

/// The dead keysyms of `ibus_keysym_to_unicode` (`src/ibuscomposetable.c`),
/// in its order, each with its combining character and its isolated one.
pub const DEAD_CHARACTERS: [(&str, char, char); 49] = [
    ("dead_a", '\u{3041}', '\u{3041}'),
    ("dead_A", '\u{3042}', '\u{3042}'),
    ("dead_i", '\u{3043}', '\u{3043}'),
    ("dead_I", '\u{3044}', '\u{3044}'),
    ("dead_u", '\u{3045}', '\u{3045}'),
    ("dead_U", '\u{3046}', '\u{3046}'),
    ("dead_e", '\u{3047}', '\u{3047}'),
    ("dead_E", '\u{3048}', '\u{3048}'),
    ("dead_o", '\u{3049}', '\u{3049}'),
    ("dead_O", '\u{304A}', '\u{304A}'),
    ("dead_abovecomma", '\u{0313}', '\u{02BC}'),
    ("dead_abovedot", '\u{0307}', '\u{02D9}'),
    ("dead_abovereversedcomma", '\u{0314}', '\u{02BD}'),
    ("dead_abovering", '\u{030A}', '\u{02DA}'),
    ("dead_aboveverticalline", '\u{030D}', '\u{02C8}'),
    ("dead_acute", '\u{0301}', '\u{00B4}'),
    ("dead_belowbreve", '\u{032E}', '\u{032E}'),
    ("dead_belowcircumflex", '\u{032D}', '\u{A788}'),
    ("dead_belowcomma", '\u{0326}', '\u{002C}'),
    ("dead_belowdiaeresis", '\u{0324}', '\u{0324}'),
    ("dead_belowdot", '\u{0323}', '\u{002E}'),
    ("dead_belowmacron", '\u{0331}', '\u{02CD}'),
    ("dead_belowring", '\u{030A}', '\u{02F3}'),
    ("dead_belowtilde", '\u{0330}', '\u{02F7}'),
    ("dead_belowverticalline", '\u{0329}', '\u{02CC}'),
    ("dead_breve", '\u{0306}', '\u{02D8}'),
    ("dead_capital_schwa", '\u{1DEA}', '\u{1D4A}'),
    ("dead_caron", '\u{030C}', '\u{02C7}'),
    ("dead_cedilla", '\u{0327}', '\u{00B8}'),
    ("dead_circumflex", '\u{0302}', '\u{005E}'),
    ("dead_currency", '\u{00A4}', '\u{00A4}'),
    ("dead_diaeresis", '\u{0308}', '\u{00A8}'),
    ("dead_doubleacute", '\u{030B}', '\u{02DD}'),
    ("dead_doublegrave", '\u{030F}', '\u{02F5}'),
    ("dead_grave", '\u{0300}', '\u{0060}'),
    ("dead_greek", '\u{03BC}', '\u{03BC}'),
    ("dead_hook", '\u{0309}', '\u{02C0}'),
    ("dead_horn", '\u{031B}', '\u{031B}'),
    ("dead_invertedbreve", '\u{032F}', '\u{032F}'),
    ("dead_iota", '\u{0345}', '\u{037A}'),
    ("dead_longsolidusoverlay", '\u{0338}', '\u{0338}'),
    ("dead_lowline", '\u{0332}', '\u{005F}'),
    ("dead_macron", '\u{0304}', '\u{00AF}'),
    ("dead_ogonek", '\u{0328}', '\u{02DB}'),
    ("dead_semivoiced_sound", '\u{309A}', '\u{309C}'),
    ("dead_small_schwa", '\u{1DEA}', '\u{1D4A}'),
    ("dead_stroke", '\u{0335}', '\u{0335}'),
    ("dead_tilde", '\u{0303}', '\u{007E}'),
    ("dead_voiced_sound", '\u{3099}', '\u{309B}'),
];

/// The combining character IBus composes a dead keysym with, if it has one.
fn combining(keysym: Keysym) -> Option<char> {
    DEAD_CHARACTERS
        .iter()
        .find(|(name, ..)| Keysym::from_name(name) == Some(keysym))
        .map(|(_, combining, _)| *combining)
}

/// The parsed table: each sequence's output, and every proper prefix of a
/// sequence.
pub struct ComposeTable {
    outputs: HashMap<Vec<Keysym>, String>,
    prefixes: HashSet<Vec<Keysym>>,
    /// Lines whose keysyms or output did not parse.
    pub skipped: usize,
}

/// The output of a Compose line's quoted string, unescaped.
fn quoted(rest: &str) -> Option<String> {
    let rest = rest.trim_start().strip_prefix('"')?;
    let mut out = String::new();
    let mut chars = rest.chars();
    loop {
        match chars.next()? {
            '"' => return Some(out),
            '\\' => out.push(chars.next()?),
            c => out.push(c),
        }
    }
}

fn parse_line(line: &str) -> Option<(Vec<Keysym>, String)> {
    let (sequence, rest) = line.split_once(':')?;
    let keysyms = sequence
        .split_whitespace()
        .map(|name| {
            name.strip_prefix('<')
                .and_then(|name| name.strip_suffix('>'))
                .and_then(Keysym::from_name)
        })
        .collect::<Option<Vec<Keysym>>>()?;
    Some((keysyms, quoted(rest)?))
}

impl ComposeTable {
    pub fn parse(text: &str) -> ComposeTable {
        let mut table = ComposeTable {
            outputs: HashMap::new(),
            prefixes: HashSet::new(),
            skipped: 0,
        };
        for line in text.lines().filter(|line| line.starts_with('<')) {
            let Some((keysyms, output)) = parse_line(line) else {
                table.skipped += 1;
                continue;
            };
            for len in 1..keysyms.len() {
                table.prefixes.insert(keysyms[..len].to_vec());
            }
            table.outputs.entry(keysyms).or_insert(output);
        }
        table
    }

    /// The reference table of `ldml.linux.compose`.
    // [spec:kbdgen:def:ldml.linux.compose+1]
    pub fn reference() -> &'static ComposeTable {
        static TABLE: LazyLock<ComposeTable> =
            LazyLock::new(|| ComposeTable::parse(include_str!("data/Compose")));
        &TABLE
    }

    pub fn len(&self) -> usize {
        self.outputs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.outputs.is_empty()
    }

    pub fn output(&self, sequence: &[Keysym]) -> Option<&str> {
        self.outputs.get(sequence).map(String::as_str)
    }
}

/// What a key press does to the composition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Composed {
    /// No sequence was pending and none starts here: the client types the
    /// keysym's character itself.
    Passed,
    /// The engine commits this text and the sequence ends.
    Committed(String),
    /// The sequence goes on.
    Pending,
    /// The key continues no sequence; IBus drops it and keeps the
    /// sequence pending, as GTK 4 does.
    Dropped,
}

/// The single character dead keysyms and a base compose to under NFC,
/// trying the combining characters in every order
/// (`ibus_check_algorithmically`, `check_normalize_nfc`). Over a Greek
/// base, the tilde is the perispomeni, as X.Org reuses `dead_tilde` for it.
fn algorithmic(dead: &[Keysym], base: Keysym) -> Option<char> {
    let base = base.to_char()?;
    let mut marks: Vec<char> = dead
        .iter()
        .map(|keysym| combining(*keysym).or_else(|| keysym.to_char()))
        .collect::<Option<Vec<char>>>()?;
    if ('\u{390}'..='\u{3FF}').contains(&base) {
        for mark in marks.iter_mut().filter(|mark| **mark == '\u{303}') {
            *mark = '\u{342}';
        }
    }
    let nfc = ComposingNormalizerBorrowed::new_nfc();
    let mut orders = vec![marks.clone()];
    for i in 0..marks.len() {
        for j in i + 1..marks.len() {
            let mut swapped = marks.clone();
            swapped.swap(i, j);
            orders.push(swapped);
        }
    }
    orders.into_iter().find_map(|marks| {
        let text: String = std::iter::once(base).chain(marks).collect();
        let composed = nfc.normalize(&text);
        let mut chars = composed.chars();
        match (chars.next(), chars.next()) {
            (Some(c), None) => Some(c),
            _ => None,
        }
    })
}

/// IBus's simple engine on one input context: the keysyms of the pending
/// sequence. A full match that a longer sequence extends stays pending;
/// IBus commits it only on a reset, which these semantics never make.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Composer {
    pub pending: Vec<Keysym>,
}

/// The outcome of checking a sequence against the table and the algorithm.
enum Check {
    Committed(String),
    Pending,
    Failed,
}

fn check(table: &ComposeTable, sequence: &[Keysym]) -> Check {
    let output = table.output(sequence);
    let longer = table.prefixes.contains(sequence);
    match (output, longer) {
        (Some(output), false) => return Check::Committed(output.to_owned()),
        (_, true) => return Check::Pending,
        (None, false) => {}
    }
    let dead = sequence
        .iter()
        .take_while(|keysym| keysym.is_dead())
        .count();
    if dead == sequence.len() {
        return match sequence {
            [first, second] if first == second => DEAD_CHARACTERS
                .iter()
                .find(|(name, ..)| Keysym::from_name(name) == Some(*first))
                .map_or(Check::Pending, |(.., isolated)| {
                    Check::Committed(isolated.to_string())
                }),
            _ => Check::Pending,
        };
    }
    match sequence.split_last() {
        Some((base, dead)) if dead.len() == sequence.len() - 1 && !dead.is_empty() => {
            match algorithmic(dead, *base) {
                Some(c) => Check::Committed(c.to_string()),
                None => Check::Failed,
            }
        }
        _ => Check::Failed,
    }
}

impl Composer {
    /// Presses `keysym`, as `ibus_engine_simple_process_key_event` handles
    /// a key without Control, Alt or Super.
    // [spec:kbdgen:def:ldml.linux.compose+1]
    pub fn press(&mut self, table: &ComposeTable, keysym: Keysym) -> Composed {
        let mut sequence = self.pending.clone();
        sequence.push(keysym);
        match check(table, &sequence) {
            Check::Committed(text) => {
                *self = Composer::default();
                Composed::Committed(text)
            }
            Check::Pending => {
                self.pending = sequence;
                Composed::Pending
            }
            Check::Failed if self.pending.is_empty() => Composed::Passed,
            Check::Failed => Composed::Dropped,
        }
    }

    /// What typing a sequence of keysyms from no pending sequence commits,
    /// a passed keysym typing its own character.
    pub fn type_keysyms(table: &ComposeTable, keysyms: &[Keysym]) -> (String, Composer) {
        let mut composer = Composer::default();
        let mut text = String::new();
        for keysym in keysyms {
            match composer.press(table, *keysym) {
                Composed::Passed => text.extend(keysym.to_char().filter(|c| !c.is_control())),
                Composed::Committed(committed) => text.push_str(&committed),
                Composed::Pending | Composed::Dropped => {}
            }
        }
        (text, composer)
    }
}
