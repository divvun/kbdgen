//! The keyboard model: the superset of CLDR LDML Keyboard 3.0 that the engine
//! and the native compilers read, its invariants and its binary encoding.
//!
//! A [`Keyboard`] is stored after resolution, so nothing is left to resolve
//! on a device: imports are spliced in, variables substituted, escapes
//! decoded, markers interned into a table and referenced out of band
//! ([`Text`]), transform syntax parsed into [`Pattern`]s, and reorder rules
//! merged and sorted. [`Keyboard::validate`] checks the invariants, and
//! [`Keyboard::to_bytes`] and [`Keyboard::from_bytes`] give the `DVKB`
//! binary form that layout DLLs and app bundles embed.
//!
//! The crate is `no_std` with `alloc`, and depends only on `serde` and
//! `postcard` without their std features, so it builds for every host
//! triple, wasm included. It defines neither a global allocator nor a panic
//! handler; the host binary provides both. `unsafe` is forbidden and
//! floating-point types are rejected by clippy, and without std there is no
//! hash map, so iteration order and encoding never vary by platform. Clippy
//! also rejects panicking calls, slice indexing and unchecked arithmetic,
//! so validation and decoding report every failure as an error. The crate
//! carries no Unicode data: NFD checks go through a caller's [`NfdCheck`].

// [spec:kbdgen:req:ldml.crate.model]
// [spec:kbdgen:req:ldml.crate.targets]
#![no_std]

extern crate alloc;

mod encoding;
mod extensions;
mod keyboard;
mod layers;
mod layout;
mod modifiers;
mod text;
mod transforms;
mod validate;

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod tests;

pub use encoding::{
    DecodeError, EncodeError, HEADER_LEN, MAGIC, MAJOR_VERSION, MINOR_VERSION, Version, read_header,
};
pub use extensions::{ExtraModifierKey, WINDOWS_KEY_NAMES, Windows};
pub use keyboard::{
    DEFAULT_WIDTH, Direction, Display, DisplayTarget, Displays, Flick, FlickIndex, FlickSegment,
    Host, Info, Key, KeyIndex, Keyboard, Labels, MAX_CONTEXT_LEN, Normalization, Role,
};
pub use layers::{BottomRow, Form, Hardware, HardwareLayer, ScanCode, TouchLayer, TouchSet};
pub use layout::{Layout, LayoutError};
pub use modifiers::{Component, ModifierSet, Modifiers};
pub use text::{MarkerIndex, Text, TextElem, is_nmtoken};
pub use transforms::{
    Alternation, Atom, Class, ClassIndex, ClassRange, Fixed, Item, MAX_CAPTURES, MAX_REPEAT,
    NodeIndex, Pattern, PatternError, PatternInfo, ReorderClass, ReorderRule, Replacement,
    ReplacementItem, Rule, Sequence, Set, SetIndex, TransformGroup, TreeAtom, TreeItem,
};
pub use validate::{Invariant, InvariantError, NfdCheck, Site, TransformList};
