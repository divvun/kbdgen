//! The keylayout input (`ldml.macos.input`): one `.keylayout` document as
//! plain data, in emission order. The v3 path and the v4 adapter both build
//! it, and one writer turns it into XML.

use indexmap::IndexMap;
use language_tags::LanguageTag;

/// The state every key starts and ends in when no dead key is pending.
pub const NONE_STATE: &str = "none";

/// A `<when>`: in `state`, type `output` and/or move to `next`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct When {
    pub state: String,
    pub output: Option<String>,
    pub next: Option<String>,
}

impl When {
    pub fn output(state: impl Into<String>, output: impl Into<String>) -> Self {
        When {
            state: state.into(),
            output: Some(output.into()),
            next: None,
        }
    }

    pub fn next(state: impl Into<String>, next: impl Into<String>) -> Self {
        When {
            state: state.into(),
            output: None,
            next: Some(next.into()),
        }
    }
}

/// What a `<key>` does: a plain output, or an action with its own id.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Binding {
    Output(String),
    Action { id: String, whens: Vec<When> },
}

/// One `<key>` of a key map.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Key {
    pub code: u16,
    pub binding: Binding,
}

/// One `<keyMap>` and the `<keyMapSelect>` with the same index: the
/// `keys` attribute of each of its `<modifier>` elements, and its keys in
/// emission order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KeyMap {
    pub modifiers: Vec<String>,
    pub keys: Vec<Key>,
}

/// One layout's `.keylayout`, decoded text throughout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeylayoutInput {
    /// The keyboard name (`keylayout.document.name`), which also names the
    /// file and its `KLInfo_` entry.
    pub name: String,
    pub tag: LanguageTag,
    pub display_names: IndexMap<LanguageTag, String>,
    pub default_index: usize,
    pub maps: Vec<KeyMap>,
    pub terminators: Vec<When>,
}
