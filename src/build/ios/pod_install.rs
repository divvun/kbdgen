use std::{ffi::OsString, path::Path, process::Command};

use anyhow::{Context, Result, bail};
use async_trait::async_trait;

use crate::{build::BuildStep, bundle::KbdgenBundle};

use super::REPOSITORY_FOLDER;

pub struct PodInstall;

// [spec:kbdgen:req:ios.build-chain+2]
#[async_trait(?Send)]
impl BuildStep for PodInstall {
    async fn build(&self, _bundle: &KbdgenBundle, output_path: &Path) -> Result<()> {
        let path = output_path.join(REPOSITORY_FOLDER);
        let mut project_directory = OsString::from("--project-directory=");
        project_directory.push(&path);

        tracing::info!("Running pod install in {}", path.display());
        let status = Command::new("pod")
            .args(["install", "--repo-update"])
            .arg(project_directory)
            .status()
            .context("`pod` could not be started; install CocoaPods")?;
        if !status.success() {
            bail!("`pod install` failed with {status}; its output is above");
        }

        Ok(())
    }
}
