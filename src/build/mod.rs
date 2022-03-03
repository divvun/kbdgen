mod windows;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::bundle::KbdgenBundle;

use windows::klc::KlcFile;

pub trait BuildSteps {
    fn populate_steps(&mut self);
    fn count(&self) -> usize;
    fn build_full(&self);
}

pub trait BuildStep {
    fn build(&self, bundle: Arc<KbdgenBundle>, output_path: &Path);
}

pub struct WindowsBuild {
    pub bundle: Arc<KbdgenBundle>,
    pub output_path: PathBuf,
    pub steps: Vec<Box<dyn BuildStep>>,
}

impl BuildSteps for WindowsBuild {
    fn populate_steps(&mut self) {
        &self.steps.push(Box::new(GenerateKlc {}));
        &self.steps.push(Box::new(Print {}));
    }

    fn count(&self) -> usize {
        *&self.steps.len()
    }

    fn build_full(&self) {
        &self.steps.iter().for_each(|step| {
            step.build(self.bundle.clone(), &self.output_path);
        });
    }
}

pub struct GenerateKlc {}

impl BuildStep for GenerateKlc {
    fn build(&self, bundle: Arc<KbdgenBundle>, output_path: &Path) {
        let klc_file = KlcFile {};
    }
}

pub struct WriteKlc {}

impl BuildStep for WriteKlc {
    fn build(&self, _bundle: Arc<KbdgenBundle>, output_path: &Path) {}
}

pub struct Print {}

impl BuildStep for Print {
    fn build(&self, _bundle: Arc<KbdgenBundle>, output_path: &Path) {
        println!("print step");
    }
}
