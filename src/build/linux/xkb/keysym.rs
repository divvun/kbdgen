//! Keysyms, numbered, named and mapped to characters as libxkbcommon does
//! (`ldml.linux.values`, `ldml.linux.semantics`), from the table in
//! `data/keysyms.tsv`.

use std::collections::HashMap;
use std::fmt;
use std::sync::LazyLock;

/// The offset of the Unicode keysyms, `0x01000000` + the scalar value.
const UNICODE_OFFSET: u32 = 0x0100_0000;

/// An X keysym.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Keysym(pub u32);

struct Table {
    /// Keysym → canonical name.
    names: HashMap<u32, &'static str>,
    /// Name → keysym, every name of the header.
    by_name: HashMap<&'static str, u32>,
    /// Keysym → character, `xkb_keysym_to_utf32`.
    chars: HashMap<u32, char>,
    /// Character → legacy keysym, where `xkb_utf32_to_keysym` gives one.
    legacy: HashMap<char, u32>,
}

static TABLE: LazyLock<Table> = LazyLock::new(|| {
    let mut table = Table {
        names: HashMap::new(),
        by_name: HashMap::new(),
        chars: HashMap::new(),
        legacy: HashMap::new(),
    };
    let rows = include_str!("data/keysyms.tsv")
        .lines()
        .filter(|line| !line.starts_with('#'));
    for line in rows {
        let mut columns = line.split('\t');
        let (Some(value), Some(name)) = (columns.next(), columns.next()) else {
            continue;
        };
        let Some(value) = value
            .strip_prefix("0x")
            .and_then(|hex| u32::from_str_radix(hex, 16).ok())
        else {
            continue;
        };
        table.by_name.entry(name).or_insert(value);
        table.names.entry(value).or_insert(name);
        let char = columns
            .next()
            .and_then(|hex| u32::from_str_radix(hex, 16).ok())
            .and_then(char::from_u32);
        if let Some(c) = char {
            table.chars.insert(value, c);
            if columns.next() == Some("*") {
                table.legacy.insert(c, value);
            }
        }
    }
    table
});

/// The simple uppercase mapping of `c` that libxkbcommon 1.8 and later
/// applies to keysyms, with its one exception, ß to ẞ. Where Unicode's
/// full mapping has several characters, the simple one is `c` itself but
/// for the Greek letters with ypogegrammeni, which map to their
/// prosgegrammeni forms.
fn simple_uppercase(c: char) -> char {
    if c == 'ß' {
        return 'ẞ';
    }
    let mut upper = c.to_uppercase();
    if let (Some(first), None) = (upper.next(), upper.next()) {
        return first;
    }
    let code = u32::from(c);
    let mapped = match code {
        0x1F80..=0x1F87 | 0x1F90..=0x1F97 | 0x1FA0..=0x1FA7 => code + 8,
        0x1FB3 | 0x1FC3 | 0x1FF3 => code + 9,
        _ => code,
    };
    char::from_u32(mapped).unwrap_or(c)
}

impl Keysym {
    pub const NO_SYMBOL: Keysym = Keysym(0);
    pub const VOID_SYMBOL: Keysym = Keysym(0x00ff_ffff);
    pub const KP_DELETE: Keysym = Keysym(0xff9f);
    pub const KP_SEPARATOR: Keysym = Keysym(0xffac);
    pub const KP_DECIMAL: Keysym = Keysym(0xffae);
    pub const ISO_LEVEL3_SHIFT: Keysym = Keysym(0xfe03);
    pub const SPACE: Keysym = Keysym(0x20);

    /// The keysym of `c`, as `xkb_utf32_to_keysym` gives it: the legacy
    /// keysym where there is one, else the Unicode keysym.
    // [spec:kbdgen:sem:ldml.linux.values+1]
    pub fn from_char(c: char) -> Keysym {
        let code = u32::from(c);
        match code {
            0x20..=0x7e | 0xa0..=0xff => Keysym(code),
            0x08..=0x0b | 0x0d | 0x1b => Keysym(code | 0xff00),
            0x7f => Keysym(0xffff),
            0xfdd0..=0xfdef => Keysym::NO_SYMBOL,
            _ if code & 0xfffe == 0xfffe => Keysym::NO_SYMBOL,
            _ => TABLE
                .legacy
                .get(&c)
                .map_or(Keysym(code | UNICODE_OFFSET), |value| Keysym(*value)),
        }
    }

    /// The character the keysym types, as `xkb_keysym_to_utf32` gives it.
    pub fn to_char(self) -> Option<char> {
        if let Some(c) = TABLE.chars.get(&self.0) {
            return Some(*c);
        }
        char::from_u32(self.0.checked_sub(UNICODE_OFFSET)?)
    }

    /// Whether IBus takes the keysym for a dead key (`IS_DEAD_KEY`).
    pub fn is_dead(self) -> bool {
        (0xfe50..=0xfe93).contains(&self.0)
    }

    /// The keysym's name as a symbols file writes it: a Unicode keysym as
    /// `U` and at least four uppercase hexadecimal digits, else its name in
    /// `xkbcommon-keysyms.h`.
    // [spec:kbdgen:sem:ldml.linux.values+1]
    pub fn name(self) -> String {
        if let Some(code) = self.0.checked_sub(UNICODE_OFFSET) {
            return format!("U{code:04X}");
        }
        match TABLE.names.get(&self.0) {
            Some(name) => (*name).to_owned(),
            None => format!("{:#x}", self.0),
        }
    }

    /// The keysym a symbols file or a Compose file names, as
    /// `xkb_keysym_from_name` reads it, case-sensitively.
    pub fn from_name(name: &str) -> Option<Keysym> {
        if let Some(value) = TABLE.by_name.get(name) {
            return Some(Keysym(*value));
        }
        if let Some(hex) = name.strip_prefix('U') {
            let code = u32::from_str_radix(hex, 16).ok()?;
            return match code {
                0..=0xff => (code >= 0x20 && !(0x7f..0xa0).contains(&code)).then_some(Keysym(code)),
                0x100..=0x10_ffff => Some(Keysym(code | UNICODE_OFFSET)),
                _ => None,
            };
        }
        let hex = name.strip_prefix("0x")?;
        u32::from_str_radix(hex, 16).ok().map(Keysym)
    }

    /// The keysym Lock turns this one into, `xkb_keysym_to_upper`: the
    /// keysym of its character's simple uppercase.
    // [spec:kbdgen:sem:ldml.linux.semantics+1]
    pub fn to_upper(self) -> Keysym {
        match self.to_char() {
            Some(c) if self.0 < 0xff00 || self.0 >= UNICODE_OFFSET => {
                let upper = simple_uppercase(c);
                if upper == c {
                    self
                } else {
                    Keysym::from_char(upper)
                }
            }
            _ => self,
        }
    }
}

impl fmt::Display for Keysym {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.name())
    }
}
