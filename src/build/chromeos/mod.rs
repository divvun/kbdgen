//! The ChromeOS target: a Manifest V3 input-method extension whose service
//! worker runs `kbd-engine`, compiled to WebAssembly, on each layout's
//! `chromeOS` keyboard (`docs/spec/chromeos.md`,
//! `docs/spec/ldml/chromeos.md`).

use std::path::{Path, PathBuf};

use async_trait::async_trait;

use crate::bundle::KbdgenBundle;

pub use self::generate_chromeos::GenerateChromeOs;

use super::{BuildStep, BuildSteps};

mod classify;
#[cfg(test)]
mod differential;
mod generate_chromeos;
mod manifest;
mod source;
#[cfg(test)]
mod tests;

pub struct ChromeOsBuild {
    pub bundle: KbdgenBundle,
    pub output_path: PathBuf,
    pub steps: Vec<Box<dyn BuildStep>>,
}

#[async_trait(?Send)]
impl BuildSteps for ChromeOsBuild {
    fn new(bundle: KbdgenBundle, output_path: PathBuf) -> Self {
        let steps: Vec<Box<dyn BuildStep>> = vec![Box::new(GenerateChromeOs)];

        ChromeOsBuild {
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
