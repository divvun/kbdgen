//! The keyboard engine: turns key events into edit operations against a
//! [`kbd_model`] keyboard, deterministically, on every host.
//!
//! [`Model::key`] is a pure function from a model, a host-owned [`State`],
//! the text before the caret ([`Context`]) and a [`KeyEvent`] to an
//! [`Action`] and the next state. Hosts own all mutable state; the API
//! uses only plain data (integers, `bool`, strings, `Vec`s and enums), with
//! no callbacks or trait objects, and reports every failure as an
//! [`Error`].
//!
//! The crate is `no_std` with `alloc`, and depends only on `kbd-model` and,
//! behind the default `normalization` feature, on `icu_normalizer` with its
//! compiled data. So it builds for every host triple, wasm included, and
//! has no access to I/O, clocks, randomness or threads. It defines neither a
//! global allocator nor a panic handler; the host binary provides both.
//!
//! The crate's lints keep the API thin enough for C, JNI and wasm bindings:
//! `unsafe` is forbidden, so there is no `static mut`; clippy rejects
//! atomics, so there is no other global mutable state, and all mutable state
//! lives in the host-owned `State`; clippy rejects `Rc` and the cell types,
//! so public types stay `Send` and `Sync`; and clippy rejects `panic!`,
//! `unwrap`, `expect`, `unreachable!` and slice indexing, so every failure
//! is returned as an error.

// [spec:kbdgen:req:ldml.crate.engine]
// [spec:kbdgen:req:ldml.crate.targets]
// [spec:kbdgen:req:ldml.crate.ffi]
#![no_std]

extern crate alloc;

mod api;
mod matcher;
mod model;
mod modifiers;
mod normalize;
mod reorder;
mod transforms;

#[cfg(test)]
mod tests;

pub use api::{
    Action, BackspacePolicy, Context, Error, Gesture, Key, KeyEvent, ModifierState, Options,
    OutputForm, State,
};
pub use model::Model;
