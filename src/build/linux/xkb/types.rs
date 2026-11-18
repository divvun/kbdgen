//! The eight states of a key, the key types the layout names, and how
//! libxkbcommon types a key of a type in a state (`ldml.linux.semantics`,
//! `ldml.linux.types`).

use std::fmt;

use super::keysym::Keysym;

pub const SHIFT: u8 = 1;
/// The Lock modifier, which Caps Lock locks.
pub const LOCK: u8 = 2;
/// The LevelThree modifier, which `level3(ralt_switch)` puts on Right Alt.
pub const LEVEL3: u8 = 4;
pub const NUM_LOCK: u8 = 8;

/// One of a key's eight states (*s*, *c*, *g*): Shift, Lock and Level3.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct XkbState(u8);

impl XkbState {
    /// Every state, Shift the lowest bit, then Lock, then Level3.
    pub const ALL: [XkbState; 8] = [
        XkbState(0),
        XkbState(1),
        XkbState(2),
        XkbState(3),
        XkbState(4),
        XkbState(5),
        XkbState(6),
        XkbState(7),
    ];

    pub fn shift(self) -> bool {
        self.0 & SHIFT != 0
    }

    pub fn lock(self) -> bool {
        self.0 & LOCK != 0
    }

    pub fn level3(self) -> bool {
        self.0 & LEVEL3 != 0
    }

    pub fn mods(self) -> u8 {
        self.0
    }

    /// The state's index in [`XkbState::ALL`].
    pub fn index(self) -> usize {
        usize::from(self.0)
    }
}

impl fmt::Display for XkbState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let names: Vec<&str> = [
            (self.shift(), "Shift"),
            (self.lock(), "Caps Lock"),
            (self.level3(), "AltGr"),
        ]
        .into_iter()
        .filter_map(|(on, name)| on.then_some(name))
        .collect();
        if names.is_empty() {
            f.write_str("no modifier")
        } else {
            f.write_str(&names.join("+"))
        }
    }
}

/// A `map` entry of a type: the modifiers it matches, the level it
/// selects and the modifiers it preserves.
struct Entry {
    mods: u8,
    level: usize,
    preserve: u8,
}

const fn entry(mods: u8, level: usize) -> Entry {
    Entry {
        mods,
        level,
        preserve: 0,
    }
}

const S: u8 = SHIFT;
const L: u8 = LOCK;
const G: u8 = LEVEL3;

const ONE_LEVEL: &[Entry] = &[entry(0, 0)];
const TWO_LEVEL: &[Entry] = &[entry(S, 1)];
const ALPHABETIC: &[Entry] = &[entry(S, 1), entry(L, 1)];
const FOUR_LEVEL: &[Entry] = &[entry(0, 0), entry(S, 1), entry(G, 2), entry(S | G, 3)];
const FOUR_LEVEL_ALPHABETIC: &[Entry] = &[
    entry(0, 0),
    entry(S, 1),
    entry(L, 1),
    entry(S | L, 0),
    entry(G, 2),
    entry(S | G, 3),
    entry(L | G, 3),
    entry(S | L | G, 2),
];
const FOUR_LEVEL_SEMIALPHABETIC: &[Entry] = &[
    entry(0, 0),
    entry(S, 1),
    entry(L, 1),
    entry(S | L, 0),
    entry(G, 2),
    entry(S | G, 3),
    Entry {
        mods: L | G,
        level: 2,
        preserve: L,
    },
    Entry {
        mods: S | L | G,
        level: 3,
        preserve: L,
    },
];
const FOUR_LEVEL_PLUS_LOCK: &[Entry] = &[
    entry(0, 0),
    entry(S, 1),
    entry(G, 2),
    entry(S | G, 3),
    entry(L, 4),
    entry(S | L, 1),
    entry(L | G, 2),
    entry(S | L | G, 3),
];
const KEYPAD: &[Entry] = &[entry(0, 0), entry(NUM_LOCK, 1)];

/// A key type of xkeyboard-config's `complete` set, as `types/basic`,
/// `types/extra` and `types/numpad` define it; levels count from 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyType {
    OneLevel,
    TwoLevel,
    Alphabetic,
    FourLevel,
    FourLevelSemialphabetic,
    FourLevelAlphabetic,
    FourLevelPlusLock,
    Keypad,
}

impl KeyType {
    /// The types a key may take, in the order of `ldml.linux.types`, those
    /// with Level3 only when the model uses it.
    pub fn candidates(uses_level3: bool) -> Vec<KeyType> {
        use KeyType::*;
        let mut types = vec![OneLevel, TwoLevel, Alphabetic];
        if uses_level3 {
            types.extend([FourLevel, FourLevelSemialphabetic, FourLevelAlphabetic]);
        }
        types.push(FourLevelPlusLock);
        types
    }

    pub fn name(self) -> &'static str {
        match self {
            KeyType::OneLevel => "ONE_LEVEL",
            KeyType::TwoLevel => "TWO_LEVEL",
            KeyType::Alphabetic => "ALPHABETIC",
            KeyType::FourLevel => "FOUR_LEVEL",
            KeyType::FourLevelSemialphabetic => "FOUR_LEVEL_SEMIALPHABETIC",
            KeyType::FourLevelAlphabetic => "FOUR_LEVEL_ALPHABETIC",
            KeyType::FourLevelPlusLock => "FOUR_LEVEL_PLUS_LOCK",
            KeyType::Keypad => "KEYPAD",
        }
    }

    pub fn from_name(name: &str) -> Option<KeyType> {
        use KeyType::*;
        [
            OneLevel,
            TwoLevel,
            Alphabetic,
            FourLevel,
            FourLevelSemialphabetic,
            FourLevelAlphabetic,
            FourLevelPlusLock,
            Keypad,
        ]
        .into_iter()
        .find(|ty| ty.name() == name)
    }

    /// How many levels the type has.
    pub fn levels(self) -> usize {
        match self {
            KeyType::OneLevel => 1,
            KeyType::TwoLevel | KeyType::Alphabetic | KeyType::Keypad => 2,
            KeyType::FourLevelPlusLock => 5,
            _ => 4,
        }
    }

    /// The state whose value level `level` holds (`ldml.linux.types`).
    pub fn level_state(level: usize) -> XkbState {
        let mods = match level {
            1 => SHIFT,
            2 => LEVEL3,
            3 => SHIFT | LEVEL3,
            4 => LOCK,
            _ => 0,
        };
        XkbState(mods)
    }

    fn mods(self) -> u8 {
        match self {
            KeyType::OneLevel => 0,
            KeyType::TwoLevel => SHIFT,
            KeyType::Alphabetic => SHIFT | LOCK,
            KeyType::FourLevel => SHIFT | LEVEL3,
            KeyType::Keypad => NUM_LOCK,
            _ => SHIFT | LOCK | LEVEL3,
        }
    }

    fn entries(self) -> &'static [Entry] {
        match self {
            KeyType::OneLevel => ONE_LEVEL,
            KeyType::TwoLevel => TWO_LEVEL,
            KeyType::Alphabetic => ALPHABETIC,
            KeyType::FourLevel => FOUR_LEVEL,
            KeyType::FourLevelAlphabetic => FOUR_LEVEL_ALPHABETIC,
            KeyType::FourLevelSemialphabetic => FOUR_LEVEL_SEMIALPHABETIC,
            KeyType::FourLevelPlusLock => FOUR_LEVEL_PLUS_LOCK,
            KeyType::Keypad => KEYPAD,
        }
    }

    /// What a key of this type with `levels` types under the modifiers
    /// `mods`: the level the matching entry selects, Level1 when none
    /// matches, uppercased when Lock is active and the type does not
    /// consume it (`src/state.c`).
    // [spec:kbdgen:sem:ldml.linux.semantics+1]
    pub fn resolve(self, levels: &[Keysym], mods: u8) -> Keysym {
        let active = mods & self.mods();
        let (level, preserve) = self
            .entries()
            .iter()
            .find(|entry| entry.mods == active)
            .map_or((0, 0), |entry| (entry.level, entry.preserve));
        let consumed = self.mods() & !preserve;
        let keysym = levels.get(level).copied().unwrap_or(Keysym::NO_SYMBOL);
        if mods & LOCK != 0 && consumed & LOCK == 0 {
            keysym.to_upper()
        } else {
            keysym
        }
    }
}

/// A key's type and levels, and the states the type gets wrong.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Typed {
    pub key_type: KeyType,
    pub levels: Vec<Keysym>,
    pub wrong: Vec<XkbState>,
}

/// The type of a key whose states have `values`: the first candidate that
/// gives every state that can occur, else the one that gives the most, the
/// earliest on a tie. Without Level3 in the model, no state with Level3
/// can occur.
// [spec:kbdgen:sem:ldml.linux.types+1]
pub fn choose(values: &[Keysym; 8], uses_level3: bool) -> Typed {
    let states: Vec<XkbState> = XkbState::ALL
        .into_iter()
        .filter(|state| uses_level3 || !state.level3())
        .collect();
    let mut best: Option<Typed> = None;
    for key_type in KeyType::candidates(uses_level3) {
        let levels: Vec<Keysym> = (0..key_type.levels())
            .map(|level| values[KeyType::level_state(level).index()])
            .collect();
        let wrong: Vec<XkbState> = states
            .iter()
            .copied()
            .filter(|state| key_type.resolve(&levels, state.mods()) != values[state.index()])
            .collect();
        let typed = Typed {
            key_type,
            levels,
            wrong,
        };
        if typed.wrong.is_empty() {
            return typed;
        }
        if best
            .as_ref()
            .is_none_or(|b| typed.wrong.len() < b.wrong.len())
        {
            best = Some(typed);
        }
    }
    best.unwrap_or(Typed {
        key_type: KeyType::OneLevel,
        levels: vec![values[0]],
        wrong: Vec::new(),
    })
}
