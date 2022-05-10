use std::path::{Path, PathBuf};

use async_trait::async_trait;

use crate::bundle::KbdgenBundle;

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

#[async_trait(?Send)]
impl BuildSteps for AndroidBuild {
    fn new(bundle: KbdgenBundle, output_path: PathBuf) -> Self {
        let steps: Vec<Box<dyn BuildStep>> =
            vec![Box::new(CloneGiellaKbd), Box::new(GenerateAndroid)];

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
