//! The Divvun keyboard IBus engine (`docs/spec/linux.md`, "IBus engine"):
//! one program, `ibus-engine-divvun`, shared by every keyboard kbdgen
//! builds, that runs `kbd-engine` on the model of each installed keyboard.
//! Keyboards supply only data (`linux.ibus.data`); the program is released
//! and installed separately, with the component file `data/divvun.xml.in`
//! that packaging completes.
//!
//! The library is the program but for its command line: the keyboards'
//! data (`data`), the D-Bus front end ibus-daemon talks to (`dbus`), and
//! beneath it the host logic, free of D-Bus, that a Fcitx5 addon or a
//! Stevia completer would reuse (`planner`, `keys`, `context`).

// [spec:kbdgen:def:linux.ibus.component]
// [spec:kbdgen:req:linux.ibus.crate+1]

pub mod address;
pub mod context;
pub mod data;
#[cfg(unix)]
pub mod dbus;
pub mod diag;
pub mod guard;
pub mod keys;
pub mod keysym;
// [spec:kbdgen:req:linux.ibus.planner]
pub mod planner;
#[cfg(test)]
mod tests;
