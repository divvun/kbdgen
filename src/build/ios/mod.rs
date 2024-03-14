use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use async_trait::async_trait;
use language_tags::LanguageTag;
use once_cell::sync::Lazy;

use crate::bundle::{layout::Layout, project::Project, KbdgenBundle};

use self::{
    clone_giellakbd::CloneGiellaKbd,
    generate_ios::GenerateIos,
    generate_xcode::GenerateXcode,
    pod_install::PodInstall,
    xcodebuild::{fastlane_env, BuildXcarchive, FastlaneProvisioning},
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
