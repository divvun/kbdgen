//! The COM component (`tsf.component`): the exported entry points, the
//! text input processor object and its sinks, edit sessions and the
//! display attribute provider. Every method runs through
//! [`crate::guard::contain`] and hands the work to the host-independent
//! modules.

mod data;
mod display;
mod events;
mod exports;
mod input;
mod register;
mod session;
mod tip;

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
