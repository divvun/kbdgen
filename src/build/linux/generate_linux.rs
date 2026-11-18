use std::path::Path;

use anyhow::{Context, Result, bail};
use async_trait::async_trait;

use super::ids::Ids;
use super::source;
use crate::build::BuildStep;
use crate::build::diag::{Diagnostics, Platform};
use crate::bundle::KbdgenBundle;

/// The step of `target linux`, which names and compiles every layout with
/// Linux output.
pub struct GenerateLinux;

#[async_trait(?Send)]
impl BuildStep for GenerateLinux {
    async fn build(&self, bundle: &KbdgenBundle, _output_path: &Path) -> Result<()> {
        let target = bundle
            .targets
            .linux
            .as_ref()
            .context("targets/linux.yaml is missing")?;
        let ids = Ids::new(target)?;
        let layouts = source::layouts(bundle)?;
        if layouts.is_empty() {
            bail!("no layout has a linux keyboard");
        }
        for layout in &layouts {
            Diagnostics::new(Platform::Linux, layout.tag.to_string()).inform(format!(
                "compiled as the Linux layout {}",
                ids.layout_id(&layout.tag)
            ));
        }
        tracing::info!("Prepared {} layout(s) for Linux", layouts.len());
        Ok(())
    }
}
