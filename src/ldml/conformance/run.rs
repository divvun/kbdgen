//! Running golden vector files (`ldml.test.vectors`) through the harness
//! (`ldml.test.harness`).

use std::path::Path;

use kbd_engine::harness::Harness;
use kbd_engine::{Key, KeyEvent, Model};
use kbd_model::{Host, Keyboard};

use super::steps::{Expect, Step};
use super::vectors::{Source, VectorFile, VectorTest, parse};
use super::{Failure, Observed, Report};
use crate::ldml::LdmlError;
use crate::ldml::layouts::normalise_tag;
use crate::ldml::migrate::migrate_file;
use crate::ldml::yaml::{Layout4, load, lower, read_yaml};

fn vector_error(path: &Path, message: impl Into<String>) -> LdmlError {
    LdmlError::Vectors {
        path: path.to_path_buf(),
        message: message.into(),
    }
}

// [spec:kbdgen:def:ldml.test.vectors+1]
/// The v4 layout at `path`; a v3 layout is migrated in memory
/// (`ldml.migrate.*`), and its blocking defects are an error naming their
/// codes.
fn layout(path: &Path, tag: &str) -> Result<Layout4, LdmlError> {
    match load(path, tag) {
        Err(LdmlError::V3Layout { .. }) => {
            let migration = migrate_file(path, tag)?;
            let codes: Vec<&str> = migration
                .blocking_codes()
                .into_iter()
                .map(|c| c.name())
                .collect();
            match migration.layout {
                Some(layout) if codes.is_empty() => Ok(layout),
                _ => Err(vector_error(
                    path,
                    format!(
                        "the v3 layout does not migrate: blocked by defects {}",
                        codes.join(", ")
                    ),
                )),
            }
        }
        other => other,
    }
}

/// The keyboard of `host`'s document of the layout at `path`, tagged by
/// its file stem as bundle layouts are.
fn layout_keyboard(path: &Path, host: Host) -> Result<Keyboard, LdmlError> {
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let tag = normalise_tag(&stem).ok_or(LdmlError::InvalidTag { tag: stem })?;
    let documents = lower(&layout(path, &tag)?)?;
    let (_, source) = documents
        .into_iter()
        .find(|(h, _)| *h == host)
        .ok_or_else(|| vector_error(path, format!("the layout has no {} document", host.name())))?;
    let mut keyboard = kbd_ldml::resolve(&source)?.keyboard;
    keyboard.host = Some(host);
    Ok(keyboard)
}

/// The keyboard of the XML file at `path`; `host`, when given, must agree
/// with a host the document names, and is set when it names none.
fn xml_keyboard(path: &Path, host: Option<Host>) -> Result<Keyboard, LdmlError> {
    let mut keyboard = kbd_ldml::resolve(&kbd_ldml::read_keyboard_file(path)?)?.keyboard;
    match (keyboard.host, host) {
        (Some(own), Some(given)) if own != given => {
            return Err(vector_error(
                path,
                format!(
                    "the keyboard is the {} document, not {}",
                    own.name(),
                    given.name()
                ),
            ));
        }
        (None, Some(given)) => keyboard.host = Some(given),
        _ => {}
    }
    Ok(keyboard)
}

/// The model a vector file runs on, with paths relative to `root`.
pub fn vector_model(file: &VectorFile, root: &Path) -> Result<Model, LdmlError> {
    let keyboard = match &file.source {
        Source::Layout(rel) => {
            let host = file
                .host
                .ok_or_else(|| vector_error(&root.join(rel), "a layout needs host"))?;
            layout_keyboard(&root.join(rel), host)?
        }
        Source::Keyboard(rel) => xml_keyboard(&root.join(rel), file.host)?,
    };
    let options = kbd_engine::Options {
        host: file.host,
        ..file.options
    };
    Model::from_keyboard(keyboard, options).map_err(|e| {
        vector_error(
            root,
            format!("the keyboard does not load in the engine: {e}"),
        )
    })
}

/// The touch event of a `touch` step: the set by name and the layer by id.
fn touch_event(model: &Model, step: &Step) -> Result<KeyEvent, String> {
    let Step::Touch {
        size,
        layer,
        row,
        col,
        gesture,
    } = step
    else {
        return Err(format!("{step} is not a touch step"));
    };
    let set = model
        .touch_set_by_name(size)
        .ok_or_else(|| format!("the keyboard has no touch set named {size}"))?;
    let layer = model
        .keyboard()
        .touch
        .get(set)
        .and_then(|s| s.layers.iter().position(|l| &l.id == layer))
        .ok_or_else(|| format!("touch set {size} has no layer {layer}"))?;
    Ok(KeyEvent::new(Key::Touch {
        set,
        layer,
        row: *row,
        col: *col,
        gesture: gesture.clone(),
    }))
}

/// The event a step sends; `None` for steps that send none.
fn event(model: &Model, step: &Step) -> Result<Option<KeyEvent>, String> {
    Ok(Some(match step {
        Step::Press { scan, modifiers } => KeyEvent::with(Key::Scan(*scan), *modifiers),
        Step::Touch { .. } => touch_event(model, step)?,
        Step::Id { id, gesture } => KeyEvent::new(Key::Id {
            id: id.clone(),
            gesture: gesture.clone(),
        }),
        Step::Emit(text) => KeyEvent::new(Key::Emit(text.clone())),
        Step::Backspace(m) => KeyEvent::with(Key::Backspace, *m),
        Step::Decimal(m) => KeyEvent::with(Key::Decimal, *m),
        Step::Commit => KeyEvent::new(Key::Commit),
        Step::Reset | Step::Context(_) | Step::Expect(_) => return Ok(None),
    }))
}

fn layer_text(layer: Option<&str>) -> String {
    layer.map_or("none".to_string(), |l| format!("{l:?}"))
}

/// The fields `expect` states, as `(field, value)`, when any of them
/// differs from what the harness shows; compared exactly, without
/// normalization.
fn mismatch(h: &Harness, expect: &Expect) -> Option<Vec<(String, String)>> {
    let mut stated = Vec::new();
    let mut differs = false;
    let mut field = |name: &str, value: String, equal: bool| {
        stated.push((name.to_string(), value));
        differs |= !equal;
    };
    if let Some(text) = &expect.text {
        field("text", format!("{text:?}"), text == h.document());
    }
    if let Some(preedit) = &expect.preedit {
        field("preedit", format!("{preedit:?}"), preedit == h.preedit());
    }
    if let Some(pass) = expect.pass {
        field("pass", pass.to_string(), pass == h.passed());
    }
    if let Some(layer) = &expect.layer {
        let layer = layer.as_deref();
        field("layer", layer_text(layer), layer == h.layer());
    }
    differs.then_some(stated)
}

fn run_test(model: &Model, test: &VectorTest, label: &str, report: &mut Report) {
    let mut h = Harness::new(model);
    h.set_document(&test.context);
    h.set_at_start(test.at_start);
    report.tests += 1;
    for (i, step) in test.steps.iter().enumerate() {
        let failure = |expected: Vec<(String, String)>, h: &Harness| Failure {
            file: label.to_string(),
            test: test.name.clone(),
            step: i + 1,
            written: step.to_string(),
            expected,
            observed: Observed::of(h),
        };
        match (step, event(model, step)) {
            (Step::Reset, _) => h.reset(),
            (Step::Context(text), _) => h.set_document(text),
            (Step::Expect(expect), _) => {
                report.checks += 1;
                if let Some(expected) = mismatch(&h, expect) {
                    report.failures.push(failure(expected, &h));
                }
            }
            (_, Ok(Some(e))) => {
                h.send(&e);
            }
            (_, Ok(None)) => {}
            (_, Err(message)) => {
                report
                    .failures
                    .push(failure(vec![("key".to_string(), message)], &h));
            }
        }
    }
}

/// Runs every test of `file` on `model`; `label` names the file in
/// failures.
pub fn run_tests(model: &Model, file: &VectorFile, label: &str) -> Report {
    let mut report = Report {
        files: 1,
        ..Report::default()
    };
    for test in &file.tests {
        run_test(model, test, label, &mut report);
    }
    report
}

// [spec:kbdgen:def:ldml.test.vectors+1]
/// Reads and runs the vector file at `path`, whose `layout` and
/// `keyboard` paths are relative to `root`; `label` names it in messages.
pub fn run_vector_file(path: &Path, root: &Path, label: &str) -> Result<Report, LdmlError> {
    let file = parse(label, &read_yaml(path)?)?;
    let model = vector_model(&file, root)?;
    Ok(run_tests(&model, &file, label))
}
