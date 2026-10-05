//! LDML keyboard3 support: source documents read and written through
//! `xmlem`, the embedded CLDR imports, parsers for LDML's syntaxes,
//! resolution to the `kbd-model` model, and export.
//!
//! This is the std half of the split: XML and LDML's syntaxes are handled
//! once, at build time, so devices carry only `kbd-model` and `kbd-engine`.

// [spec:kbdgen:def:ldml.crate.layout]
