//! The names of a bundle's Linux output (`linux.ids`).

use anyhow::{Result, bail};
use language_tags::LanguageTag;

use crate::bundle::target::Linux;

/// The `packageId` and `layoutPrefix` of `targets/linux.yaml`, checked, and
/// the names derived from them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ids {
    package_id: String,
    layout_prefix: String,
}

/// Whether `id` is a reverse-DNS name: two or more `.`-separated labels,
/// none empty, of lowercase ASCII letters, digits and `-`.
fn is_reverse_dns(id: &str) -> bool {
    id.contains('.')
        && id.split('.').all(|label| {
            !label.is_empty()
                && label
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        })
}

fn is_layout_prefix(prefix: &str) -> bool {
    let mut bytes = prefix.bytes();
    prefix.len() <= 16
        && bytes.next().is_some_and(|b| b.is_ascii_lowercase())
        && bytes.all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
}

impl Ids {
    // [spec:kbdgen:def:linux.ids]
    pub fn new(target: &Linux) -> Result<Ids> {
        let Linux {
            package_id,
            layout_prefix,
            ..
        } = target;
        if !is_reverse_dns(package_id) {
            bail!(
                "targets/linux.yaml: packageId {package_id:?} is not a reverse-DNS name: two or more dot-separated labels of lowercase ASCII letters, digits and '-'"
            );
        }
        if !is_layout_prefix(layout_prefix) {
            bail!(
                "targets/linux.yaml: layoutPrefix {layout_prefix:?} is not a lowercase ASCII letter followed by at most 15 lowercase ASCII letters and digits"
            );
        }
        Ok(Ids {
            package_id: package_id.clone(),
            layout_prefix: layout_prefix.clone(),
        })
    }

    /// The layout id L of the layout tagged `tag`, which names its XKB
    /// layout, its IBus engine and its GNOME Shell and Stevia layouts.
    // [spec:kbdgen:def:linux.ids]
    pub fn layout_id(&self, tag: &LanguageTag) -> String {
        format!("{}-{}", self.layout_prefix, tag.as_str().to_lowercase())
    }

    /// The UUID of the bundle's GNOME Shell extension.
    // [spec:kbdgen:def:linux.ids]
    pub fn extension_uuid(&self) -> String {
        format!("osk@{}", self.package_id)
    }

    /// The id of the plasma-keyboard package of the layout tagged `tag`.
    // [spec:kbdgen:def:linux.ids]
    pub fn kpackage_id(&self, tag: &LanguageTag) -> String {
        format!("{}.{}", self.package_id, tag.as_str().replace('-', "_"))
    }

    /// The name of both the bundle's XKB package directory and its IBus
    /// data directory (`linux.install`).
    // [spec:kbdgen:def:linux.ids]
    pub fn package_dir(&self) -> &str {
        &self.package_id
    }
}
