//! Extension data: the Windows options.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

/// A physical key that can be bound as an extra modifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ExtraModifierKey {
    RightCtrl,
    CapsLock,
    B00,
}

impl ExtraModifierKey {
    /// The name v4 YAML and the kbdgen XML namespace use.
    pub const fn name(self) -> &'static str {
        match self {
            ExtraModifierKey::RightCtrl => "rightCtrl",
            ExtraModifierKey::CapsLock => "capsLock",
            ExtraModifierKey::B00 => "B00",
        }
    }
}

/// The English entry names of `kbdl.key-names`, `aKeyNames` then
/// `aKeyNamesExt`, which `Windows::key_names` may rename.
pub const WINDOWS_KEY_NAMES: &[&str] = &[
    "Esc",
    "Backspace",
    "Tab",
    "Enter",
    "Ctrl",
    "Shift",
    "Right Shift",
    "Num *",
    "Alt",
    "Space",
    "Caps Lock",
    "F1",
    "F2",
    "F3",
    "F4",
    "F5",
    "F6",
    "F7",
    "F8",
    "F9",
    "F10",
    "Pause",
    "Scroll Lock",
    "Num 7",
    "Num 8",
    "Num 9",
    "Num -",
    "Num 4",
    "Num 5",
    "Num 6",
    "Num +",
    "Num 1",
    "Num 2",
    "Num 3",
    "Num 0",
    "Num Del",
    "Sys Req",
    "F11",
    "F12",
    "F13",
    "F14",
    "F15",
    "F16",
    "F17",
    "F18",
    "F19",
    "F20",
    "F21",
    "F22",
    "F23",
    "F24",
    "Num Enter",
    "Right Ctrl",
    "Num /",
    "Prnt Scrn",
    "Right Alt",
    "Num Lock",
    "Break",
    "Home",
    "Up",
    "Page Up",
    "Left",
    "Right",
    "End",
    "Down",
    "Page Down",
    "Insert",
    "Delete",
    "<00>",
    "Help",
    "Left Windows",
    "Right Windows",
    "Application",
];

// [spec:kbdgen:def:ldml.model.windows]
/// Extension: `kbdl.input`'s Windows-only parts, read by the engine on
/// Windows and by the `kbdl` adapter.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Windows {
    /// At most three distinct keys; the *i*-th (0-based) binds component
    /// `extra`*i+1*.
    pub extra_modifiers: Vec<ExtraModifierKey>,
    pub shift_lock: bool,
    pub lrm_rlm: bool,
    /// Entry name of [`WINDOWS_KEY_NAMES`] → replacement name.
    pub key_names: BTreeMap<String, String>,
}
