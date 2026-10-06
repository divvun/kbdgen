//! The acceptance cases of `tsf.test.acceptance` and what `kbd-engine`
//! makes of them, following the text service's key handling: scan codes
//! by `tsf.keys.identity`, Ctrl+Alt as AltGr (`tsf.keys.ctrl-alt`), a
//! `Commit` before every key that passes other than a lone modifier
//! (`tsf.edit.reset`), and a passed Backspace erasing one scalar value
//! before the caret, as a Win32 `EDIT` does. A Ctrl+Alt chord that passes
//! never reaches the text service, so it keeps the context, and types
//! nothing: the US dummy layout beneath has no Ctrl+Alt column.

use kbd_engine::harness::Harness;
use kbd_engine::{Action, Key, KeyEvent, Model, ModifierState};

use crate::keys::{AltGrChords, ctrl_alt};

/// How the control's text after a case is checked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum End {
    /// It is the engine's text.
    Engine,
    /// The case ends with a Backspace the text service passes, after a
    /// grapheme of several scalar values, which each control erases its own
    /// way. The engine's text before that Backspace is the case's text,
    /// and the control must hold what its own Backspace leaves of it.
    Native,
}

/// What the layout DLL alone, without the text service, must type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dll {
    /// The engine's text: the case is table-expressible.
    Same,
    /// Something else: the case needs the text service.
    Differs,
    /// Either; recorded only.
    Unchecked,
}

// [spec:kbdgen:req:tsf.test.acceptance]
/// Name, scan-code chords (`e0` prefixes extended keys, `+` holds keys
/// together), the text after the last key, how the control's text is
/// checked, and what the layout DLL alone must type.
#[rustfmt::skip]
pub const CASES: &[(&str, &str, &str, End, Dll)] = &[
    ("plain", "10 11 12", "qwe", End::Engine, Dll::Same),
    ("acute a", "0d 1e", "á", End::Engine, Dll::Same),
    ("acute b", "0d 30", "b\u{301}", End::Engine, Dll::Differs),
    ("acute shift b", "0d 2a+30", "B\u{301}", End::Engine, Dll::Differs),
    ("acute e", "0d 12", "e\u{323}\u{301}", End::Engine, Dll::Differs),
    ("acute z", "0d 2c", "\u{1D56B}\u{301}", End::Engine, Dll::Differs),
    ("acute space", "0d 39", "´", End::Engine, Dll::Same),
    ("acute j", "0d 24", "´j", End::Engine, Dll::Same),
    ("acute caron c", "0d 29 2e", "´č", End::Engine, Dll::Unchecked),
    ("chain tilde acute o", "2a+29 0d 18", "ṍ", End::Engine, Dll::Same),
    ("chain tilde acute caron o", "2a+29 0d 29 18", "õ\u{301}\u{30C}", End::Engine, Dll::Differs),
    ("chain tilde acute space", "2a+29 0d 39", "~´", End::Engine, Dll::Differs),
    ("chain tilde acute j", "2a+29 0d 24", "~´j", End::Engine, Dll::Unchecked),
    ("altgr acute b", "e038+0d 30", "b\u{301}", End::Engine, Dll::Differs),
    ("altgr at", "e038+03", "@", End::Engine, Dll::Same),
    ("altgr t", "e038+14", "t\u{301}", End::Engine, Dll::Same),
    ("acute altgr t", "0d e038+14", "´t\u{301}", End::Engine, Dll::Unchecked),
    ("acute altgr ligature", "0d e038+1e", "´a\u{331}", End::Engine, Dll::Differs),
    ("q apostrophe", "10 2b", "ʠ", End::Engine, Dll::Differs),
    ("z apostrophe twice", "2c 2b 2b", "ʐ", End::Engine, Dll::Differs),
    ("sch apostrophe", "1f 2e 23 2b", "ʃ", End::Engine, Dll::Differs),
    ("a q apostrophe", "1e 10 2b", "ɑʠ", End::Engine, Dll::Differs),
    ("vowel hyphen", "12 35", "e\u{304}", End::Engine, Dll::Differs),
    ("acute a apostrophe", "0d 1e 2b", "a\u{30B}", End::Engine, Dll::Differs),
    ("decimal", "53", ",", End::Engine, Dll::Same),
    ("caps acute a", "3a 0d 1e 3a", "ęA", End::Engine, Dll::Unchecked),
    ("acute end a", "0d e04f 1e", "´a", End::Engine, Dll::Unchecked),
    ("acute backspace a", "0d 0e 1e", "a", End::Engine, Dll::Unchecked),
    ("chain backspace o", "2a+29 0d 0e 18", "o", End::Engine, Dll::Unchecked),
    ("z apostrophe backspace apostrophe", "2c 2b 0e 2b", "'", End::Engine, Dll::Unchecked),
    ("q backspace apostrophe", "10 0e 2b", "'", End::Engine, Dll::Same),
    ("acute b backspace", "0d 30 0e", "b\u{301}", End::Native, Dll::Unchecked),
    ("ctrl alt at", "1d+38+03", "@", End::Engine, Dll::Same),
    ("ctrl altgr t", "1d+e038+14", "t\u{301}", End::Engine, Dll::Same),
    ("rctrl alt at", "e01d+38+03", "@", End::Engine, Dll::Same),
    ("rctrl altgr t", "e01d+e038+14", "t\u{301}", End::Engine, Dll::Same),
    ("ctrl alt acute a", "1d+38+0d 1e", "á", End::Engine, Dll::Same),
    ("ctrl alt acute b", "1d+38+0d 30", "b\u{301}", End::Engine, Dll::Differs),
    ("ctrl altgr acute a", "1d+e038+0d 1e", "á", End::Engine, Dll::Same),
    ("rctrl alt acute a", "e01d+38+0d 1e", "á", End::Engine, Dll::Same),
    ("acute ctrl alt q a", "0d 1d+38+10 1e", "á", End::Engine, Dll::Same),
    ("acute ctrl altgr q a", "0d 1d+e038+10 1e", "á", End::Engine, Dll::Same),
    ("acute rctrl alt q a", "0d e01d+38+10 1e", "á", End::Engine, Dll::Same),
    ("ctrl alt shift t", "1d+38+2a+14", "", End::Engine, Dll::Same),
];

/// The UTF-16 units of `text` as space-separated lowercase hex, as the
/// driver reports a control's text.
pub fn hex(text: &str) -> String {
    let units: Vec<String> = text
        .encode_utf16()
        .map(|unit| format!("{unit:04x}"))
        .collect();
    units.join(" ")
}

/// Applies `Commit` and resets the state, as the text service does before
/// a key passes.
fn commit(harness: &mut Harness) {
    harness.send(&KeyEvent::new(Key::Commit));
    harness.reset();
}

/// Holds the modifier `code` in `held`, or toggles Caps Lock in `toggled`;
/// false for a key that is not a modifier.
fn modifier(code: u16, held: &mut ModifierState, toggled: &mut ModifierState) -> bool {
    match code {
        0x2a => held.shift_l = true,
        0x36 => held.shift_r = true,
        0x1d => held.ctrl_l = true,
        0xe01d => held.ctrl_r = true,
        0x38 => held.alt_l = true,
        0xe038 => {
            held.alt_r = true;
            held.altgr = true;
        }
        0x3a => toggled.caps = !toggled.caps,
        _ => return false,
    }
    true
}

/// Types one chord; true when its key passed the text service.
fn press(
    harness: &mut Harness,
    model: &Model,
    chords: &AltGrChords,
    caps: &mut ModifierState,
    chord: &str,
) -> Result<bool, String> {
    let mut held = ModifierState::default();
    let mut key = None;
    for part in chord.split('+') {
        let code = u16::from_str_radix(part, 16).map_err(|e| format!("chord {chord}: {e}"))?;
        if !modifier(code, &mut held, caps) {
            key = Some(code);
        }
    }
    held.caps = caps.caps;
    let Some(code) = key else {
        return Ok(false);
    };
    let key = match code {
        0xe04f => {
            commit(harness);
            return Ok(true);
        }
        0x0e => Key::Backspace,
        0x53 => Key::Decimal,
        0x00..=0xff => Key::Scan(code as u8),
        _ => {
            return Err(format!(
                "chord {chord}: no extended key but End is simulated"
            ));
        }
    };
    let backspace = key == Key::Backspace;
    let windows = &model.keyboard().windows;
    let Some(held) = ctrl_alt(&key, held, windows, &chords.typed) else {
        return Ok(true);
    };
    if !matches!(harness.send(&KeyEvent::with(key, held)), Action::Pass) {
        return Ok(false);
    }
    commit(harness);
    if !backspace {
        return Err(format!(
            "chord {chord}: the engine passes it to the layout beneath"
        ));
    }
    let mut document: Vec<char> = harness.document().chars().collect();
    document.pop();
    harness.set_document(&document.into_iter().collect::<String>());
    Ok(true)
}

/// What the engine leaves after `chords` from a cleared control, typed as
/// the driver does (End first), and whether the last key passed.
pub fn typed(model: &Model, chords: &str) -> Result<(String, bool), String> {
    let mut harness = Harness::new(model);
    let altgr = AltGrChords::of(model);
    let mut caps = ModifierState::default();
    commit(&mut harness);
    let mut passed = false;
    for chord in chords.split(' ') {
        passed = press(&mut harness, model, &altgr, &mut caps, chord)?;
    }
    if caps.caps {
        return Err("the case leaves Caps Lock on".to_owned());
    }
    if !harness.preedit().is_empty() {
        return Err(format!(
            "the case ends with {:?} pending",
            harness.preedit()
        ));
    }
    Ok((harness.document().to_owned(), passed))
}

/// The engine's text of a case: for `End::Native`, the text before its
/// last key, which must be a Backspace that passes.
pub fn engine_text(model: &Model, chords: &str, end: End) -> Result<String, String> {
    let (text, passed) = typed(model, chords)?;
    if end == End::Engine {
        return Ok(text);
    }
    let Some((before, "0e")) = chords.rsplit_once(' ') else {
        return Err("a native case must end with Backspace".to_owned());
    };
    if !passed {
        return Err("the engine consumes the final Backspace".to_owned());
    }
    Ok(typed(model, before)?.0)
}
