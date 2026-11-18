use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};
use async_trait::async_trait;
use serde::Serialize;

use super::manifest::{DEFAULT_LOCALE, ICON, SERVICE_WORKER, locales, manifest};
use super::source::{self, ChromeOsLayout};
use crate::build::BuildStep;
use crate::build::output::{fresh_dir, write};
use crate::bundle::KbdgenBundle;

/// `crates/kbd-wasm` for `wasm32-unknown-unknown`, which the build script
/// compiles from this workspace, so its engine reads exactly the models
/// this kbdgen encodes.
// [spec:kbdgen:req:chromeos.engine]
pub const ENGINE_WASM: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/kbd_wasm.wasm"));

// [spec:kbdgen:sem:ldml.chromeos.keys]
// [spec:kbdgen:sem:ldml.chromeos.modifiers]
// [spec:kbdgen:sem:ldml.chromeos.semantics]
// [spec:kbdgen:sem:ldml.chromeos.context]
pub const SERVICE_WORKER_JS: &str = include_str!("extension/background.js");

pub const ENGINE_FILE: &str = "engine.wasm";
pub const MODELS_DIR: &str = "models";
const LOCALES_DIR: &str = "_locales";

fn write_json(path: &Path, value: &impl Serialize) -> Result<()> {
    let mut json = serde_json::to_string_pretty(value)
        .with_context(|| format!("serialise {}", path.display()))?;
    json.push('\n');
    write(path, json.as_bytes())
}

/// The extension icon: the default icon among the bundle's ChromeOS
/// resources. Chrome takes no SVG or ICNS for an extension icon, and scales
/// one PNG to every size it needs.
// [spec:kbdgen:req:chromeos.icons+1]
fn icon(bundle: &KbdgenBundle) -> Result<Option<&Path>> {
    let Some(icon) = bundle
        .resources
        .chromeos
        .as_ref()
        .and_then(|resources| resources.icon.as_deref())
    else {
        return Ok(None);
    };
    if icon.extension().is_none_or(|extension| extension != "png") {
        bail!(
            "{}: a ChromeOS extension icon must be a PNG",
            icon.display()
        );
    }
    Ok(Some(icon))
}

pub struct GenerateChromeOs;

#[async_trait(?Send)]
impl BuildStep for GenerateChromeOs {
    // [spec:kbdgen:def:chromeos.extension]
    // [spec:kbdgen:req:ldml.chromeos.target]
    async fn build(&self, bundle: &KbdgenBundle, output_path: &Path) -> Result<()> {
        let target = bundle
            .targets
            .chromeos
            .as_ref()
            .context("targets/chromeos.yaml is missing")?;
        let default_metadata = bundle
            .project
            .locales
            .get(DEFAULT_LOCALE)
            .with_context(|| format!("project.yaml: locales has no {DEFAULT_LOCALE} entry"))?;

        let layouts: Vec<ChromeOsLayout> = source::layouts(bundle)?;
        if layouts.is_empty() {
            bail!("no layout has a chromeOS keyboard");
        }
        let icon = icon(bundle)?;
        let manifest = manifest(&target.build, &target.version, &layouts, icon.is_some())?;
        let locales = locales(&bundle.project, default_metadata, &layouts);

        write(
            &output_path.join(SERVICE_WORKER),
            SERVICE_WORKER_JS.as_bytes(),
        )?;
        write(&output_path.join(ENGINE_FILE), ENGINE_WASM)?;
        let models = output_path.join(MODELS_DIR);
        fresh_dir(&models)?;
        for layout in &layouts {
            write(&models.join(format!("{}.dvkb", layout.tag)), &layout.model)?;
        }
        write_json(&output_path.join("manifest.json"), &manifest)?;
        let locales_dir = output_path.join(LOCALES_DIR);
        fresh_dir(&locales_dir)?;
        for (name, messages) in &locales {
            write_json(&locales_dir.join(name).join("messages.json"), messages)?;
        }
        let icon_path = output_path.join(ICON);
        match icon {
            Some(icon) => {
                fs::copy(icon, &icon_path).with_context(|| {
                    format!("copy {} to {}", icon.display(), icon_path.display())
                })?;
            }
            None => {
                tracing::info!("The bundle has no ChromeOS icon.png, so the extension has no icon");
                if icon_path.exists() {
                    fs::remove_file(&icon_path)
                        .with_context(|| format!("remove stale {}", icon_path.display()))?;
                }
            }
        }

        tracing::info!(
            "Wrote the ChromeOS extension with {} layout(s) to {}",
            layouts.len(),
            output_path.display()
        );
        Ok(())
    }
}
