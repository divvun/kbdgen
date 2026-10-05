//! v4 tests: loading, lowering, the engine on lowered layouts, and import.

use std::path::{Path, PathBuf};

use kbd_engine::{Action, Context, Gesture, Key, KeyEvent, Model, ModifierState, Options, State};
use kbd_ldml::SourceDocument;
use kbd_model::{Host, Keyboard};

use super::{Layout4, load, lower};
use crate::ldml::LdmlError;

mod engine;
mod import;
mod load;
mod lowering;

/// The Võro example of `docs/spec/ldml/yaml.md`, with the elided rows
/// filled in and the flow-mapping escapes quoted.
const VRO: &str = include_str!("../../../../crates/kbd-engine/tests/golden/layouts/vro.yaml");

/// A layout file `<tag>.yaml` in a fresh directory.
fn write(tag: &str, yaml: &str) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(format!("{tag}.yaml"));
    std::fs::write(&path, yaml).unwrap();
    (dir, path)
}

fn load_as(tag: &str, yaml: &str) -> Result<Layout4, LdmlError> {
    let (_dir, path) = write(tag, yaml);
    load(&path, tag)
}

/// A `sme` layout: `format`, the autonym, then `body`.
fn sme(body: &str) -> String {
    format!("format: 4\ndisplayNames: {{sme: Davvisámegiella}}\n{body}")
}

fn error(yaml: &str) -> String {
    load_as("sme", yaml).unwrap_err().to_string()
}

fn documents(tag: &str, yaml: &str) -> Vec<(Host, SourceDocument)> {
    let layout = load_as(tag, yaml).unwrap_or_else(|e| panic!("{e}"));
    lower(&layout).unwrap_or_else(|e| panic!("{e}"))
}

fn lower_error(yaml: &str) -> String {
    let layout = load_as("sme", yaml).unwrap_or_else(|e| panic!("{e}"));
    lower(&layout).unwrap_err().to_string()
}

fn document(tag: &str, yaml: &str, host: Host) -> SourceDocument {
    documents(tag, yaml)
        .into_iter()
        .find(|(h, _)| *h == host)
        .unwrap_or_else(|| panic!("no {} document", host.name()))
        .1
}

fn xml(tag: &str, yaml: &str, host: Host) -> String {
    kbd_ldml::write(&document(tag, yaml, host).document)
}

fn keyboard(tag: &str, yaml: &str, host: Host) -> Keyboard {
    kbd_ldml::resolve(&document(tag, yaml, host))
        .unwrap_or_else(|e| panic!("{e}"))
        .keyboard
}

/// Indents every line of `text` by `n` spaces, for rows in block scalars.
fn indent(text: &str, n: usize) -> String {
    text.lines()
        .map(|l| format!("{}{l}\n", " ".repeat(n)))
        .collect()
}

/// An `iso` layer whose positions are all `token`, as 13, 12, 12 and 11.
fn iso_rows(first: &str, token: &str) -> String {
    let row = |n: usize| vec![token; n].join(" ");
    format!(
        "{first} {}\n{}\n{}\n{}\n",
        [token; 12].join(" "),
        row(12),
        row(12),
        row(11)
    )
}

/// `hardware.default` with the given layers, each an `iso` layer.
fn hardware(layers: &[(&str, &str)]) -> String {
    let mut out = String::from("hardware:\n  default:\n    layers:\n");
    for (key, rows) in layers {
        out.push_str(&format!("      {key}: |\n{}", indent(rows, 8)));
    }
    out
}

const QWERTY: &str = "` 1 2 3 4 5 6 7 8 9 0 - =\nq w e r t y u i o p [ ]\na s d f g h j k l ; ' #\n< z x c v b n m , . /\n";
const SHIFTED: &str = "~ ! @ £ $ % ^ & * ( ) _ +\nQ W E R T Y U I O P { }\nA S D F G H J K L : \" |\n> Z X C V B N M < > ?\n";

/// A host session on a lowered keyboard, applying actions as a TSF-like
/// host does.
struct Typist {
    model: Model,
    state: State,
    text: String,
    preedit: String,
    layer: Option<String>,
}

impl Typist {
    fn new(keyboard: Keyboard) -> Typist {
        let host = keyboard.host;
        Typist {
            model: Model::from_keyboard(
                keyboard,
                Options {
                    host,
                    ..Options::default()
                },
            )
            .unwrap_or_else(|e| panic!("{e:?}")),
            state: State::default(),
            text: String::new(),
            preedit: String::new(),
            layer: None,
        }
    }

    fn press(&mut self, event: KeyEvent) -> &mut Typist {
        let context = Context::new(self.text.clone());
        let (action, state) = self.model.key(&self.state, &context, &event);
        match action {
            Action::Pass => {
                self.text.push_str(&self.preedit);
                self.preedit.clear();
                self.state = State::default();
                if event.key == Key::Backspace {
                    self.text.pop();
                }
            }
            Action::Edit {
                delete,
                insert,
                preedit,
                layer,
            } => {
                let n = self.text.chars().count();
                self.text = self.text.chars().take(n - delete).collect();
                self.text.push_str(&insert);
                self.preedit = preedit;
                self.state = state;
                self.layer = layer;
            }
        }
        self
    }

    fn scan(&mut self, code: u8) -> &mut Typist {
        self.press(KeyEvent::new(Key::Scan(code)))
    }

    fn scan_with(&mut self, code: u8, modifiers: ModifierState) -> &mut Typist {
        self.press(KeyEvent::with(Key::Scan(code), modifiers))
    }

    fn id(&mut self, id: &str, gesture: Gesture) -> &mut Typist {
        self.press(KeyEvent::new(Key::Id {
            id: id.to_string(),
            gesture,
        }))
    }

    fn touch(
        &mut self,
        set: usize,
        layer: usize,
        row: usize,
        col: usize,
        gesture: Gesture,
    ) -> &mut Typist {
        self.press(KeyEvent::new(Key::Touch {
            set,
            layer,
            row,
            col,
            gesture,
        }))
    }
}

/// The layout of `tag` written into a bundle directory's `layouts`.
fn bundle(layouts: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("layouts")).unwrap();
    for (tag, yaml) in layouts {
        std::fs::write(dir.path().join("layouts").join(format!("{tag}.yaml")), yaml).unwrap();
    }
    dir
}

fn cldr_keyboards() -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("crates/kbd-ldml/testdata/cldr/48/keyboards/3.0");
    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    paths.sort();
    paths
}
