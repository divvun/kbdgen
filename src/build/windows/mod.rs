use std::{
    path::{Path, PathBuf}
};

use async_trait::async_trait;

use crate::bundle::KbdgenBundle;

use super::{BuildStep, BuildSteps};

use generate_klc::GenerateKlc;

#[cfg(target_os = "windows")]
mod build_klc;
mod generate_klc;
mod klc;
mod layer_set;

pub struct WindowsBuild {
    pub bundle: KbdgenBundle,
    pub output_path: PathBuf,
    pub steps: Vec<Box<dyn BuildStep>>,
}

#[async_trait(?Send)]
impl BuildSteps for WindowsBuild {
    fn new(bundle: KbdgenBundle, output_path: PathBuf) -> Self {
        let steps = vec![Box::new(GenerateKlc {}) as _];
        #[cfg(target_os = "windows")]
        steps.push(Box::new(build_klc::BuildKlc {}));
        #[cfg(not(target_os = "windows"))]
        {
            tracing::warn!("Skipping BuildKlc step");
            tracing::warn!(".klc .dlls require MSKLC to build, which is only available on Windows");
        }

        Self {
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
