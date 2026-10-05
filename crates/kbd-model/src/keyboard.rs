//! The keyboard and its tables.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use crate::extensions::{Emoji, EmojiKey, Windows};
use crate::layers::{Hardware, TouchSet};
use crate::text::{MarkerIndex, Text};
use crate::transforms::{Class, Set, TransformGroup};

/// An index into `Keyboard::keys`.
pub type KeyIndex = u16;
/// An index into `Keyboard::flicks`.
pub type FlickIndex = u16;

/// The largest `context_len` a keyboard may have, because a host context
/// holds at most 64 scalar values (`tsf.engine.api`).
pub const MAX_CONTEXT_LEN: u8 = 64;

/// The host a keyboard is built for. The declaration order is the host
/// order of `ldml.yaml.hosts`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Host {
    Windows,
    MacOs,
    ChromeOs,
    Linux,
    Ios,
    Android,
    Web,
}

impl Host {
    pub const ALL: [Host; 7] = [
        Host::Windows,
        Host::MacOs,
        Host::ChromeOs,
        Host::Linux,
        Host::Ios,
        Host::Android,
        Host::Web,
    ];

    /// The host's name in v4 YAML, XML and file names.
    pub const fn name(self) -> &'static str {
        match self {
            Host::Windows => "windows",
            Host::MacOs => "macOS",
            Host::ChromeOs => "chromeOS",
            Host::Linux => "linux",
            Host::Ios => "iOS",
            Host::Android => "android",
            Host::Web => "web",
        }
    }

    pub fn from_name(name: &str) -> Option<Host> {
        Host::ALL.into_iter().find(|h| h.name() == name)
    }
}

/// LDML `info`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Info {
    pub name: String,
    pub author: Option<String>,
    pub layout: Option<String>,
    pub indicator: Option<String>,
    pub attribution: Option<String>,
}

/// LDML `settings@normalization`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Normalization {
    /// LDML's default: texts are NFD and matching is canonical.
    #[default]
    Enabled,
    /// Texts are the authored scalar values, unchanged.
    Disabled,
}

/// Extension: a touch key that the host draws and handles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Role {
    Shift,
    Backspace,
    Return,
    Tab,
    Caps,
    Keyboard,
    Symbols,
    ShiftSymbols,
}

impl Role {
    pub const ALL: [Role; 8] = [
        Role::Shift,
        Role::Backspace,
        Role::Return,
        Role::Tab,
        Role::Caps,
        Role::Keyboard,
        Role::Symbols,
        Role::ShiftSymbols,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Role::Shift => "shift",
            Role::Backspace => "backspace",
            Role::Return => "return",
            Role::Tab => "tab",
            Role::Caps => "caps",
            Role::Keyboard => "keyboard",
            Role::Symbols => "symbols",
            Role::ShiftSymbols => "shiftSymbols",
        }
    }

    pub fn from_name(name: &str) -> Option<Role> {
        Role::ALL.into_iter().find(|r| r.name() == name)
    }
}

/// A key width of one key, in thousandths.
pub const DEFAULT_WIDTH: u32 = 1000;

// [spec:kbdgen:def:ldml.model.keys]
/// A key of the key table.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Key {
    /// An NMTOKEN, unique in the table.
    pub id: String,
    /// Empty when absent.
    pub output: Text,
    pub gap: bool,
    /// A touch layer id.
    pub layer_id: Option<String>,
    /// In thousandths of a key width.
    pub width: u32,
    pub stretch: bool,
    pub long_press: Vec<KeyIndex>,
    /// One of `long_press`.
    pub long_press_default: Option<KeyIndex>,
    /// Never the key itself.
    pub multi_tap: Vec<KeyIndex>,
    pub flick: Option<FlickIndex>,
    /// Extension.
    pub role: Option<Role>,
}

impl Key {
    /// A key with output `output` and every other attribute at its default.
    pub fn new(id: impl Into<String>, output: Text) -> Self {
        Key {
            id: id.into(),
            output,
            gap: false,
            layer_id: None,
            width: DEFAULT_WIDTH,
            stretch: false,
            long_press: Vec::new(),
            long_press_default: None,
            multi_tap: Vec::new(),
            flick: None,
            role: None,
        }
    }

    /// The implied `gap` key.
    pub fn gap(id: impl Into<String>) -> Self {
        Key {
            gap: true,
            ..Key::new(id, Text::new())
        }
    }
}

/// A flick direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Direction {
    N,
    Ne,
    E,
    Se,
    S,
    Sw,
    W,
    Nw,
}

impl Direction {
    pub const ALL: [Direction; 8] = [
        Direction::N,
        Direction::Ne,
        Direction::E,
        Direction::Se,
        Direction::S,
        Direction::Sw,
        Direction::W,
        Direction::Nw,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Direction::N => "n",
            Direction::Ne => "ne",
            Direction::E => "e",
            Direction::Se => "se",
            Direction::S => "s",
            Direction::Sw => "sw",
            Direction::W => "w",
            Direction::Nw => "nw",
        }
    }

    pub fn from_name(name: &str) -> Option<Direction> {
        Direction::ALL.into_iter().find(|d| d.name() == name)
    }
}

/// A flick segment: a direction sequence and its target key.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FlickSegment {
    /// Non-empty.
    pub directions: Vec<Direction>,
    pub key: KeyIndex,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Flick {
    /// An NMTOKEN, unique in the table.
    pub id: String,
    pub segments: Vec<FlickSegment>,
}

/// What a display entry applies to.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DisplayTarget {
    /// Keys with this output; markers allowed.
    Output(Text),
    /// LDML `keyId`.
    Key(KeyIndex),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Display {
    pub target: DisplayTarget,
    /// Decoded, with variables substituted, never normalized.
    pub display: String,
}

/// Extension: label strings that hosts show on the space and return keys.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Labels {
    pub space: Option<String>,
    pub r#return: Option<String>,
}

// [spec:kbdgen:def:ldml.model.displays+1]
/// LDML `displays` and `displayOptions`, plus the Extension labels.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Displays {
    /// In document order. Where targets repeat, the last entry wins.
    pub entries: Vec<Display>,
    /// `displayOptions@baseCharacter`.
    pub display_base: Option<String>,
    pub labels: Labels,
}

impl Displays {
    /// The display for keys with output `output`: the last entry for it.
    pub fn for_output(&self, output: &Text) -> Option<&str> {
        self.entries.iter().rev().find_map(|d| match &d.target {
            DisplayTarget::Output(t) if t == output => Some(d.display.as_str()),
            _ => None,
        })
    }

    /// The display for key `key` by `keyId`: the last entry for it.
    pub fn for_key(&self, key: KeyIndex) -> Option<&str> {
        self.entries.iter().rev().find_map(|d| match d.target {
            DisplayTarget::Key(k) if k == key => Some(d.display.as_str()),
            _ => None,
        })
    }
}

// [spec:kbdgen:def:ldml.model.keyboard+1]
/// A keyboard: a resolved LDML `keyboard3` document plus kbdgen's
/// Extensions. Every index refers to a table of the same keyboard.
///
/// The field order is the encoding order (`ldml.model.encoding`); a minor
/// version may only append fields.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Keyboard {
    /// Extension: none for a keyboard read from foreign XML, or one shared
    /// by several hosts (`ldml.model.layout`).
    pub host: Option<Host>,
    /// A BCP 47 tag.
    pub locale: String,
    /// Additional BCP 47 tags, in order.
    pub locales: Vec<String>,
    /// 45–49.
    pub conforms_to: u8,
    /// A semantic version.
    pub version: Option<String>,
    pub info: Info,
    pub normalization: Normalization,
    /// Marker names (NMTOKEN), in order of first appearance.
    pub markers: Vec<String>,
    /// Implied keys first, then the rest in document order. Keys that no
    /// row references are kept, because tests press keys by id.
    pub keys: Vec<Key>,
    pub flicks: Vec<Flick>,
    pub displays: Displays,
    pub hardware: Option<Hardware>,
    pub touch: Vec<TouchSet>,
    pub sets: Vec<Set>,
    pub classes: Vec<Class>,
    pub simple: Vec<TransformGroup>,
    pub backspace: Vec<TransformGroup>,
    /// See [`Keyboard::computed_context_len`].
    pub context_len: u8,
    /// Extension: the numpad decimal key's output.
    pub decimal: Option<Text>,
    // [spec:kbdgen:def:ldml.model.flush+1]
    /// Extension: marker → the plain text a pending marker stands for when
    /// input is interrupted. The engine shows it as preedit and commits it.
    pub flush: BTreeMap<MarkerIndex, String>,
    /// Extension: marker → Windows dead-key name.
    pub dead_key_names: BTreeMap<MarkerIndex, String>,
    /// Extension.
    pub windows: Windows,
    /// Extension.
    pub emoji: Emoji,
}

impl Keyboard {
    /// A keyboard with no keys, layers or transforms.
    pub fn new(locale: impl Into<String>, conforms_to: u8, info: Info) -> Self {
        Keyboard {
            host: None,
            locale: locale.into(),
            locales: Vec::new(),
            conforms_to,
            version: None,
            info,
            normalization: Normalization::Enabled,
            markers: Vec::new(),
            keys: Vec::new(),
            flicks: Vec::new(),
            displays: Displays::default(),
            hardware: None,
            touch: Vec::new(),
            sets: Vec::new(),
            classes: Vec::new(),
            simple: Vec::new(),
            backspace: Vec::new(),
            context_len: 1,
            decimal: None,
            flush: BTreeMap::new(),
            dead_key_names: BTreeMap::new(),
            windows: Windows::default(),
            emoji: Emoji::default(),
        }
    }

    pub fn key_index(&self, id: &str) -> Option<KeyIndex> {
        let i = self.keys.iter().position(|k| k.id == id)?;
        KeyIndex::try_from(i).ok()
    }

    pub fn key(&self, index: KeyIndex) -> Option<&Key> {
        self.keys.get(usize::from(index))
    }

    pub fn marker_index(&self, name: &str) -> Option<MarkerIndex> {
        let i = self.markers.iter().position(|m| m == name)?;
        MarkerIndex::try_from(i).ok()
    }

    pub fn marker_name(&self, index: MarkerIndex) -> Option<&str> {
        self.markers.get(usize::from(index)).map(String::as_str)
    }

    /// The preserved key of `ldml.model.emoji`.
    pub fn preserved_key(&self) -> Option<EmojiKey> {
        self.emoji.key
    }
}
