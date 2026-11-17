use std::{
    path::{Path, PathBuf},
    process::Output,
};

use anyhow::{Context, Result, bail};
use async_trait::async_trait;
use language_tags::LanguageTag;

use crate::bundle::{KbdgenBundle, layout::Layout, target::iOS};

use self::{
    clone_giellakbd::CloneGiellaKbd,
    generate_ios::GenerateIos,
    generate_xcode::GenerateXcode,
    pod_install::PodInstall,
    xcodebuild::{BuildXcarchive, FastlaneProvisioning, fastlane, fastlane_env},
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
// [spec:kbdgen:req:ios.layouts]
pub(crate) fn ios_layouts(bundle: &KbdgenBundle) -> Result<Vec<(&LanguageTag, &Layout)>> {
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
        bail!(
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
    // [spec:kbdgen:req:ios.build-chain+3]
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

pub(crate) fn ios_target(bundle: &KbdgenBundle) -> Result<&iOS> {
    bundle
        .targets
        .ios
        .as_ref()
        .context("the bundle has no iOS target (targets/ios.yaml)")
}

/// Fails unless the command behind `output` succeeded, quoting the end of its stderr, or of its
/// stdout when stderr is empty, as fastlane reports its errors there.
pub(crate) fn ensure_success(command: &str, output: &Output) -> Result<()> {
    const TAIL_LINES: usize = 20;

    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    let report = if stderr.trim().is_empty() {
        String::from_utf8_lossy(&output.stdout)
    } else {
        stderr
    };
    let lines: Vec<&str> = report.trim().lines().collect();
    let tail = lines[lines.len().saturating_sub(TAIL_LINES)..].join("\n");
    bail!("`{command}` failed with {}: {tail}", output.status)
}

/// The last segment of a package id, which names its keyboard's folder and Xcode target.
pub(crate) fn keyboard_name(pkg_id: &str) -> &str {
    pkg_id.rsplit_once('.').map_or(pkg_id, |(_, name)| name)
}

/// Divvun's Sami keyboards keep the package ids they were first published with.
// [spec:kbdgen:req:ios.pkg-ids+1]
fn legacy_divvun_keyboard_id(tag: &str) -> Option<&'static str> {
    Some(match tag {
        "se" | "sme" => "northern-sami-keyboard",
        "sms" => "skolt-sami-keyboard",
        "smn" => "inari-sami-keyboard",
        "smj-SE" => "julev-sami-keyboard",
        "smj-NO" => "julev-sami-keyboard-no",
        "sma" => "south-sami-keyboard",
        _ => return None,
    })
}

/// The bundle identifier of a layout's keyboard extension.
// [spec:kbdgen:req:ios.pkg-ids+1]
pub(crate) fn pkg_id(target: &iOS, layout: &Layout) -> String {
    let tag = layout.language_tag.as_str();
    let ext = if target.package_id == "no.uit.giella.keyboards.Sami" {
        legacy_divvun_keyboard_id(tag).unwrap_or(tag)
    } else {
        tag
    };
    format!("{}.{ext}", target.package_id)
}

pub trait IosProjectExt {
    fn all_pkg_ids(&self) -> Result<Vec<String>>;
}

impl IosProjectExt for KbdgenBundle {
    // [spec:kbdgen:def:ios.pkg-ids.all+1]
    fn all_pkg_ids(&self) -> Result<Vec<String>> {
        let target = ios_target(self)?;
        let mut ids: Vec<String> = std::iter::once(target.package_id.clone())
            .chain(
                self.layouts
                    .values()
                    .filter(|layout| layout.i_os.is_some())
                    .map(|layout| pkg_id(target, layout)),
            )
            .collect();
        ids.sort();
        Ok(ids)
    }
}

// [spec:kbdgen:req:ios.build-chain.init+1]
pub async fn init(bundle: KbdgenBundle, path: &Path) -> Result<()> {
    let target = ios_target(&bundle)?;
    let app_name = &bundle
        .project
        .locales
        .get(DEFAULT_LOCALE)
        .context("project.yaml has no `en` locale to name the app")?
        .name;
    let base_id = &target.package_id;
    let group = format!("group.{base_id}");
    let env = fastlane_env(target);

    tracing::info!("Registering {base_id} with App Store Connect");
    fastlane(
        path,
        &env,
        &["produce", "-a", base_id, "--app_name", app_name],
    )
    .await?;

    tracing::info!("Registering app group {group}");
    let group_name = format!("{app_name} Group");
    fastlane(
        path,
        &env,
        &["produce", "group", "-g", &group, "-n", &group_name],
    )
    .await?;

    let tasks: Vec<_> = bundle
        .all_pkg_ids()?
        .into_iter()
        .map(|id| {
            let path = path.to_path_buf();
            let env = env.clone();
            let base_id = base_id.clone();
            let group = group.clone();
            let app_name = format!("{app_name}: {}", keyboard_name(&id));

            tokio::spawn(async move {
                if id != base_id {
                    tracing::info!("Registering {id} with App Store Connect");
                    let args = ["produce", "-a", &id, "--app_name", &app_name, "--skip_itc"];
                    fastlane(&path, &env, &args).await?;
                }
                tracing::info!("Adding {id} to app group {group}");
                fastlane(
                    &path,
                    &env,
                    &["produce", "enable_services", "-a", &id, "--app-group"],
                )
                .await?;
                fastlane(
                    &path,
                    &env,
                    &["produce", "associate_group", "-a", &id, &group],
                )
                .await?;
                Ok::<_, anyhow::Error>(())
            })
        })
        .collect();

    for task in tasks {
        task.await.context("a fastlane task panicked")??;
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

    // [spec:kbdgen:req:ios.layouts/test]
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

    // [spec:kbdgen:req:ios.layouts/test]
    #[test]
    fn breaks_display_name_ties_by_language_tag() {
        let bundle = bundle(vec![
            ios_layout("smj-SE", "Lule Sami", true),
            ios_layout("smj-NO", "Lule Sami", true),
        ]);

        let layouts = ios_layouts(&bundle).unwrap();

        assert_eq!(tags(&layouts), ["smj-NO", "smj-SE"]);
    }

    // [spec:kbdgen:req:ios.layouts/test]
    #[test]
    fn skips_layouts_without_ios() {
        let mut no_ios = ios_layout("fi", "Finnish", true);
        no_ios.1.i_os = None;
        let bundle = bundle(vec![no_ios, ios_layout("se", "Northern Sami", true)]);

        let layouts = ios_layouts(&bundle).unwrap();

        assert_eq!(tags(&layouts), ["se"]);
    }

    // [spec:kbdgen:req:ios.layouts/test]
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

    // [spec:kbdgen:def:ios.keyboard-json+1/test]
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

    // [spec:kbdgen:def:ios.keyboard-json+1/test]
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

    fn target(package_id: &str) -> iOS {
        serde_yaml::from_str(&format!(
            "packageId: {package_id}\nbundleName: Keyboards\nversion: '1.0'\nbuild: 1\n"
        ))
        .unwrap()
    }

    // [spec:kbdgen:req:ios.pkg-ids+1/test]
    #[test]
    fn pkg_ids_use_the_legacy_table_only_for_the_sami_package() {
        let (_, se) = ios_layout("se", "Northern Sami", true);
        let (_, fi) = ios_layout("fi", "Finnish", true);
        let sami = target("no.uit.giella.keyboards.Sami");
        let other = target("com.example.keyboards");

        assert_eq!(
            pkg_id(&sami, &se),
            "no.uit.giella.keyboards.Sami.northern-sami-keyboard"
        );
        assert_eq!(pkg_id(&sami, &fi), "no.uit.giella.keyboards.Sami.fi");
        assert_eq!(pkg_id(&other, &se), "com.example.keyboards.se");
        assert_eq!(keyboard_name(&pkg_id(&sami, &se)), "northern-sami-keyboard");
    }

    // [spec:kbdgen:def:ios.pkg-ids.all+1/test]
    #[test]
    fn all_pkg_ids_are_sorted_and_need_an_ios_target() {
        let mut bundle = bundle(vec![
            ios_layout("se", "Northern Sami", true),
            ios_layout("fi", "Finnish", false),
        ]);

        assert_eq!(
            bundle.all_pkg_ids().unwrap_err().to_string(),
            "the bundle has no iOS target (targets/ios.yaml)"
        );

        bundle.targets.ios = Some(target("com.example.keyboards"));
        assert_eq!(
            bundle.all_pkg_ids().unwrap(),
            [
                "com.example.keyboards",
                "com.example.keyboards.fi",
                "com.example.keyboards.se"
            ]
        );
    }

    // [spec:kbdgen:req:ios.build-chain.init+1/test]
    #[tokio::test]
    async fn init_needs_an_ios_target_and_an_english_app_name() {
        let output = tempfile::tempdir().unwrap();

        let without_target = bundle(vec![ios_layout("se", "Northern Sami", true)]);
        let error = init(without_target, output.path()).await.unwrap_err();
        assert_eq!(
            error.to_string(),
            "the bundle has no iOS target (targets/ios.yaml)"
        );

        let mut without_english = bundle(vec![ios_layout("se", "Northern Sami", true)]);
        without_english.targets.ios = Some(target("com.example.keyboards"));
        let error = init(without_english, output.path()).await.unwrap_err();
        assert_eq!(
            error.to_string(),
            "project.yaml has no `en` locale to name the app"
        );
    }

    // [spec:kbdgen:req:ios.build-chain+3/test]
    // [spec:kbdgen:req:ios.build-chain.init+1/test]
    #[cfg(unix)]
    #[test]
    fn failed_commands_report_the_tail_of_their_output() {
        use std::os::unix::process::ExitStatusExt;

        let output = |code: i32, stdout: &str, stderr: &str| Output {
            status: std::process::ExitStatus::from_raw(code << 8),
            stdout: stdout.as_bytes().to_vec(),
            stderr: stderr.as_bytes().to_vec(),
        };
        let stderr: String = (1..=25).map(|line| format!("line {line}\n")).collect();
        let tail: Vec<String> = (6..=25).map(|line| format!("line {line}")).collect();

        assert!(ensure_success("tool", &output(0, "", "warning")).is_ok());
        assert_eq!(
            ensure_success("tool run", &output(1, "ignored", &stderr))
                .unwrap_err()
                .to_string(),
            format!("`tool run` failed with exit status: 1: {}", tail.join("\n"))
        );
        assert_eq!(
            ensure_success("fastlane sigh", &output(1, "[!] no profile\n", "  \n"))
                .unwrap_err()
                .to_string(),
            "`fastlane sigh` failed with exit status: 1: [!] no profile"
        );
    }
}
