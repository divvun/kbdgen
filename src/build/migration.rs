//! v3 layouts migrated in memory (`ldml.migrate.*`), with nothing written,
//! for the targets that build v3 layouts from the model: ChromeOS
//! (`ldml.chromeos.input`) and Linux (`ldml.linux.input`).

use anyhow::{Context, Result, anyhow, bail};
use language_tags::LanguageTag;

use crate::bundle::KbdgenBundle;
use crate::ldml::layouts::HostDocument;
use crate::ldml::migrate::defect::Code;
use crate::ldml::migrate::{Migration, migrate_bundle_layout};

/// The v3 layout `tag` of `bundle`, migrated in memory. A migration that
/// blocks is an error listing its blocking defect codes.
// [spec:kbdgen:def:ldml.chromeos.input]
// [spec:kbdgen:def:ldml.linux.input+1]
pub fn unblocked(bundle: &KbdgenBundle, tag: &LanguageTag) -> Result<Migration> {
    let migration = migrate_bundle_layout(&bundle.path, tag.as_str())
        .with_context(|| format!("{tag}: migrate the v3 layout"))?
        .ok_or_else(|| anyhow!("{tag}: the bundle has no v3 layout file to migrate"))?;
    if migration.blocked() {
        let codes: Vec<&str> = migration
            .blocking_codes()
            .into_iter()
            .map(Code::name)
            .collect();
        bail!(
            "{tag}: the v3 layout's migration is blocked by defects {}; `kbdgen ldml migrate --dry-run` reports them",
            codes.join(", ")
        );
    }
    Ok(migration)
}

/// The host documents (`ldml.yaml.hosts`) of `tag`'s migration, as
/// [`unblocked`] returned it: a migration that blocks has none, which would
/// read here as migrating to no layout.
pub fn host_documents(tag: &LanguageTag, migration: &Migration) -> Result<Vec<HostDocument>> {
    migration
        .host_documents()
        .with_context(|| format!("{tag}: lower the migrated layout"))?
        .ok_or_else(|| anyhow!("{tag}: the v3 layout migrated to no layout"))
}
