use std::path::PathBuf;
use std::sync::Arc;

use crate::KbdgenBundle;

use super::{BuildStep, BuildSteps};

use klc::GenerateKlc;

pub mod klc;

pub struct WindowsBuild {
    pub bundle: Arc<KbdgenBundle>,
    pub output_path: PathBuf,
    pub steps: Vec<Box<dyn BuildStep>>,
}

impl BuildSteps for WindowsBuild {
    fn populate_steps(&mut self) {
        self.steps.push(Box::new(GenerateKlc {}));
    }

    fn count(&self) -> usize {
        *&self.steps.len()
    }

    fn build_full(&self) {
        self.steps.iter().for_each(|step| {
            step.build(self.bundle.clone(), &self.output_path);
        });
    }
}
