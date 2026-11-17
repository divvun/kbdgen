//! The module's exports. Each is a plain function of integers over the
//! instance; addresses and lengths are `usize`, which is 32 bits on wasm.
//!
//! `no_mangle` is the crate's only unsafe construct: an exported symbol
//! must not collide with another of the same name. Every name here carries
//! the `kbd_` prefix, which nothing else in the module, `std` and the
//! engine included, defines. No export dereferences a raw pointer: the
//! buffers are the instance's own `Vec`s, whose addresses the host only
//! writes and reads through the module's memory between calls.

#![allow(unsafe_code)]

use crate::with_instance;

/// Sizes the input buffer to `len` bytes and gives its address.
#[unsafe(no_mangle)]
pub extern "C" fn kbd_input(len: usize) -> usize {
    with_instance(0, |i| i.input(len))
}

/// The address of the output buffer.
#[unsafe(no_mangle)]
pub extern "C" fn kbd_output() -> usize {
    with_instance(0, |i| i.output.as_ptr().expose_provenance())
}

#[unsafe(no_mangle)]
pub extern "C" fn kbd_output_len() -> usize {
    with_instance(0, |i| i.output.len())
}

/// How many leading bytes of the output are the inserted text; the rest is
/// the preedit.
#[unsafe(no_mangle)]
pub extern "C" fn kbd_insert_len() -> usize {
    with_instance(0, |i| i.insert_len)
}

/// The scalar values the last edit deletes before the caret.
#[unsafe(no_mangle)]
pub extern "C" fn kbd_delete() -> usize {
    with_instance(0, |i| i.delete)
}

/// Loads the `DVKB` model in the first `len` input bytes; 0 on failure,
/// with the reason as output.
#[unsafe(no_mangle)]
pub extern "C" fn kbd_load(len: usize) -> u32 {
    with_instance(0, |i| i.load(len))
}

#[unsafe(no_mangle)]
pub extern "C" fn kbd_unload(handle: u32) {
    with_instance((), |i| i.unload(handle))
}

/// The session's `Model::context_len`, or 0 for no such session.
#[unsafe(no_mangle)]
pub extern "C" fn kbd_context_len(handle: u32) -> u32 {
    with_instance(0, |i| i.context_len(handle))
}

#[unsafe(no_mangle)]
pub extern "C" fn kbd_extra_modifiers(handle: u32) -> u32 {
    with_instance(0, |i| i.extra_modifiers(handle))
}

#[unsafe(no_mangle)]
pub extern "C" fn kbd_shift_lock(handle: u32) -> u32 {
    with_instance(0, |i| i.shift_lock(handle))
}

/// Sends a key event with the context in the first `context_len` input
/// bytes; `PASS`, `EDIT` or a negative error.
#[unsafe(no_mangle)]
pub extern "C" fn kbd_key(
    handle: u32,
    key: u32,
    modifiers: u32,
    context_len: usize,
    flags: u32,
) -> i32 {
    with_instance(crate::UNAVAILABLE, |i| {
        i.key(handle, key, modifiers, context_len, flags)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn kbd_reset(handle: u32) {
    with_instance((), |i| i.reset(handle))
}

/// How many dead keys are pending: 0 for none, or for no such session.
#[unsafe(no_mangle)]
pub extern "C" fn kbd_pending(handle: u32) -> u32 {
    with_instance(0, |i| i.pending(handle))
}
