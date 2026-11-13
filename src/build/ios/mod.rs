use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use async_trait::async_trait;
use language_tags::LanguageTag;
use once_cell::sync::Lazy;

use crate::bundle::{KbdgenBundle, layout::Layout};

use self::{
    clone_giellakbd::CloneGiellaKbd,
    generate_ios::GenerateIos,
    generate_xcode::GenerateXcode,
    pod_install::PodInstall,
    xcodebuild::{BuildXcarchive, FastlaneProvisioning, fastlane_env},
};

use super::{BuildStep, BuildSteps};

pub mod clone_giellakbd;
pub mod generate_ios;
pub mod generate_xcode;
pub mod pbxproj;
pub mod pod_install;
pub mod serialize_pbxproj;
pub mod xcode_structures;
mod xcodebuild;

const REPOSITORY_FOLDER: &str = "repo";
const DEFAULT_LOCALE: &str = "en";

/// The bundle's iOS layouts, in the order they are generated.
///
/// `KeyboardDefinitions.json` and each keyboard's `DivvunKeyboardIndex` are both built from this list,
/// and the keyboard extension uses its index to find its entry in the JSON. They must therefore contain
/// the same layouts in the same order, so both generators must use this function.
///
/// Layouts are sorted by their English display name (the name shown in iOS Settings), then by language
/// tag. Fails if any iOS layout lacks the fields iOS needs, rather than leaving it out of one of the files.
pub(crate) fn ios_layouts(bundle: &KbdgenBundle) -> anyhow::Result<Vec<(&LanguageTag, &Layout)>> {
    let default_tag: LanguageTag = DEFAULT_LOCALE
        .parse()
        .expect("default language tag must parse");

    let mut layouts: Vec<(&LanguageTag, &Layout)> = bundle
        .layouts
        .iter()
        .filter(|(_, layout)| layout.i_os.is_some())
        .collect();

    let mut problems = Vec::new();
    for (tag, layout) in &layouts {
        if layout.key_names.is_none() {
            problems.push(format!(
                "{tag}: missing `keyNames` (space and return key labels)"
            ));
        }
        if !layout.display_names.contains_key(&default_tag) {
            problems.push(format!("{tag}: missing `displayNames.{DEFAULT_LOCALE}`"));
        }
    }
    if !problems.is_empty() {
        problems.sort();
        anyhow::bail!(
            "Some iOS layouts are incomplete:\n  {}",
            problems.join("\n  ")
        );
    }

    layouts.sort_by_cached_key(|(tag, layout)| {
        (
            layout.display_names[&default_tag].to_lowercase(),
            tag.to_string(),
        )
    });

    Ok(layouts)
}

pub struct IosBuild {
    pub bundle: KbdgenBundle,
    pub output_path: PathBuf,
    pub steps: Vec<Box<dyn BuildStep>>,
}

#[async_trait(?Send)]
impl BuildSteps for IosBuild {
    fn new(bundle: KbdgenBundle, output_path: PathBuf) -> Self {
        let steps: Vec<Box<dyn BuildStep>> = vec![
            Box::new(CloneGiellaKbd),
            Box::new(GenerateIos),
            Box::new(GenerateXcode),
            Box::new(FastlaneProvisioning),
            Box::new(PodInstall),
            Box::new(BuildXcarchive),
        ];

        IosBuild {
            bundle,
            output_path,
            steps,
        }
    }

    fn steps(&self) -> &[Box<dyn BuildStep>] {
        &self.steps
    }

    fn bundle(&self) -> &KbdgenBundle {
        &self.bundle
    }

    fn output_path(&self) -> &Path {
        &self.output_path
    }
}

pub trait IosProjectExt {
    fn pkg_id(&self, layout: &Layout) -> String;
    fn all_pkg_ids(&self) -> Vec<String>;
    fn supported_layouts(&self) -> HashMap<&LanguageTag, &Layout>;
}

static LEGACY_DIVVUN_KBD_IDS: Lazy<HashMap<&str, &str>> = Lazy::new(|| {
    [
        ("se", "northern-sami-keyboard"),
        ("sme", "northern-sami-keyboard"),
        ("sms", "skolt-sami-keyboard"),
        ("smn", "inari-sami-keyboard"),
        ("smj-SE", "julev-sami-keyboard"),
        ("smj-NO", "julev-sami-keyboard-no"),
        ("sma", "south-sami-keyboard"),
    ]
    .into()
});

impl IosProjectExt for KbdgenBundle {
    fn pkg_id(&self, layout: &Layout) -> String {
        let target = self.targets.ios.as_ref().unwrap();
        let base_id = &target.package_id;
        let tag = layout.language_tag.as_str();

        let ext = if target.package_id == "no.uit.giella.keyboards.Sami" {
            LEGACY_DIVVUN_KBD_IDS.get(tag).unwrap_or(&tag)
        } else {
            tag
        };

        format!("{base_id}.{ext}")
    }

    fn all_pkg_ids(&self) -> Vec<String> {
        let target = self.targets.ios.as_ref().unwrap();
        let base_id = &target.package_id;
        let mut v = std::iter::once(base_id.to_string())
            .chain(
                self.supported_layouts()
                    .values()
                    .map(|layout| self.pkg_id(layout)),
            )
            .collect::<Vec<_>>();
        v.sort();
        v
    }

    fn supported_layouts(&self) -> HashMap<&LanguageTag, &Layout> {
        self.layouts
            .iter()
            .filter(|(_, layout)| layout.i_os.is_some())
            .collect()
    }
}

pub async fn init(bundle: KbdgenBundle, path: &Path) -> anyhow::Result<()> {
    let target = bundle.targets.ios.as_ref().cloned().unwrap();
    let app_name = bundle.project.locales["en"].name.to_string();
    let base_id = target.package_id.to_string();

    let env = fastlane_env(&target);

    tracing::debug!(id = base_id, "registering id");
    tokio::process::Command::new("fastlane")
        .current_dir(path)
        .envs(&env)
        .args(["produce", "-a", &base_id, "--app_name", &app_name])
        .output()
        .await?;

    tracing::debug!(id = base_id, "registering base group");
    tokio::process::Command::new("fastlane")
        .current_dir(path)
        .envs(&env)
        .args(["produce", "group", "-g"])
        .arg(format!("group.{base_id}"))
        .arg("-n")
        .arg(format!("{} Group", &app_name))
        .output()
        .await?;

    let ids = bundle.all_pkg_ids();
    let futs = ids
        .into_iter()
        .map(|id| {
            let path = path.to_path_buf();
            let base_id = base_id.to_string();
            let target = target.clone();
            let app_name = app_name.clone();

            tokio::spawn(async move {
                if id != base_id {
                    tracing::debug!(id = id, "registering id");
                    let _output = tokio::process::Command::new("fastlane")
                        .current_dir(&path)
                        .envs(fastlane_env(&target))
                        .args(["produce", "-a", &id, "--app_name"])
                        .arg(format!("{app_name}: {}", id.rsplit(".").next().unwrap()))
                        .arg("--skip_itc")
                        .output()
                        .await?;
                }

                tracing::debug!(id = id, "enabling app group");
                let _output = tokio::process::Command::new("fastlane")
                    .current_dir(&path)
                    .envs(fastlane_env(&target))
                    .args(["produce", "enable_services", "-a", &id, "--app-group"])
                    .output()
                    .await?;

                tracing::debug!(id = id, "associating group");
                let _output = tokio::process::Command::new("fastlane")
                    .current_dir(&path)
                    .envs(fastlane_env(&target))
                    .args(["produce", "associate_group", "-a", &id])
                    .arg(format!("group.{base_id}"))
                    .output()
                    .await?;

                Ok::<_, std::io::Error>(())
            })
        })
        .collect::<Vec<_>>();

    for fut in futs.into_iter() {
        let _mm = fut.await?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bundle::layout::{IOsTarget, KeyNames};
    use indexmap::IndexMap;

    fn ios_layout(tag: &str, english_name: &str, with_key_names: bool) -> (LanguageTag, Layout) {
        let tag: LanguageTag = tag.parse().unwrap();
        let primary: LanguageTag = tag.primary_language().parse().unwrap();
        let mut display_names = IndexMap::new();
        display_names.insert(DEFAULT_LOCALE.parse().unwrap(), english_name.to_string());
        display_names.insert(primary, format!("{english_name} autonym"));

        let layout = Layout {
            language_tag: tag.clone(),
            display_names,
            decimal: None,
            windows: None,
            chrome_os: None,
            mac_os: None,
            i_os: Some(IOsTarget {
                config: Default::default(),
                primary: None,
                i_pad_9in: None,
                i_pad_12in: None,
                dead_keys: None,
            }),
            android: None,
            longpress: None,
            transforms: None,
            key_names: with_key_names.then(|| KeyNames {
                space: "space".to_string(),
                r#return: "return".to_string(),
            }),
        };
        (tag, layout)
    }

    fn bundle(layouts: Vec<(LanguageTag, Layout)>) -> KbdgenBundle {
        KbdgenBundle::new_test("test".to_string(), layouts.into_iter().collect())
    }

    fn tags(layouts: &[(&LanguageTag, &Layout)]) -> Vec<String> {
        layouts.iter().map(|(tag, _)| tag.to_string()).collect()
    }

    #[test]
    fn sorts_by_english_display_name_ignoring_case() {
        let bundle = bundle(vec![
            ios_layout("se", "Northern Sami", true),
            ios_layout("en", "English", true),
            ios_layout("crk", "Plains Cree", true),
            ios_layout("nn", "norwegian Nynorsk", true),
            ios_layout("fo", "Faroese", true),
        ]);

        let layouts = ios_layouts(&bundle).unwrap();

        assert_eq!(tags(&layouts), ["en", "fo", "se", "nn", "crk"]);
    }

    #[test]
    fn breaks_display_name_ties_by_language_tag() {
        let bundle = bundle(vec![
            ios_layout("smj-SE", "Lule Sami", true),
            ios_layout("smj-NO", "Lule Sami", true),
        ]);

        let layouts = ios_layouts(&bundle).unwrap();

        assert_eq!(tags(&layouts), ["smj-NO", "smj-SE"]);
    }

    #[test]
    fn skips_layouts_without_ios() {
        let mut no_ios = ios_layout("fi", "Finnish", true);
        no_ios.1.i_os = None;
        let bundle = bundle(vec![no_ios, ios_layout("se", "Northern Sami", true)]);

        let layouts = ios_layouts(&bundle).unwrap();

        assert_eq!(tags(&layouts), ["se"]);
    }

    #[test]
    fn fails_listing_every_layout_missing_key_names() {
        let mut no_english_name = ios_layout("sma", "Southern Sami", true);
        no_english_name
            .1
            .display_names
            .shift_remove(&DEFAULT_LOCALE.parse::<LanguageTag>().unwrap());
        let bundle = bundle(vec![
            ios_layout("se", "Northern Sami", true),
            ios_layout("en", "English", false),
            ios_layout("waw", "Wai Wai", false),
            no_english_name,
        ]);

        let error = ios_layouts(&bundle).unwrap_err().to_string();

        assert_eq!(
            error,
            "Some iOS layouts are incomplete:\n  \
             en: missing `keyNames` (space and return key labels)\n  \
             sma: missing `displayNames.en`\n  \
             waw: missing `keyNames` (space and return key labels)"
        );
    }

    #[tokio::test]
    async fn keyboard_definitions_json_follows_ios_layouts_order() {
        let bundle = bundle(vec![
            ios_layout("waw", "Wai Wai", true),
            ios_layout("en", "English", true),
            ios_layout("crk", "Plains Cree", true),
        ]);
        let output = tempfile::tempdir().unwrap();
        let models = output
            .path()
            .join(REPOSITORY_FOLDER)
            .join("Keyboard/Models");
        std::fs::create_dir_all(&models).unwrap();

        GenerateIos.build(&bundle, output.path()).await.unwrap();

        let json: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(models.join("KeyboardDefinitions.json")).unwrap(),
        )
        .unwrap();
        let json_locales: Vec<&str> = json
            .as_array()
            .unwrap()
            .iter()
            .map(|definition| definition["locale"].as_str().unwrap())
            .collect();
        let expected = tags(&ios_layouts(&bundle).unwrap());
        assert_eq!(json_locales, expected);
        assert_eq!(json_locales, ["en", "crk", "waw"]);
    }

    #[tokio::test]
    async fn keyboard_definitions_json_is_not_written_when_key_names_are_missing() {
        let bundle = bundle(vec![
            ios_layout("se", "Northern Sami", true),
            ios_layout("en", "English", false),
        ]);
        let output = tempfile::tempdir().unwrap();
        let models = output
            .path()
            .join(REPOSITORY_FOLDER)
            .join("Keyboard/Models");
        std::fs::create_dir_all(&models).unwrap();

        let result = GenerateIos.build(&bundle, output.path()).await;

        assert!(result.is_err());
        assert!(!models.join("KeyboardDefinitions.json").exists());
    }
}
