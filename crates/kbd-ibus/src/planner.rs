//! The host logic of one engine object (`linux.ibus.planner`), with no
//! D-Bus calls: what each call ibus-daemon makes does to the engine state,
//! and which signals it emits, in order, before its reply. `crate::dbus`
//! only decodes the calls and encodes the signals; a Fcitx5 addon or a
//! Stevia completer would drive the same [`Session`].

use kbd_engine::Model;

/// `IBUS_ENGINE_PREEDIT_*`: what the client does with a shown preedit when
/// the input context resets or loses focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[expect(dead_code, reason = "edits (linux.ibus.edit) show preedits")]
pub enum PreeditMode {
    /// Drop it.
    Clear = 0,
    /// Commit it.
    Commit = 1,
}

/// A signal of `org.freedesktop.IBus.Engine`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[expect(dead_code, reason = "edits (linux.ibus.edit) emit them")]
#[expect(
    clippy::enum_variant_names,
    reason = "the variants are named as the D-Bus signals"
)]
pub enum Signal {
    CommitText(String),
    /// The preedit, its cursor in scalar values, whether it is shown.
    UpdatePreeditText {
        text: String,
        cursor: u32,
        visible: bool,
        mode: PreeditMode,
    },
    /// Delete `nchars` scalar values from `offset` relative to the caret.
    DeleteSurroundingText {
        offset: i32,
        nchars: u32,
    },
    RequireSurroundingText,
}

/// What a `ProcessKeyEvent` call gives: the signals, then the reply.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Reply {
    /// The engine handled the key, so the client does not type it.
    pub handled: bool,
    pub signals: Vec<Signal>,
}

/// One engine object's state.
#[derive(Debug)]
pub struct Session {
    #[expect(dead_code, reason = "key handling (linux.ibus.keys) reads it")]
    model: Model,
}

impl Session {
    pub fn new(model: Model) -> Session {
        Session { model }
    }

    pub fn process_key_event(&mut self, _keyval: u32, _keycode: u32, _state: u32) -> Reply {
        Reply::default()
    }

    pub fn focus_in(&mut self) -> Vec<Signal> {
        Vec::new()
    }

    pub fn focus_out(&mut self) -> Vec<Signal> {
        Vec::new()
    }

    pub fn reset(&mut self) -> Vec<Signal> {
        Vec::new()
    }

    pub fn enable(&mut self) -> Vec<Signal> {
        Vec::new()
    }

    pub fn disable(&mut self) -> Vec<Signal> {
        Vec::new()
    }

    pub fn set_capabilities(&mut self, _capabilities: u32) {}

    pub fn set_content_type(&mut self, _purpose: u32, _hints: u32) {}

    /// A reading of the client's text, positions in scalar values.
    pub fn set_surrounding_text(&mut self, _text: &str, _cursor: u32, _anchor: u32) {}
}
