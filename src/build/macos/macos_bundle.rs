use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, anyhow};
use indexmap::IndexMap;
use language_tags::LanguageTag;
use serde::Serialize;

use super::input::KeylayoutInput;
use super::util::run;
use crate::bundle::KbdgenBundle;
use crate::bundle::target::MacOS;

const TOP_FOLDER: &str = "Contents";
const RESOURCES_FOLDER: &str = "Resources";
const KEY_LAYOUT_EXT: &str = "keylayout";
const LPROJ_EXT: &str = "lproj";

pub fn macos_target(bundle: &KbdgenBundle) -> Result<&MacOS> {
    bundle
        .targets
        .macos
        .as_ref()
        .ok_or_else(|| anyhow!("{}: the bundle has no targets/macos.yaml", bundle.name()))
}

/// `{packageId}.keyboardlayout.{stem}`, which also names the `.bundle`
/// directory and the installer package.
// [spec:kbdgen:req:macbundle.plist+2]
pub fn bundle_id(target: &MacOS, bundle: &KbdgenBundle) -> String {
    format!("{}.keyboardlayout.{}", target.package_id, bundle.name())
}

#[derive(Serialize)]
// [spec:kbdgen:req:macbundle.plist+2]
struct InfoPlist {
    #[serde(rename = "CFBundleIdentifier")]
    cf_bundle_identifier: String,
    #[serde(rename = "CFBundleName")]
    cf_bundle_name: String,
    #[serde(rename = "CFBundleVersion")]
    cf_bundle_version: String,
    #[serde(rename = "CFBundleShortVersionString")]
    cf_bundle_short_version_string: String,
    /// Serialised after the CFBundle keys in insertion order, which is bundle
    /// layout order because keylayouts are added in that order.
    #[serde(flatten)]
    kl_info_map: IndexMap<String, KlInfo>,
}

#[derive(Serialize)]
struct KlInfo {
    #[serde(rename = "TISInputSourceID")]
    tis_input_source_id: String,

    #[serde(rename = "TISIntendedLanguage")]
    tis_intended_language: String,
}

pub struct MacOsBundle {
    path: PathBuf,
    info_plist: InfoPlist,
    translation_strings: IndexMap<LanguageTag, IndexMap<String, String>>,
    /// Each layout's keyboard name and `.keylayout` text, by language tag.
    layouts: IndexMap<LanguageTag, (String, String)>,
    icons: IndexMap<LanguageTag, PathBuf>,
}

impl MacOsBundle {
    // [spec:kbdgen:req:macbundle.plist+2]
    pub fn new(output_path: &Path, bundle: &KbdgenBundle) -> Result<MacOsBundle> {
        let target = macos_target(bundle)?;
        let cf_bundle_identifier = bundle_id(target, bundle);
        let path = output_path.join(format!("{cf_bundle_identifier}.bundle"));
        let resources = path.join(TOP_FOLDER).join(RESOURCES_FOLDER);
        std::fs::create_dir_all(&resources)
            .with_context(|| format!("create {}", resources.display()))?;

        Ok(Self {
            path,
            info_plist: InfoPlist {
                cf_bundle_identifier,
                cf_bundle_name: target.bundle_name.clone(),
                cf_bundle_version: target.build.clone(),
                cf_bundle_short_version_string: target.version.clone(),
                kl_info_map: IndexMap::new(),
            },
            translation_strings: IndexMap::new(),
            layouts: IndexMap::new(),
            icons: bundle
                .resources
                .macos
                .as_ref()
                .map(|resources| resources.icons.clone())
                .unwrap_or_default(),
        })
    }

    fn resources(&self) -> PathBuf {
        self.path.join(TOP_FOLDER).join(RESOURCES_FOLDER)
    }

    // [spec:kbdgen:req:macbundle.plist+2]
    // [spec:kbdgen:req:macbundle.plist.bundle-resources+1]
    // [spec:kbdgen:req:macbundle.plist.strings]
    pub fn add_key_layout(&mut self, input: &KeylayoutInput, layout_xml: String) {
        let name = &input.name;
        self.info_plist.kl_info_map.insert(
            format!("KLInfo_{name}"),
            KlInfo {
                tis_input_source_id: format!("{}.{name}", self.info_plist.cf_bundle_identifier),
                tis_intended_language: input.tag.to_string(),
            },
        );
        for (lang, display_name) in &input.display_names {
            self.translation_strings
                .entry(lang.clone())
                .or_default()
                .insert(name.clone(), display_name.clone());
        }
        self.layouts
            .insert(input.tag.clone(), (name.clone(), layout_xml));
    }

    /// Converts the layout's `icon.<tag>.*`, if it has one, into
    /// `<name>.icns`.
    // [spec:kbdgen:req:macbundle.plist.bundle-resources+1]
    fn write_icon(&self, tag: &LanguageTag, name: &str) -> Result<()> {
        const SIZES: &[(&str, u32)] = &[
            ("icon_16x16", 16),
            ("icon_16x16@2x", 32),
            ("icon_32x32", 32),
            ("icon_32x32@2x", 64),
        ];

        let Some(icon) = self.icons.get(tag) else {
            return Ok(());
        };
        if !cfg!(target_os = "macos") {
            tracing::warn!(
                "{name}: skipping icon {}: iconutil only exists on macOS",
                icon.display()
            );
            return Ok(());
        }
        let icns = self.resources().join(format!("{name}.icns"));
        tracing::info!("Converting {} into {}", icon.display(), icns.display());

        let tmpdir = tempfile::tempdir().context("create a directory for the iconset")?;
        let iconset = tmpdir.path().join("tmp.iconset");
        std::fs::create_dir(&iconset).with_context(|| format!("create {}", iconset.display()))?;
        for (file, size) in SIZES {
            let size = format!("{size}x{size}");
            run(Command::new("convert")
                .args(["-resize", &size])
                .args(["-background", "transparent", "-gravity", "center"])
                .args(["-extent", &size])
                .arg(icon)
                .arg(iconset.join(format!("{file}.png"))))
            .with_context(|| format!("{name}: resize {} to {file}.png", icon.display()))?;
        }
        run(Command::new("iconutil")
            .args(["--convert", "icns", "--output"])
            .arg(&icns)
            .arg(&iconset))
        .with_context(|| format!("{name}: write {}", icns.display()))
    }

    // [spec:kbdgen:req:macbundle.plist+2]
    // [spec:kbdgen:req:macbundle.plist.bundle-resources+1]
    // [spec:kbdgen:req:macbundle.plist.strings]
    pub fn write_all(self) -> Result<()> {
        let plist_path = self.path.join(TOP_FOLDER).join("Info.plist");
        tracing::debug!("Writing {}", plist_path.display());
        plist::to_file_xml(&plist_path, &self.info_plist)
            .with_context(|| format!("write {}", plist_path.display()))?;

        let resources = self.resources();
        for (tag, (name, layout_xml)) in &self.layouts {
            let path = resources.join(format!("{name}.{KEY_LAYOUT_EXT}"));
            tracing::debug!("Writing {}", path.display());
            std::fs::write(&path, layout_xml)
                .with_context(|| format!("write {}", path.display()))?;
            self.write_icon(tag, name)?;
        }

        for (lang, names) in &self.translation_strings {
            let lproj_path = resources.join(format!("{lang}.{LPROJ_EXT}"));
            std::fs::create_dir_all(&lproj_path)
                .with_context(|| format!("create {}", lproj_path.display()))?;
            let output = names
                .iter()
                .map(|(k, v)| format!("{k:?} = {v:?};"))
                .collect::<Vec<_>>()
                .join("\n");
            let path = lproj_path.join("InfoPlist.strings");
            tracing::debug!("Writing {}", path.display());
            std::fs::write(&path, output).with_context(|| format!("write {}", path.display()))?;
        }

        tracing::info!(
            "Wrote {} keyboard layout(s) to {}",
            self.layouts.len(),
            self.path.display()
        );
        Ok(())
    }
}
