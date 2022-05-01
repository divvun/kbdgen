use std::{collections::HashMap, path::PathBuf};

use indexmap::IndexMap;
use language_tags::LanguageTag;
use serde::{Deserialize, Serialize};
use xmlem::Document;

const TOP_FOLDER: &str = "Contents";
const RESOURCES_FOLDER: &str = "Resources";
const KEY_LAYOUT_EXT: &str = "keylayout";
const LPROJ_EXT: &str = "lproj";

#[derive(Serialize, Deserialize)]
pub struct InfoPlist {
    #[serde(rename = "CFBundleIdentifier")]
    pub cf_bundle_identifier: String,
    #[serde(rename = "CFBundleName")]
    pub cf_bundle_name: String,
    #[serde(rename = "CFBundleVersion")]
    pub cf_bundle_version: String,
    #[serde(rename = "CFBundleShortVersionString")]
    pub cf_bundle_short_version_string: String,
    #[serde(flatten)]
    pub kl_info_map: HashMap<String, KlInfo>,
}

#[derive(Serialize, Deserialize)]
pub struct KlInfo {
    #[serde(rename = "TISInputSourceID")]
    tis_input_source_id: String,

    #[serde(rename = "TISIntendedLanguage")]
    tis_intended_language: String,
}

pub struct Bundle {
    path: PathBuf,
    info_plist: InfoPlist,
    translation_strings: IndexMap<LanguageTag, IndexMap<String, String>>,
    layouts: IndexMap<LanguageTag, (String, String)>,
    icons: IndexMap<LanguageTag, PathBuf>,
}

impl Bundle {
    pub fn new(
        path: PathBuf,
        name: &str,
        kbdgen_bundle: &crate::bundle::KbdgenBundle,
    ) -> Result<Bundle, std::io::Error> {
        let target = kbdgen_bundle.targets.macos.as_ref().unwrap();
        let icons = kbdgen_bundle
            .resources
            .macos
            .as_ref()
            .unwrap()
            .icons
            .clone();

        let cf_bundle_identifier = format!("{}.keyboardlayout.{}", target.package_id, name);

        let path = path.join(format!("{cf_bundle_identifier}.bundle"));
        let contents_path = path.join(TOP_FOLDER);
        let resources_path = contents_path.join(RESOURCES_FOLDER);

        std::fs::create_dir_all(&contents_path)?;
        std::fs::create_dir_all(&resources_path)?;

        Ok(Self {
            path,
            layouts: Default::default(),
            icons,
            info_plist: InfoPlist {
                cf_bundle_identifier,
                cf_bundle_name: target.bundle_name.to_string(),
                cf_bundle_version: target.build.to_string(),
                cf_bundle_short_version_string: target.version.to_string(),
                kl_info_map: Default::default(),
            },
            translation_strings: Default::default(),
        })
    }

    pub fn add_key_layout(
        &mut self,
        language_tag: LanguageTag,
        layout_doc: Document,
        layout_names: &IndexMap<LanguageTag, String>,
    ) {
        let name = layout_doc
            .root()
            .attribute(&layout_doc, "name")
            .expect("name attr must exist");

        let tis_input_source_id = format!("{}.{}", self.info_plist.cf_bundle_identifier, name);
        self.info_plist.kl_info_map.insert(
            format!("KLInfo_{name}"),
            KlInfo {
                tis_input_source_id,
                tis_intended_language: language_tag.to_string(),
            },
        );

        for (lang, value) in layout_names.iter() {
            self.translation_strings
                .entry(lang.clone())
                .and_modify(|e| {
                    e.insert(name.to_string(), value.to_string());
                })
                .or_insert_with(|| {
                    let mut map = IndexMap::new();
                    map.insert(name.to_string(), value.to_string());
                    map
                });
        }

        self.layouts.insert(
            language_tag,
            (name.to_string(), layout_doc.to_string_pretty()),
        );
    }

    pub fn write_icons(&self, language_tag: LanguageTag, name: &str) -> Result<(), std::io::Error> {
        const FILES: &[(&str, i32)] = &[
            ("icon_16x16", 16),
            ("icon_16x16@2x", 32),
            ("icon_32x32", 32),
            ("icon_32x32@2x", 64),
        ];

        let icon = match self.icons.get(&language_tag) {
            Some(v) => v,
            None => return Ok(()),
        };

        let tmpdir = tempfile::tempdir()?;
        let iconset_path = tmpdir.path().join("tmp.iconset");
        std::fs::create_dir(&iconset_path)?;

        for (icon_name, d) in FILES {
            std::process::Command::new("convert")
                .arg("-resize")
                .arg(format!("{d}x{d}"))
                .args(&[
                    "-background",
                    "transparent",
                    "-gravity",
                    "center",
                    "-extent",
                ])
                .arg(format!("{d}x{d}"))
                .arg(icon)
                .arg(iconset_path.join(format!("{icon_name}.png")))
                .output()
                .expect("convert failed to run");
        }

        let resources = self.path.join(TOP_FOLDER).join(RESOURCES_FOLDER);
        std::process::Command::new("iconutil")
            .args(&["--convert", "icns", "--output"])
            .arg(resources.join(format!("{name}.icns")))
            .arg(iconset_path)
            .output()
            .expect("iconutil failed to run");

        Ok(())
    }

    pub fn write_all(self) -> Result<(), std::io::Error> {
        plist::to_file_xml(
            self.path.join(TOP_FOLDER).join("Info.plist"),
            &self.info_plist,
        )
        .expect("Could not serialize Info.plist");

        let resources = self.path.join(TOP_FOLDER).join(RESOURCES_FOLDER);
        for (tag, (name, layout_xml)) in self.layouts.iter() {
            let key_layout_path = resources.join(format!("{name}.{KEY_LAYOUT_EXT}"));
            tracing::debug!("Writing {name} to {key_layout_path:?}...");
            std::fs::write(key_layout_path, layout_xml)?;

            tracing::debug!("Writing icons for {name}...");
            self.write_icons(tag.clone(), &name)?;
        }

        for (lang, text) in self.translation_strings {
            let lproj_path = resources.join(format!("{}.{LPROJ_EXT}", lang.to_string()));
            std::fs::create_dir_all(&lproj_path)?;

            let output = text
                .iter()
                .map(|(k, v)| format!("{k:?} = {v:?};"))
                .collect::<Vec<_>>()
                .join("\n");
            std::fs::write(lproj_path.join("InfoPlist.strings"), output)?;
        }

        Ok(())
    }
}
