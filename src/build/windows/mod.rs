use std::{path::PathBuf, sync::Arc};

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
    pub bundle: Arc<KbdgenBundle>,
    pub output_path: PathBuf,
    pub steps: Vec<Box<dyn BuildStep>>,
}

#[async_trait(?Send)]
impl BuildSteps for WindowsBuild {
    fn populate_steps(&mut self) {
        self.steps.push(Box::new(GenerateKlc {}));
        #[cfg(target_os = "windows")]
        self.steps.push(Box::new(build_klc::BuildKlc {}));
        #[cfg(not(target_os = "windows"))]
        {
            tracing::warn!("Skipping BuildKlc step");
            tracing::warn!(".klc .dlls require MSKLC to build, which is only available on Windows");
        }
    }

    fn count(&self) -> usize {
        *&self.steps.len()
    }

    async fn build_full(&self) {
        for step in &self.steps {
            step.build(self.bundle.clone(), &self.output_path).await;
        }
    }
}
