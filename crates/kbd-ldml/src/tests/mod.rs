//! Tests over whole documents: shared fixtures here, one module per topic.

use std::path::PathBuf;

use crate::{Resolved, SourceDocument, read_keyboard, resolve};

mod cldr;
mod export;
mod read;
mod resolution;

fn testdata(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("testdata")
        .join(rel)
}

fn cldr_keyboards() -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(testdata("cldr/48/keyboards/3.0"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "xml"))
        .collect();
    paths.sort();
    paths
}

fn source(xml: &str) -> SourceDocument {
    read_keyboard("test.xml", None, xml.as_bytes()).unwrap()
}

fn resolved(xml: &str) -> Resolved {
    resolve(&source(xml)).unwrap_or_else(|e| panic!("{e}"))
}

/// The error message of resolving `xml`.
fn resolve_error(xml: &str) -> String {
    match read_keyboard("test.xml", None, xml.as_bytes()) {
        Err(e) => e.to_string(),
        Ok(s) => resolve(&s).map(|_| ()).unwrap_err().to_string(),
    }
}

/// A minimal keyboard3 document around `body`.
fn keyboard(attrs: &str, body: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<keyboard3 locale="sme" conformsTo="45" {attrs}>
  <info name="Test"/>
{body}
</keyboard3>
"#
    )
}

/// Drives the engine over a document, as `ldml.test.harness` describes:
/// the last `context_len` scalar values are the context, and each edit
/// deletes and appends scalar values.
struct Session {
    model: kbd_engine::Model,
    state: kbd_engine::State,
    text: String,
    preedit: String,
    layer: Option<String>,
}

impl Session {
    fn new(keyboard: kbd_model::Keyboard) -> Session {
        let model = kbd_engine::Model::from_keyboard(keyboard, kbd_engine::Options::default())
            .unwrap_or_else(|e| panic!("{e:?}"));
        Session {
            model,
            state: kbd_engine::State::default(),
            text: String::new(),
            preedit: String::new(),
            layer: None,
        }
    }

    /// Sends `event`; returns whether it passed.
    fn send(&mut self, event: kbd_engine::KeyEvent) -> bool {
        let chars: Vec<char> = self.text.chars().collect();
        let skip = chars.len().saturating_sub(self.model.context_len());
        let context = kbd_engine::Context {
            text: chars[skip..].iter().collect(),
            authoritative: true,
            at_start: skip == 0,
        };
        let (action, state) = self.model.key(&self.state, &context, &event);
        self.state = state;
        match action {
            kbd_engine::Action::Pass => true,
            kbd_engine::Action::Edit {
                delete,
                insert,
                preedit,
                layer,
            } => {
                let keep = self.text.chars().count().saturating_sub(delete);
                self.text = self.text.chars().take(keep).collect();
                self.text.push_str(&insert);
                self.preedit = preedit;
                self.layer = layer;
                false
            }
        }
    }

    fn id(&mut self, id: &str, gesture: kbd_engine::Gesture) -> bool {
        self.send(kbd_engine::KeyEvent::new(kbd_engine::Key::Id {
            id: id.to_string(),
            gesture,
        }))
    }

    fn tap(&mut self, id: &str) -> bool {
        self.id(id, kbd_engine::Gesture::Tap)
    }
}
