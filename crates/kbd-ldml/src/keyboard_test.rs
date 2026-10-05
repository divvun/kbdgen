//! CLDR keyboardTest3 documents (`ldmlKeyboardTest3.dtd`, a technical
//! preview), read through `xmlem` into plain data for a test runner.
//!
//! Text values are escape-decoded once here, so runners compare scalar
//! values. `repertoire` elements are read but carry no steps: running them
//! is deferred (`ldml.test.repertoire`).

use std::path::Path;

use kbd_model::{Direction, is_nmtoken};

use crate::diag::{Diagnostic, Error, Result};
use crate::escape::decode_plain;
use crate::read::read_document;
use crate::tree::El;

/// The `repertoire@type` values of the DTD.
pub const REPERTOIRE_TYPES: [&str; 7] = [
    "default",
    "simple",
    "gesture",
    "flick",
    "longPress",
    "multiTap",
    "hardware",
];

/// A keyboardTest3 document: the keyboard file it tests, its repertoires
/// and its test groups.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyboardTest {
    /// `info@name`.
    pub name: String,
    pub author: Option<String>,
    /// `info@keyboard`, a keyboard file name such as `pcm.xml`.
    pub keyboard: String,
    pub repertoires: Vec<Repertoire>,
    pub groups: Vec<TestGroup>,
}

/// A `repertoire`: a UnicodeSet of characters the keyboard must produce,
/// kept as written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repertoire {
    pub name: String,
    pub chars: String,
    /// One of [`REPERTOIRE_TYPES`], if given.
    pub kind: Option<String>,
}

/// A `tests` element.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestGroup {
    pub name: String,
    pub tests: Vec<Test>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Test {
    pub name: String,
    pub steps: Vec<TestStep>,
}

/// How a `keystroke` presses its key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TestGesture {
    Tap,
    /// `flick`: a direction sequence.
    Flick(Vec<Direction>),
    /// `longPress`: the 1-based index into the key's long-press list.
    LongPress(usize),
    /// `tapCount`: the tap count, at least 2.
    TapCount(usize),
}

/// A step of a `test`; text values are escape-decoded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TestStep {
    StartContext(String),
    Keystroke { key: String, gesture: TestGesture },
    Emit(String),
    Backspace,
    Check(String),
}

fn required<'a>(el: &'a El, name: &str) -> Result<&'a str> {
    el.attr(name)
        .ok_or_else(|| el.attr_error(name, format!("{name} is required")))
}

fn token(el: &El, name: &str) -> Result<String> {
    let value = required(el, name)?;
    if is_nmtoken(value) {
        Ok(value.to_string())
    } else {
        Err(el.attr_error(name, format!("{value:?} is not an NMTOKEN")))
    }
}

fn text(el: &El, name: &str) -> Result<String> {
    decode_plain(required(el, name)?).map_err(|e| el.attr_error(name, e.to_string()))
}

/// A count attribute in `min..=999`.
fn count(el: &El, name: &str, min: usize) -> Result<usize> {
    let value = required(el, name)?;
    match value.parse::<usize>() {
        Ok(n) if (min..=999).contains(&n) => Ok(n),
        _ => Err(el.attr_error(
            name,
            format!("{value:?} is not a whole number from {min} to 999"),
        )),
    }
}

fn gesture(el: &El) -> Result<TestGesture> {
    let given: Vec<&str> = ["flick", "longPress", "tapCount"]
        .into_iter()
        .filter(|a| el.attr(a).is_some())
        .collect();
    match given.as_slice() {
        [] => Ok(TestGesture::Tap),
        ["flick"] => {
            let directions = required(el, "flick")?
                .split_ascii_whitespace()
                .map(|d| {
                    Direction::from_name(d)
                        .ok_or_else(|| el.attr_error("flick", format!("{d:?} is not a direction")))
                })
                .collect::<Result<Vec<_>>>()?;
            if directions.is_empty() {
                return Err(el.attr_error("flick", "a flick has at least one direction"));
            }
            Ok(TestGesture::Flick(directions))
        }
        ["longPress"] => Ok(TestGesture::LongPress(count(el, "longPress", 1)?)),
        ["tapCount"] => Ok(TestGesture::TapCount(count(el, "tapCount", 2)?)),
        _ => Err(el.error(format!(
            "a keystroke has one gesture, not {}",
            given.join(" and ")
        ))),
    }
}

fn step(el: &El, first: bool) -> Result<TestStep> {
    match el.name.as_str() {
        "startContext" if first => Ok(TestStep::StartContext(text(el, "to")?)),
        "startContext" => Err(el.error("startContext must be the first step of a test")),
        "keystroke" => Ok(TestStep::Keystroke {
            key: token(el, "key")?,
            gesture: gesture(el)?,
        }),
        "emit" => Ok(TestStep::Emit(text(el, "to")?)),
        "backspace" => Ok(TestStep::Backspace),
        "check" => Ok(TestStep::Check(text(el, "result")?)),
        other => Err(el.error(format!("{other} is not a test step"))),
    }
}

/// The children of `el` other than `special` and foreign-namespace
/// elements, which a test runner ignores.
fn own_children(el: &El) -> impl Iterator<Item = &El> {
    el.children
        .iter()
        .filter(|c| c.prefix.is_none() && c.name != "special")
}

fn test(el: &El) -> Result<Test> {
    let steps = own_children(el)
        .enumerate()
        .map(|(i, s)| step(s, i == 0))
        .collect::<Result<Vec<_>>>()?;
    Ok(Test {
        name: token(el, "name")?,
        steps,
    })
}

fn group(el: &El) -> Result<TestGroup> {
    let tests = own_children(el)
        .map(|t| match t.name.as_str() {
            "test" => test(t),
            other => Err(t.error(format!("{other} is not allowed in tests"))),
        })
        .collect::<Result<Vec<_>>>()?;
    if tests.is_empty() {
        return Err(el.error("tests holds at least one test"));
    }
    Ok(TestGroup {
        name: token(el, "name")?,
        tests,
    })
}

fn repertoire(el: &El) -> Result<Repertoire> {
    let kind = el.attr("type").map(str::to_string);
    if let Some(k) = &kind
        && !REPERTOIRE_TYPES.contains(&k.as_str())
    {
        return Err(el.attr_error("type", format!("{k:?} is not a repertoire type")));
    }
    Ok(Repertoire {
        name: token(el, "name")?,
        chars: required(el, "chars")?.to_string(),
        kind,
    })
}

fn document(root: &El) -> Result<KeyboardTest> {
    if root.name != "keyboardTest3" {
        return Err(root.error("the root of a keyboard test must be keyboardTest3"));
    }
    required(root, "conformsTo")?;
    let info = root
        .child("info")
        .ok_or_else(|| root.error("info is required"))?;
    let mut out = KeyboardTest {
        name: token(info, "name")?,
        author: info.attr("author").map(str::to_string),
        keyboard: required(info, "keyboard")?.to_string(),
        repertoires: Vec::new(),
        groups: Vec::new(),
    };
    for el in own_children(root) {
        match el.name.as_str() {
            "info" => {}
            "repertoire" => out.repertoires.push(repertoire(el)?),
            "tests" => out.groups.push(group(el)?),
            other => return Err(el.error(format!("{other} is not allowed in keyboardTest3"))),
        }
    }
    Ok(out)
}

// [spec:kbdgen:req:ldml.test.cldr]
// [spec:kbdgen:req:ldml.test.repertoire]
/// Reads a keyboardTest3 document per `ldml.xml.read`. Steps keep their
/// order; `special` elements are skipped; `repertoire` elements are kept
/// as written, for a runner to note and skip.
pub fn read_keyboard_test(name: &str, path: Option<&Path>, bytes: &[u8]) -> Result<KeyboardTest> {
    let source = read_document(name, path, bytes)?;
    document(&El::from_document(name, &source.document)?)
}

/// Reads a keyboardTest3 file.
pub fn read_keyboard_test_file(path: &Path) -> Result<KeyboardTest> {
    let name = path.display().to_string();
    let bytes = std::fs::read(path)
        .map_err(|e| Error::from(Diagnostic::new(&name, "", format!("cannot read: {e}"))))?;
    read_keyboard_test(&name, Some(path), &bytes)
}
