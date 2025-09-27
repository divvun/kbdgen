use std::path::{Path, PathBuf};

use async_trait::async_trait;

use crate::bundle::KbdgenBundle;
use anyhow::Result;

pub mod android;
pub mod chromeos;
pub mod ios;
pub mod macos;
pub mod pahkat;
#[allow(dead_code)]
pub mod svg;
pub mod windows;

#[async_trait(?Send)]
pub trait BuildSteps {
    fn new(bundle: KbdgenBundle, output_path: PathBuf) -> Self
    where
        Self: Sized;

    fn steps(&self) -> &[Box<dyn BuildStep>];
    fn bundle(&self) -> &KbdgenBundle;
    fn output_path(&self) -> &Path;

    async fn build_full(&self) -> Result<()> {
        for step in self.steps() {
            step.build(self.bundle(), self.output_path()).await?;
        }

        Ok(())
    }
}

#[async_trait(?Send)]
pub trait BuildStep {
    async fn build(&self, bundle: &KbdgenBundle, output_path: &Path) -> Result<()>;
}
