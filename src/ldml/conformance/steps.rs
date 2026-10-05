//! The steps of a golden vector test (`ldml.test.vectors`), read strictly
//! from YAML: each step is a bare word (`decimal`, `commit`, `reset`,
//! `backspace`) or a mapping with exactly one step field.

use std::fmt;

use kbd_engine::{Gesture, ModifierState};
use kbd_ldml::escape::decode_plain;
use kbd_model::Direction;
use serde_yaml::Value;

use crate::ldml::yaml::node::{Fields, boolean, integer, list, string};
use crate::ldml::yaml::{At, YamlError, position_scan_code};

type Result<T> = std::result::Result<T, YamlError>;

/// The scan code of `space`.
const SPACE: u8 = 0x39;

/// What a golden vector checks after the steps before it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Expect {
    pub text: Option<String>,
    pub preedit: Option<String>,
    pub pass: Option<bool>,
    /// `Some(None)` expects that the last edit switched no layer.
    pub layer: Option<Option<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    Press {
        scan: u8,
        modifiers: ModifierState,
    },
    /// A touch key of the touch set named `size`, at 0-based `row` and
    /// `col` of the touch layer with id `layer`.
    Touch {
        size: String,
        layer: String,
        row: usize,
        col: usize,
        gesture: Gesture,
    },
    Id {
        id: String,
        gesture: Gesture,
    },
    Emit(String),
    Backspace(ModifierState),
    Decimal(ModifierState),
    Commit,
    Reset,
    /// Replaces the document and keeps the state, as keyboardTest3's
    /// `startContext` does.
    Context(String),
    Expect(Expect),
}

/// The modifier names of `ldml.test.vectors`; `shift`, `ctrl` and `alt`
/// are the left keys.
fn set_modifier(m: &mut ModifierState, name: &str) -> bool {
    match name {
        "shift" | "shiftL" => m.shift_l = true,
        "shiftR" => m.shift_r = true,
        "caps" => m.caps = true,
        "ctrl" | "ctrlL" => m.ctrl_l = true,
        "ctrlR" => m.ctrl_r = true,
        "alt" | "altL" => m.alt_l = true,
        "altR" => m.alt_r = true,
        "altgr" => {
            m.alt_r = true;
            m.altgr = true;
        }
        "cmd" => m.cmd = true,
        "extra1" => m.extra[0] = true,
        "extra2" => m.extra[1] = true,
        "extra3" => m.extra[2] = true,
        _ => return false,
    }
    true
}

fn modifiers(value: &Value, at: &At) -> Result<ModifierState> {
    let mut m = ModifierState::default();
    for name in string(value, at)?.split_ascii_whitespace() {
        if !set_modifier(&mut m, name) {
            return Err(at.error(format!("{name} is not a modifier name")));
        }
    }
    Ok(m)
}

/// A text value, with LDML's `\u{…}` escapes decoded.
fn decoded(value: &Value, at: &At) -> Result<String> {
    decode_plain(string(value, at)?).map_err(|e| at.error(e.to_string()))
}

fn index(value: &Value, at: &At) -> Result<usize> {
    usize::try_from(integer(value, at)?).map_err(|_| at.error("the number is too large"))
}

/// `tap`, or `{longPress: n}`, `{multiTap: n}` or `{flick: directions}`.
fn gesture(value: &Value, at: &At) -> Result<Gesture> {
    if let Value::String(word) = value {
        return match word.as_str() {
            "tap" => Ok(Gesture::Tap),
            other => Err(at.error(format!("{other} is not a gesture"))),
        };
    }
    let mut f = Fields::new(value, at)?;
    let gesture = match f.names().as_slice() {
        ["longPress"] => {
            let (v, a) = f.require("longPress")?;
            Gesture::LongPress(index(v, &a)?)
        }
        ["multiTap"] => {
            let (v, a) = f.require("multiTap")?;
            Gesture::MultiTap(index(v, &a)?)
        }
        ["flick"] => {
            let (v, a) = f.require("flick")?;
            let directions = string(v, &a)?
                .split_ascii_whitespace()
                .map(|d| {
                    Direction::from_name(d)
                        .ok_or_else(|| a.error(format!("{d} is not a direction")))
                })
                .collect::<Result<Vec<_>>>()?;
            Gesture::Flick(directions)
        }
        _ => {
            return Err(at.error("a gesture is tap, or one of longPress, multiTap and flick"));
        }
    };
    f.finish()?;
    Ok(gesture)
}

/// An ISO position name, `space`, or a hex scan code `0xNN`.
fn scan_code(value: &Value, at: &At) -> Result<u8> {
    let key = string(value, at)?;
    if key == "space" {
        return Ok(SPACE);
    }
    if let Some(hex) = key.strip_prefix("0x")
        && hex.len() == 2
    {
        return u8::from_str_radix(hex, 16)
            .map_err(|_| at.error(format!("{key} is not a hex scan code")));
    }
    position_scan_code(key)
        .ok_or_else(|| at.error(format!("{key} is not an ISO position, space or 0xNN")))
}

fn optional_mods(f: &mut Fields) -> Result<ModifierState> {
    f.take("mods")
        .map(|(v, a)| modifiers(v, &a))
        .transpose()
        .map(Option::unwrap_or_default)
}

fn press(value: &Value, at: &At) -> Result<Step> {
    let mut f = Fields::new(value, at)?;
    let (k, ka) = f.require("key")?;
    let scan = scan_code(k, &ka)?;
    let modifiers = optional_mods(&mut f)?;
    f.finish()?;
    Ok(Step::Press { scan, modifiers })
}

fn touch(value: &Value, at: &At) -> Result<Step> {
    let mut f = Fields::new(value, at)?;
    let (v, a) = f.require("size")?;
    let size = string(v, &a)?.to_string();
    let (v, a) = f.require("layer")?;
    let layer = string(v, &a)?.to_string();
    let (v, a) = f.require("row")?;
    let row = index(v, &a)?;
    let (v, a) = f.require("col")?;
    let col = index(v, &a)?;
    let gesture = match f.take("gesture") {
        Some((v, a)) => gesture(v, &a)?,
        None => Gesture::Tap,
    };
    f.finish()?;
    Ok(Step::Touch {
        size,
        layer,
        row,
        col,
        gesture,
    })
}

/// `backspace` and `decimal`: null or `{mods?}`.
fn mods_only(value: &Value, at: &At) -> Result<ModifierState> {
    let mut f = Fields::new(value, at)?;
    let modifiers = optional_mods(&mut f)?;
    f.finish()?;
    Ok(modifiers)
}

fn expect(value: &Value, at: &At) -> Result<Expect> {
    let mut f = Fields::new(value, at)?;
    let text = f.take("text").map(|(v, a)| decoded(v, &a)).transpose()?;
    let preedit = f.take("preedit").map(|(v, a)| decoded(v, &a)).transpose()?;
    let pass = f.take("pass").map(|(v, a)| boolean(v, &a)).transpose()?;
    let layer = match f.take("layer") {
        Some((Value::Null, _)) => Some(None),
        Some((v, a)) => Some(Some(string(v, &a)?.to_string())),
        None => None,
    };
    f.finish()?;
    let expect = Expect {
        text,
        preedit,
        pass,
        layer,
    };
    if expect == Expect::default() {
        return Err(at.error("expect names at least one of text, preedit, pass and layer"));
    }
    Ok(expect)
}

const STEP_FIELDS: &str = "press, touch, id, emit, backspace, decimal, context or expect";

fn mapped(value: &Value, at: &At) -> Result<Step> {
    let mut f = Fields::new(value, at)?;
    let names = f.names();
    let main: Vec<&str> = names.into_iter().filter(|n| *n != "gesture").collect();
    let [name] = main.as_slice() else {
        return Err(at.error(format!("a step has exactly one of {STEP_FIELDS}")));
    };
    let (v, a) = f.require(name)?;
    let step = match *name {
        "press" => press(v, &a)?,
        "touch" => touch(v, &a)?,
        "id" => Step::Id {
            id: string(v, &a)?.to_string(),
            gesture: match f.take("gesture") {
                Some((g, ga)) => gesture(g, &ga)?,
                None => Gesture::Tap,
            },
        },
        "emit" => Step::Emit(decoded(v, &a)?),
        "backspace" => Step::Backspace(mods_only(v, &a)?),
        "decimal" => Step::Decimal(mods_only(v, &a)?),
        "context" => Step::Context(decoded(v, &a)?),
        "expect" => Step::Expect(expect(v, &a)?),
        other => return Err(a.error(format!("{other} is not a step; one of {STEP_FIELDS}"))),
    };
    f.finish()?;
    Ok(step)
}

fn step(value: &Value, at: &At) -> Result<Step> {
    match value {
        Value::String(word) => match word.as_str() {
            "decimal" => Ok(Step::Decimal(ModifierState::default())),
            "backspace" => Ok(Step::Backspace(ModifierState::default())),
            "commit" => Ok(Step::Commit),
            "reset" => Ok(Step::Reset),
            other => Err(at.error(format!(
                "{other} is not a step; the bare steps are decimal, backspace, commit and reset"
            ))),
        },
        other => mapped(other, at),
    }
}

pub fn steps(value: &Value, at: &At) -> Result<Vec<Step>> {
    list(value, at)?
        .into_iter()
        .map(|(v, a)| step(v, &a))
        .collect()
}

fn mods_text(m: &ModifierState) -> String {
    let names = [
        (m.shift_l, "shiftL"),
        (m.shift_r, "shiftR"),
        (m.caps, "caps"),
        (m.ctrl_l, "ctrlL"),
        (m.ctrl_r, "ctrlR"),
        (m.alt_l, "altL"),
        (m.alt_r && !m.altgr, "altR"),
        (m.altgr, "altgr"),
        (m.cmd, "cmd"),
        (m.extra[0], "extra1"),
        (m.extra[1], "extra2"),
        (m.extra[2], "extra3"),
    ];
    names
        .iter()
        .filter(|(on, _)| *on)
        .map(|(_, n)| format!(" {n}"))
        .collect()
}

fn gesture_text(g: &Gesture) -> String {
    match g {
        Gesture::Tap => String::new(),
        Gesture::LongPress(n) => format!(" longPress {n}"),
        Gesture::MultiTap(n) => format!(" multiTap {n}"),
        Gesture::Flick(ds) => {
            let names: Vec<&str> = ds.iter().map(|d| d.name()).collect();
            format!(" flick {}", names.join(" "))
        }
    }
}

impl fmt::Display for Step {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Step::Press { scan, modifiers } => {
                write!(f, "press 0x{scan:02X}{}", mods_text(modifiers))
            }
            Step::Touch {
                size,
                layer,
                row,
                col,
                gesture,
            } => write!(
                f,
                "touch {size} {layer} row {row} col {col}{}",
                gesture_text(gesture)
            ),
            Step::Id { id, gesture } => write!(f, "id {id}{}", gesture_text(gesture)),
            Step::Emit(text) => write!(f, "emit {text:?}"),
            Step::Backspace(m) => write!(f, "backspace{}", mods_text(m)),
            Step::Decimal(m) => write!(f, "decimal{}", mods_text(m)),
            Step::Commit => f.write_str("commit"),
            Step::Reset => f.write_str("reset"),
            Step::Context(text) => write!(f, "context {text:?}"),
            Step::Expect(_) => f.write_str("expect"),
        }
    }
}
