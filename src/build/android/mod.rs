use std::path::{Path, PathBuf};

use anyhow::Result;
use async_trait::async_trait;

use crate::{build::pahkat, bundle::KbdgenBundle};

use self::{clone_giellakbd::CloneGiellaKbd, generate_android::GenerateAndroid};

use super::{BuildStep, BuildSteps};

pub mod clone_giellakbd;
pub mod generate_android;

const REPOSITORY_FOLDER: &str = "repo";

pub struct AndroidBuild {
    pub bundle: KbdgenBundle,
    pub output_path: PathBuf,
    pub steps: Vec<Box<dyn BuildStep>>,
}

pub struct DownloadDependencies;

#[async_trait(?Send)]
impl BuildStep for DownloadDependencies {
    async fn build(&self, _bundle: &KbdgenBundle, _output_path: &Path) -> Result<()> {
        pahkat::install_android_deps().await;

        Ok(())
    }
}

#[async_trait(?Send)]
impl BuildSteps for AndroidBuild {
    fn new(bundle: KbdgenBundle, output_path: PathBuf) -> Self {
        let steps: Vec<Box<dyn BuildStep>> = vec![
            Box::new(DownloadDependencies),
            Box::new(CloneGiellaKbd),
            Box::new(GenerateAndroid),
        ];

        AndroidBuild {
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
