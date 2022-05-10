use std::path::{Path, PathBuf};

use async_trait::async_trait;

use crate::bundle::KbdgenBundle;

use self::{generate_ios::GenerateIos, clone_giellakbd::CloneGiellaKbd};

use super::{BuildStep, BuildSteps};

pub mod clone_giellakbd;
pub mod generate_ios;

const REPOSITORY_FOLDER: &str = "repo";

pub struct IosBuild {
    pub bundle: KbdgenBundle,
    pub output_path: PathBuf,
    pub steps: Vec<Box<dyn BuildStep>>,
}

#[async_trait(?Send)]
impl BuildSteps for IosBuild {
    fn new(bundle: KbdgenBundle, output_path: PathBuf) -> Self {
        let steps: Vec<Box<dyn BuildStep>> =
            vec![Box::new(CloneGiellaKbd), Box::new(GenerateIos)];

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
