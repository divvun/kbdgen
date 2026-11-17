//! The COM component (`tsf.component`): the exported entry points, the
//! text input processor object and its sinks, edit sessions and the
//! display attribute provider. Every method runs through
//! [`crate::guard::contain`] and hands the work to the host-independent
//! modules.
//!
//! The raw pointers a COM method receives are, by COM's contract, null or
//! valid for the access their interface documents, which is what makes
//! `as_ref` and `as_mut` on them sound. Most other `unsafe` blocks only
//! call COM methods or Win32 functions with Rust-owned arguments.

mod data;
mod display;
mod events;
mod exports;
mod facts;
mod input;
mod register;
mod session;
mod tip;
pub(crate) mod trace;

#[cfg(test)]
mod tests;

use windows::Win32::Foundation::E_UNEXPECTED;
use windows_core::Error;

use crate::guard::Panicked;
use crate::text::Failed;

impl From<Panicked> for Error {
    fn from(_: Panicked) -> Error {
        Error::from(E_UNEXPECTED)
    }
}

impl From<Error> for Failed {
    fn from(_: Error) -> Failed {
        Failed
    }
}

/// `text` as a NUL-terminated UTF-16 string.
fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain([0]).collect()
}
