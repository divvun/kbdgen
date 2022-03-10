use std::path::Path;
use std::sync::Arc;

use crate::bundle::KbdgenBundle;

pub mod svg;
pub mod windows;

pub trait BuildSteps {
    fn populate_steps(&mut self);
    fn count(&self) -> usize;
    fn build_full(&self);
}

pub trait BuildStep {
    fn build(&self, bundle: Arc<KbdgenBundle>, output_path: &Path);
}
