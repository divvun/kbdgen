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
    xcodebuild::{BuildXcarchive, FastlaneProvisioning},
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
