use std::{path::PathBuf, sync::Arc};

use crate::bundle::KbdgenBundle;

use build_klc::BuildKlc;
use generate_klc::GenerateKlc;

use super::{BuildStep, BuildSteps};

mod build_klc;
mod generate_klc;
mod klc;
mod layer_set;

pub struct WindowsBuild {
    pub bundle: Arc<KbdgenBundle>,
    pub output_path: PathBuf,
    pub steps: Vec<Box<dyn BuildStep>>,
}

impl BuildSteps for WindowsBuild {
    fn populate_steps(&mut self) {
        self.steps.push(Box::new(GenerateKlc {}));
        self.steps.push(Box::new(BuildKlc {}));
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
