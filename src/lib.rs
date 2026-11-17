//! kbdgen: keyboard bundles and the targets generated from them.
//!
//! The `kbdgen` package is the root of a Cargo workspace whose members live
//! in `crates/*`: the `no_std` keyboard model (`kbd-model`) and engine
//! (`kbd-engine`), and LDML keyboard3 support (`kbd-ldml`). kbdgen depends
//! on all three by path, and its build script compiles a fourth, `kbd-wasm`,
//! to the WebAssembly module the ChromeOS extension runs. Layout crates
//! generated for Windows declare their own empty `[workspace]`, so this
//! workspace never captures them.

// [spec:kbdgen:def:ldml.crate.layout+2]
// [spec:kbdgen:req:ldml.crate.kbdgen+1]
pub mod build;
pub mod bundle;
pub mod ldml;
pub mod util;
