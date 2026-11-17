use std::{path::Path, process::Command};

use anyhow::{Context, Result};
use async_trait::async_trait;

use crate::{build::BuildStep, bundle::KbdgenBundle};

use super::{REPOSITORY_FOLDER, ensure_success};

const REPOSITORY_URL: &str = "https://github.com/divvun/giellakbd-ios.git";

pub struct CloneGiellaKbd;

// [spec:kbdgen:req:ios.build-chain+3]
#[async_trait(?Send)]
impl BuildStep for CloneGiellaKbd {
    async fn build(&self, _bundle: &KbdgenBundle, output_path: &Path) -> Result<()> {
        tracing::info!(
            "Cloning {REPOSITORY_URL} into {}",
            output_path.join(REPOSITORY_FOLDER).display()
        );
        let output = Command::new("git")
            .args(["clone", REPOSITORY_URL, REPOSITORY_FOLDER])
            .current_dir(output_path)
            .output()
            .context("`git` could not be started")?;
        ensure_success(&format!("git clone {REPOSITORY_URL}"), &output)
    }
}
