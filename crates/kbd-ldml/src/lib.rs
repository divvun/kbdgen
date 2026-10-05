//! LDML keyboard3 support: source documents read and written through
//! `xmlem`, the embedded CLDR imports, parsers for LDML's syntaxes,
//! resolution to the `kbd-model` model, and export.
//!
//! This is the std half of the split: XML and LDML's syntaxes are handled
//! once, at build time, so devices carry only `kbd-model` and `kbd-engine`.
//! Every XML byte goes through `xmlem` (`ldml.crate.ldml`): [`read`] gives
//! a [`SourceDocument`], [`resolve()`] turns one into a model, [`export()`]
//! builds one from a model, and [`write()`] serializes one.

// [spec:kbdgen:def:ldml.crate.layout+1]
// [spec:kbdgen:req:ldml.crate.ldml]

pub mod cldr;
mod diag;
pub mod escape;
mod export;
mod gencat;
pub mod keyboard_test;
mod nfd;
pub mod read;
mod resolve;
mod special;
pub mod syntax;
mod tree;
mod write;

pub use diag::{Diagnostic, Error, Result};
pub use export::{LayoutData, encode_width, export, is_kbdgen_layer, replace_extensions, set_host};
pub use gencat::is_mark;
pub use keyboard_test::{KeyboardTest, read_keyboard_test, read_keyboard_test_file};
pub use nfd::{IcuNfd, nfd_str};
pub use read::{SourceDocument, read_document, read_import, read_keyboard, read_keyboard_file};
pub use resolve::{Resolved, implied_form, implied_keys, parse_width, resolve};
pub use resolve::{encode_modifiers, parse_modifiers};
pub use special::{
    Compose, ComposeValue, DeadKey, Extensions, Generated, KBDGEN_NS, KBDGEN_PREFIX, Target,
};
pub use write::write;

#[cfg(test)]
mod tests;
