use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use async_trait::async_trait;

use crate::bundle::KbdgenBundle;

use super::{BuildStep, BuildSteps};

pub struct MacOsBuild {
    pub bundle: Arc<KbdgenBundle>,
    pub output_path: PathBuf,
    pub steps: Vec<Box<dyn BuildStep + Send + Sync>>,
}

#[async_trait(?Send)]
impl BuildSteps for MacOsBuild {
    fn populate_steps(&mut self) {
        self.steps.push(Box::new(GenerateMacOs {}));
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

pub struct GenerateMacOs {}

#[async_trait(?Send)]
impl BuildStep for GenerateMacOs {
    async fn build(&self, bundle: Arc<KbdgenBundle>, output_path: &Path) {}
}
