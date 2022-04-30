use std::{path::PathBuf, sync::Arc};

use async_trait::async_trait;

use crate::bundle::KbdgenBundle;

use super::{BuildStep, BuildSteps};

use generate_mac_os::GenerateMacOs;

mod generate_mac_os;
mod keymap;
mod layers;
mod util;

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
