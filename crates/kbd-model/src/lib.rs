//! The keyboard model: the superset of CLDR LDML Keyboard 3.0 that the engine
//! and the native compilers read, its invariants and its binary encoding.
//!
//! The crate is `no_std` with `alloc`, and depends only on `serde` and
//! `postcard` without their std features, so it builds for every host
//! triple, wasm included. It defines neither a global allocator nor a panic
//! handler; the host binary provides both. `unsafe` is forbidden and
//! floating-point types are rejected by clippy, and without std there is no
//! hash map, so iteration order and encoding never vary by platform.

// [spec:kbdgen:req:ldml.crate.model]
// [spec:kbdgen:req:ldml.crate.targets]
#![no_std]

extern crate alloc;
