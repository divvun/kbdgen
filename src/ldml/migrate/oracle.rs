//! What a v3 desktop platform typed in the caps states
//! (`ldml.migrate.caps-diff`): Windows by its key attributes (`kbdl.caps`,
//! `kbdl.caps.sgcaps`), macOS by its key-map selection
//! (`keylayout.keymaps`), ChromeOS by its runtime fallback.

use super::convert::{DesktopTrace, Out};
use super::source::Desktop;

/// A caps state of `ldml.migrate.caps-diff`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapsState {
    Caps,
    CapsShift,
    AltCaps,
}

impl CapsState {
    pub const ALL: [CapsState; 3] = [CapsState::Caps, CapsState::CapsShift, CapsState::AltCaps];

    /// The v3 layer name of the state.
    pub fn name(self) -> &'static str {
        match self {
            CapsState::Caps => "caps",
            CapsState::CapsShift => "caps+shift",
            CapsState::AltCaps => "alt+caps",
        }
    }
}

fn layer<'a>(trace: &'a DesktopTrace, name: &str, position: usize) -> Option<&'a Out> {
    trace
        .layers
        .iter()
        .find(|l| l.name == name)
        .and_then(|l| l.cells.get(position))
        .map(|c| &c.out)
}

/// A cell of the row that follows an SGCAPS row: a dead key types its
/// character, and more than one UTF-16 unit is no key.
fn sgcaps_cell(cell: Option<Out>) -> Out {
    match cell {
        Some(Out::Dead(t) | Out::Text(t)) if t.encode_utf16().count() == 1 => Out::Text(t),
        _ => Out::NoKey,
    }
}

const SGCAPS: u8 = 0x02;
const CAPLOK: u8 = 0x01;
const CAPLOKALTGR: u8 = 0x04;

/// What a Windows layout DLL types in `state` for a key whose cells by
/// `kbdl` layer name are `cell`, "no key" being none (`kbdl.caps`,
/// `kbdl.caps.sgcaps`).
pub fn windows_caps(cell: &dyn Fn(&str) -> Option<Out>, state: CapsState) -> Out {
    let (default, shift, caps) = (cell("default"), cell("shift"), cell("caps"));
    let (alt, alt_shift, alt_caps) = (cell("alt"), cell("alt+shift"), cell("alt+caps"));
    let has_dead = ["default", "shift", "ctrl", "alt", "alt+shift"]
        .iter()
        .any(|n| matches!(cell(n), Some(Out::Dead(_))));
    let without_caps = || {
        let mut a = 0;
        if default != shift {
            a |= CAPLOK;
        }
        if alt != alt_shift {
            a |= CAPLOKALTGR;
        }
        a
    };
    let mut attributes = if caps.is_some() && caps != default && caps != shift {
        SGCAPS
    } else if caps.is_some() {
        let mut a = 0;
        if caps == shift {
            a |= CAPLOK;
        }
        if alt_caps == alt_shift {
            a |= CAPLOKALTGR;
        }
        a
    } else {
        without_caps()
    };
    if attributes == SGCAPS && has_dead {
        attributes = without_caps();
    }
    let or_none = |o: Option<Out>| o.unwrap_or(Out::NoKey);
    match state {
        CapsState::Caps if attributes == SGCAPS => sgcaps_cell(caps),
        CapsState::Caps if attributes & CAPLOK != 0 => or_none(shift),
        CapsState::Caps => or_none(default),
        CapsState::CapsShift if attributes == SGCAPS => sgcaps_cell(cell("caps+shift")),
        CapsState::CapsShift if attributes & CAPLOK != 0 => or_none(default),
        CapsState::CapsShift => or_none(shift),
        CapsState::AltCaps if attributes & CAPLOKALTGR != 0 => or_none(alt_shift),
        CapsState::AltCaps => or_none(alt),
    }
}

/// The v3 macOS layer a state selects: the first layer in file order whose
/// modifier keys match it, else the first layer (`defaultIndex="0"`).
fn macos_layer(trace: &DesktopTrace, state: CapsState) -> Option<&str> {
    let matches: &[&str] = match state {
        CapsState::Caps => &["caps"],
        CapsState::CapsShift => &["shift", "caps+shift"],
        CapsState::AltCaps => &["alt+caps"],
    };
    trace
        .layers
        .iter()
        .find(|l| matches.contains(&l.name.as_str()))
        .or_else(|| trace.layers.first())
        .map(|l| l.name.as_str())
}

/// The v3 ChromeOS layer a state selects: the exact layer, then `caps`,
/// `shift` and `default`.
fn chromeos_layer(trace: &DesktopTrace, state: CapsState) -> Option<&str> {
    [state.name(), "caps", "shift", "default"]
        .into_iter()
        .find(|n| trace.layers.iter().any(|l| l.name == *n))
}

/// What v3 typed in `state` at `position` on the trace's platform.
pub fn caps_output(trace: &DesktopTrace, state: CapsState, position: usize) -> Out {
    let selected = match trace.platform {
        Desktop::Windows => {
            let cell = |name: &str| {
                layer(trace, name, position)
                    .filter(|o| **o != Out::NoKey)
                    .cloned()
            };
            return windows_caps(&cell, state);
        }
        Desktop::MacOs => macos_layer(trace, state),
        Desktop::ChromeOs => chromeos_layer(trace, state),
    };
    selected
        .and_then(|name| layer(trace, name, position))
        .cloned()
        .unwrap_or(Out::NoKey)
}
