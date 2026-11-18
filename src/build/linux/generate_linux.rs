use std::path::Path;

use anyhow::{Context, Result, bail};
use async_trait::async_trait;

use super::ibus;
use super::ids::Ids;
use super::osk::{self, Osk, plasma};
use super::source::{self, LinuxLayout};
use super::xkb;
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
        let xkb = xkb::Tree::new(&ids, &layouts)?;
        tracing::debug!("The XKB tier has {} file(s)", xkb.files.len());
        let engine = ibus::Tree::new(&ids, &bundle.project.author, &target.version, &layouts)?;
        tracing::debug!("The IBus engine's data has {} file(s)", engine.files.len());
        let packages = on_screen_keyboards(&ids, &target.version, &layouts)?;
        tracing::debug!("Built {} plasma-keyboard package(s)", packages.len());
        tracing::info!("Prepared {} layout(s) for Linux", layouts.len());
        Ok(())
    }
}

/// Each layout's on-screen keyboards in memory, reported as they are
/// built (`linux.osk.*`), so that a layout they cannot present fails the
/// step before anything is written.
fn on_screen_keyboards(
    ids: &Ids,
    version: &str,
    layouts: &[LinuxLayout],
) -> Result<Vec<plasma::Package>> {
    layouts
        .iter()
        .map(|layout| {
            let presented = osk::present(layout)?;
            let classification = osk::classify(&layout.keyboard, &presented, &layout.tag);
            let mut diag = Diagnostics::new(Platform::Linux, layout.tag.to_string());
            osk::inform(&classification, &mut diag);
            osk::report(&classification, Osk::PlasmaKeyboard, &mut diag);
            plasma::package(ids, version, layout, &presented)
        })
        .collect()
}
