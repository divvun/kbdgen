use std::collections::BTreeSet;
use std::path::Path;

use anyhow::{Context, Result, bail};
use async_trait::async_trait;

use super::ibus;
use super::ids::Ids;
use super::osk::{self, Osk, gnome, plasma, stevia};
use super::source::{self, LinuxLayout};
use super::xkb;
use crate::build::BuildStep;
use crate::build::diag::{Diagnostics, Platform};
use crate::bundle::KbdgenBundle;
use crate::bundle::project::LocaleProjectDescription;

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
        let en = bundle
            .project
            .locales
            .get("en")
            .context("project.yaml: locales has no en entry")?;
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
        let osk = on_screen_keyboards(&ids, &target.version, en, &layouts, &xkb)?;
        tracing::debug!(
            "Built the GNOME Shell extension {} with {} layout(s), {} plasma-keyboard package(s) and {} Stevia layout(s)",
            osk.gnome.uuid,
            osk.gnome.layouts.len(),
            osk.plasma.len(),
            osk.stevia.len()
        );
        tracing::info!("Prepared {} layout(s) for Linux", layouts.len());
        Ok(())
    }
}

/// The bundle's on-screen keyboards in memory (`linux.osk.*`).
struct OnScreenKeyboards {
    gnome: gnome::Extension,
    plasma: Vec<plasma::Package>,
    stevia: Vec<stevia::Layout>,
}

/// Each layout's on-screen keyboards in memory, reported as they are
/// built, so that a layout they cannot present fails the step before
/// anything is written. `xkb` is the layouts' XKB tier, whose dead
/// keysyms GNOME Shell's layouts send.
fn on_screen_keyboards(
    ids: &Ids,
    version: &str,
    en: &LocaleProjectDescription,
    layouts: &[LinuxLayout],
    xkb: &xkb::Tree,
) -> Result<OnScreenKeyboards> {
    let mut gnome_layouts = Vec::new();
    let mut plasma = Vec::new();
    let mut stevia = Vec::new();
    for (layout, adapted) in layouts.iter().zip(&xkb.layouts) {
        let presented = osk::present(layout)?;
        let classification = osk::classify(&layout.keyboard, &presented, &layout.tag);
        let mut diag = Diagnostics::new(Platform::Linux, layout.tag.to_string());
        osk::inform(&classification, &mut diag);
        let keyvals = gnome::keyvals(adapted);
        let sent: BTreeSet<_> = keyvals.keys().copied().collect();
        osk::report(&classification, Osk::GnomeShell(&sent), &mut diag);
        osk::report(&classification, Osk::PlasmaKeyboard, &mut diag);
        osk::report(&classification, Osk::Stevia, &mut diag);
        let id = ids.layout_id(&layout.tag);
        gnome_layouts.push(gnome::layout(&id, layout, &presented, &keyvals)?);
        plasma.push(plasma::package(ids, version, layout, &presented)?);
        stevia.push(stevia::layout(&id, layout, &presented)?);
    }
    Ok(OnScreenKeyboards {
        gnome: gnome::extension(ids, version, en, gnome_layouts)?,
        plasma,
        stevia,
    })
}
