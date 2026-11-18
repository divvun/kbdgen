//! The XKB tier of the Linux target (`docs/spec/linux.md`, "XKB layout";
//! `docs/spec/ldml/linux.md`): each layout's registry entry and the
//! bundle's `rules/evdev.xml`, built in memory for the output step to
//! write.

mod registry;
#[cfg(test)]
mod tests;

use anyhow::Result;

pub use registry::{RegistryEntry, evdev_xml};

use super::ids::Ids;
use super::source::LinuxLayout;

/// The bundle's `xkb/` tree: file paths relative to it, and contents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tree {
    pub files: Vec<(String, String)>,
}

impl Tree {
    /// The tree of `layouts`, in bundle layout order.
    // [spec:kbdgen:syn:linux.xkb.registry]
    pub fn new(ids: &Ids, layouts: &[LinuxLayout]) -> Result<Tree> {
        let entries = layouts
            .iter()
            .map(|layout| RegistryEntry::new(ids, layout))
            .collect::<Result<Vec<_>>>()?;
        Ok(Tree {
            files: vec![("rules/evdev.xml".to_owned(), evdev_xml(&entries))],
        })
    }
}
