//! The engine's plain-data API types: events, contexts, states, actions,
//! options and errors.

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use kbd_model::{DecodeError, Direction, Host, InvariantError, MarkerIndex, Text, TextElem};

/// A touch gesture, or how an `Id` key is pressed.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Gesture {
    Tap,
    /// `LongPress(n)` picks `long_press[n-1]` for *n* ≥ 1 and
    /// `long_press_default` for *n* = 0.
    LongPress(usize),
    /// The *n*-th tap of a multi-tap sequence, *n* ≥ 2. `MultiTap(1)` is the
    /// key itself.
    MultiTap(usize),
    /// The direction sequence of a flick.
    Flick(Vec<Direction>),
}

/// The key of a [`KeyEvent`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Key {
    /// A hardware key: a PC/AT set 1 scan code without `E0`.
    Scan(u8),
    /// The numpad decimal key.
    Decimal,
    Backspace,
    /// A touch key of touch set `set` (`Model::touch_set_for_width`) at
    /// (`row`, `col`) of layer `layer`. For a keyboard with no touch sets,
    /// set 0 is its hardware set presented as touch and `layer` indexes the
    /// hardware layers.
    Touch {
        set: usize,
        layer: usize,
        row: usize,
        col: usize,
        gesture: Gesture,
    },
    /// A key named by its id, whatever its layer; used by tests.
    Id {
        id: String,
        gesture: Gesture,
    },
    /// Text handled as a key's output, already escape-decoded; used by
    /// tests.
    Emit(String),
    /// The host is about to change the context (`ldml.engine.commit`).
    Commit,
}

/// The modifier state of an event. `caps` is the Caps Lock state, and
/// `altgr` says the host treats Right Alt as AltGr.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ModifierState {
    pub shift_l: bool,
    pub shift_r: bool,
    pub caps: bool,
    pub ctrl_l: bool,
    pub ctrl_r: bool,
    pub alt_l: bool,
    pub alt_r: bool,
    pub altgr: bool,
    pub cmd: bool,
    /// `extra[n-1]` is component `extra`*n*.
    pub extra: [bool; 3],
}

impl ModifierState {
    /// Either Shift.
    pub fn shift() -> Self {
        ModifierState {
            shift_l: true,
            ..ModifierState::default()
        }
    }

    /// Right Alt sent as AltGr, as a TSF host sends it.
    pub fn altgr() -> Self {
        ModifierState {
            alt_r: true,
            altgr: true,
            ..ModifierState::default()
        }
    }

    pub(crate) fn any_shift(self) -> bool {
        self.shift_l || self.shift_r
    }

    pub(crate) fn any_ctrl(self) -> bool {
        self.ctrl_l || self.ctrl_r
    }

    pub(crate) fn any_alt(self) -> bool {
        self.alt_l || self.alt_r
    }
}

// [spec:kbdgen:def:ldml.engine.event]
/// A key event: a key, its modifiers and whether it is an auto-repeat. A
/// repeat is handled exactly like any other press.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeyEvent {
    pub key: Key,
    pub modifiers: ModifierState,
    pub repeat: bool,
}

impl KeyEvent {
    /// `key` with no modifiers, not a repeat.
    pub fn new(key: Key) -> Self {
        KeyEvent {
            key,
            modifiers: ModifierState::default(),
            repeat: false,
        }
    }

    /// `key` with `modifiers`, not a repeat.
    pub fn with(key: Key, modifiers: ModifierState) -> Self {
        KeyEvent {
            key,
            modifiers,
            repeat: false,
        }
    }
}

/// The text before the caret.
///
/// `text` holds at most `Model::context_len` scalar values; the engine uses
/// only the last `context_len` of a longer text, and then treats it as not
/// starting at a start of text. `authoritative` only informs diagnostics.
/// `at_start` says that `text` begins at a start of text; a host that
/// cannot tell passes `false`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct Context {
    pub text: String,
    pub authoritative: bool,
    pub at_start: bool,
}

impl Context {
    /// An authoritative context holding `text`, not known to start at a
    /// start of text.
    pub fn new(text: impl Into<String>) -> Self {
        Context {
            text: text.into(),
            authoritative: true,
            at_start: false,
        }
    }
}

// [spec:kbdgen:def:ldml.engine.state]
/// The host-owned engine state: `tail`, the last elements of the engine's
/// view of the context, markers included.
///
/// `tail` covers at most `context_len` scalar values plus any markers
/// among or after them; `State::default()`, the reset state, has an empty
/// `tail`. Hosts reset it whenever the caret moves, focus changes, text
/// changes other than by an applied `Edit`, or a key other than a lone
/// modifier passes.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct State {
    pub(crate) tail: Text,
}

impl State {
    /// The elements of the engine's view of the end of the context.
    pub fn tail(&self) -> &Text {
        &self.tail
    }
}

/// The marker references of the trailing marker run of `elems`, in order.
pub(crate) fn trailing_markers(elems: &[TextElem]) -> impl Iterator<Item = MarkerIndex> + '_ {
    let start = elems
        .iter()
        .rposition(|e| matches!(e, TextElem::Char(_)))
        .map_or(0, |i| i.saturating_add(1));
    elems.iter().skip(start).filter_map(|e| match e {
        TextElem::Marker(m) => Some(*m),
        TextElem::Char(_) => None,
    })
}

// [spec:kbdgen:def:ldml.engine.action]
/// What the host does with an event.
///
/// For `Edit`, the host deletes `delete` scalar values before the caret,
/// inserts `insert` as committed text, then shows `preedit` after the
/// caret as an uncommitted composition (empty for none): the triple
/// (*d*, *s*, *p*) of `tsf.edit.ops`. `delete` never exceeds the scalar
/// count of the `Context.text` the engine used, and neither text ever holds
/// a marker, because both are `String`s built from plain text. `layer`,
/// an Extension, is the touch layer id the host shows next.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Action {
    /// The host handles the key as if the engine were absent.
    Pass,
    Edit {
        delete: usize,
        insert: String,
        preedit: String,
        layer: Option<String>,
    },
}

// [spec:kbdgen:def:ldml.engine.output.form]
/// The form of inserted text for keyboards with normalization `Enabled`.
/// Keyboards with `Disabled` normalization insert exactly what is authored.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum OutputForm {
    #[default]
    Nfc,
    Nfd,
}

/// The default Backspace when no backspace rule matched
/// (`ldml.engine.backspace.default`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum BackspacePolicy {
    /// The macOS rule: cancel a trailing marker run, otherwise pass.
    #[default]
    CancelOrPass,
    /// LDML's default backspace transform: delete the last scalar value
    /// with the markers adjoining it.
    CodePoint,
}

/// How a [`crate::Model`] behaves where the platform chooses.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Options {
    pub output_form: OutputForm,
    pub backspace: BackspacePolicy,
    /// The host the engine runs on, for a keyboard shared by several hosts
    /// (`ldml.model.layout`), which has no host of its own. A keyboard
    /// built for one host always uses that host.
    pub host: Option<Host>,
}

/// Why a model could not be loaded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// The bytes are not an encoded keyboard (`ldml.model.decode`).
    Decode(DecodeError),
    /// The keyboard violates an invariant (`ldml.model.invariants`).
    Invalid(InvariantError),
    /// The keyboard's normalization is `Enabled`, and the crate was built
    /// without its `normalization` feature.
    NormalizationUnsupported,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Decode(e) => e.fmt(f),
            Error::Invalid(e) => write!(f, "keyboard model is invalid: {e}"),
            Error::NormalizationUnsupported => f.write_str(
                "keyboard normalization is enabled, but kbd-engine was built without its \
                 `normalization` feature",
            ),
        }
    }
}

impl core::error::Error for Error {}
