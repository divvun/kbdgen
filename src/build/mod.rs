use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;

use crate::bundle::KbdgenBundle;

pub mod macos;
#[cfg(target_os = "windows")]
pub mod pahkat;
pub mod svg;
pub mod windows;

#[async_trait(?Send)]
pub trait BuildSteps {
    fn populate_steps(&mut self);
    fn count(&self) -> usize;
    async fn build_full(&self);
}

#[async_trait(?Send)]
pub trait BuildStep {
    async fn build(&self, bundle: Arc<KbdgenBundle>, output_path: &Path);
}
