//! `ldml.linux.test.typing` and `linux.test.xkbcommon`: the written XKB
//! layout, read back and simulated per `ldml.linux.semantics`, against
//! `kbd-engine` on the same model; and the layout compiled, listed and
//! typed through the installed libxkbcommon.

mod compare;
mod exceptions;
mod parse;
mod tests;
mod xkbcommon;

use kbd_engine::{BackspacePolicy, Model, Options, OutputForm};
use kbd_model::Host;

use super::tests::{fixture_text, ids, linux_layouts};
use super::{Adapted, adapt};
use crate::build::linux::source::LinuxLayout;

pub use parse::{Parsed, parse};

/// The engine as the IBus engine runs it (`linux.ibus.semantics`).
pub const OPTIONS: Options = Options {
    output_form: OutputForm::Nfc,
    backspace: BackspacePolicy::CancelOrPass,
    host: Some(Host::Linux),
};

/// A layout as its XKB layout ships: the adapter's output, the written
/// symbols file read back, and the engine on the encoded model.
pub struct Subject {
    pub label: String,
    pub layout: LinuxLayout,
    pub adapted: Adapted,
    pub parsed: Parsed,
    pub model: Model,
}

/// The engine on `layout`'s encoded model, with the IBus engine's options.
pub fn model(layout: &LinuxLayout) -> Model {
    Model::from_bytes(&layout.model)
        .unwrap_or_else(|e| panic!("{}: {e}", layout.tag))
        .with_options(OPTIONS)
}

impl Subject {
    pub fn new(label: &str, layout: LinuxLayout) -> Subject {
        let adapted = adapt(&ids(), &layout).unwrap_or_else(|e| panic!("{label}: {e:#}"));
        let parsed =
            parse(&adapted.text, &adapted.entry.name).unwrap_or_else(|e| panic!("{label}: {e:#}"));
        Subject {
            label: label.to_owned(),
            model: model(&layout),
            layout,
            adapted,
            parsed,
        }
    }
}

pub fn subject(label: &str, tag: &str, yaml: &str) -> Subject {
    let layout = linux_layouts(&[(tag, yaml)])
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("{label}: no linux keyboard"));
    Subject::new(label, layout)
}

/// The fixture layouts of `ldml.linux.test.typing`: the golden v4 layouts,
/// the golden v3 layouts migrated in memory, and the Windows and macOS
/// feature fixtures as `linux` variants, as (label, tag, text).
pub fn fixture_layouts() -> Vec<(String, String, String)> {
    let golden = "crates/kbd-engine/tests/golden";
    let mut layouts = Vec::new();
    for (dir, tags) in [("layouts", ["und-x-mods", "vro"]), ("v3", ["se-NO", "vro"])] {
        for tag in tags {
            let text = fixture_text(&format!("{golden}/{dir}/{tag}.yaml"));
            layouts.push((format!("{dir}/{tag}"), tag.to_owned(), text));
        }
    }
    layouts.push((
        "windows/features".to_owned(),
        "sme".to_owned(),
        fixture_text("src/build/windows/kbdl/testdata/features.yaml")
            .replace("hardware:\n  windows:", "hardware:\n  linux:"),
    ));
    layouts.push((
        "macos/features".to_owned(),
        "sme".to_owned(),
        fixture_text("src/build/macos/testdata/features.yaml")
            .replace("hardware:\n  macOS:", "hardware:\n  linux:"),
    ));
    layouts
}
