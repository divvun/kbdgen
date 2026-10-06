//! The Divvun keyboard text service: a Text Services Framework text input
//! processor that runs `kbd-engine` on the model carried by each layout
//! DLL (`docs/spec/tsf.md`).
//!
//! The Windows build is one in-process COM server shared by every Divvun
//! keyboard. Each keyboard adds a language profile whose GUID is its
//! layout's product code; on activation the text service finds that
//! layout, reads its model resource and maps keys itself, from scan codes,
//! so it never relies on the dummy layout Windows puts beneath it.
//!
//! The logic that needs no TSF call (key identity, claims, the edit
//! planner, the per-context document with its cache and resets, locating
//! data, panic containment) lives in host-independent modules and is
//! tested on every host. The COM component, under `win`, only adapts TSF
//! to them.

// [spec:kbdgen:req:ldml.crate.tsf+1]
// [spec:kbdgen:req:tsf.component.crate]
// [spec:kbdgen:def:tsf.engine.api]
// [spec:kbdgen:sem:tsf.pairing.substitute]
// [spec:kbdgen:sem:tsf.pairing.alternatives]
// [spec:kbdgen:sem:tsf.security.integrity]

pub mod claim;
pub mod document;
pub mod guard;
pub mod guid;
pub mod keys;
pub mod locate;
// [spec:kbdgen:req:tsf.test.host]
pub mod planner;
pub mod server;
pub mod text;

#[cfg(windows)]
mod win;

// [spec:kbdgen:req:tsf.test.host]
#[cfg(test)]
mod tests;
