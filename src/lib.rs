//! kbdgen: keyboard bundles and the targets generated from them.
//!
//! The `kbdgen` package is the root of a Cargo workspace whose members live
//! in `crates/*`: the `no_std` keyboard model (`kbd-model`) and engine
//! (`kbd-engine`), and LDML keyboard3 support (`kbd-ldml`). kbdgen depends
//! on all three by path. Layout crates generated for Windows declare their
//! own empty `[workspace]`, so this workspace never captures them.

// [spec:kbdgen:def:ldml.crate.layout+1]
// [spec:kbdgen:req:ldml.crate.kbdgen]
pub mod build;
pub mod bundle;
pub mod ldml;
pub mod util;
