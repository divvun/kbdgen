//! The on-screen keyboards of the Linux target (`linux.osk.*`). Every OSK
//! shows one presented set, the layout's desktop layout as touch
//! (`presented`), and reports what it cannot show or type (`classify`);
//! `plasma` writes it as a plasma-keyboard package.

use anyhow::{Result, anyhow};

use super::source::LinuxLayout;

pub mod classify;
pub mod plasma;
pub mod presented;
#[cfg(test)]
mod tests;

pub use self::classify::{Classification, Osk, classify, inform, report};
pub use self::presented::{
    ADDED_KEY_WIDTH, DeadKey, Item, Level, LevelKind, Presented, TextKey, present,
};

/// The layout's display name under its primary language subtag
/// (`bundle.layouts.autonym`), which names it on every OSK.
pub fn display_name(layout: &LinuxLayout) -> Result<&str> {
    let primary = layout.tag.primary_language();
    layout
        .display_names
        .iter()
        .find(|(tag, _)| tag.eq_ignore_ascii_case(primary))
        .map(|(_, name)| name.as_str())
        .ok_or_else(|| anyhow!("{}: displayNames has no {primary} entry", layout.tag))
}
