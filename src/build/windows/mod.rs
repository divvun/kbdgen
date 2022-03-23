use std::{path::PathBuf, sync::Arc};

use async_trait::async_trait;

use crate::bundle::KbdgenBundle;

use super::{BuildStep, BuildSteps};

use build_klc::BuildKlc;
use generate_klc::GenerateKlc;

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
        self.steps.push(Box::new(BuildKlc {}));
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
