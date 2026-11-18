//! The Linux target (`docs/spec/linux.md`, `docs/spec/ldml/linux.md`): the
//! names of the bundle's Linux output (`ids`) and the `linux` keyboard of
//! each layout that has one (`source`).

use std::path::{Path, PathBuf};

use async_trait::async_trait;

use crate::bundle::KbdgenBundle;

pub use self::generate_linux::GenerateLinux;

use super::{BuildStep, BuildSteps};

mod generate_linux;
pub mod ids;
pub mod source;
#[cfg(test)]
mod tests;

pub struct LinuxBuild {
    pub bundle: KbdgenBundle,
    pub output_path: PathBuf,
    pub steps: Vec<Box<dyn BuildStep>>,
}

#[async_trait(?Send)]
impl BuildSteps for LinuxBuild {
    fn new(bundle: KbdgenBundle, output_path: PathBuf) -> Self {
        let steps: Vec<Box<dyn BuildStep>> = vec![Box::new(GenerateLinux)];

        LinuxBuild {
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
