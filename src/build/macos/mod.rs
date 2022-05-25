mod generate_macos;
mod keymap;
mod layers;
mod macos_bundle;
mod package_macos;
mod util;

use std::path::{Path, PathBuf};

use async_trait::async_trait;

use super::{BuildStep, BuildSteps};
use crate::bundle::KbdgenBundle;
pub use generate_macos::GenerateMacOs;
pub use package_macos::GenerateInstaller;

pub struct MacOsBuild {
    pub bundle: KbdgenBundle,
    pub output_path: PathBuf,
    pub steps: Vec<Box<dyn BuildStep>>,
}

#[async_trait(?Send)]
impl BuildSteps for MacOsBuild {
    fn new(bundle: KbdgenBundle, output_path: PathBuf) -> Self {
        #[allow(unused_mut)]
        let mut steps: Vec<Box<dyn BuildStep>> = vec![Box::new(GenerateMacOs)];
        #[cfg(target_os = "macos")]
        steps.push(Box::new(GenerateInstaller));
        #[cfg(not(target_os = "macos"))]
        {
            tracing::warn!("Skipping Installer step");
            tracing::warn!("pkgutil and friends are only available on macOS");
        }

        MacOsBuild {
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
