use std::{path::Path, process::Command};

use anyhow::Result;
use async_trait::async_trait;

use crate::{build::BuildStep, bundle::KbdgenBundle};

use super::REPOSITORY_FOLDER;

pub struct CloneGiellaKbd;

#[async_trait(?Send)]
impl BuildStep for CloneGiellaKbd {
    async fn build(&self, _bundle: &KbdgenBundle, output_path: &Path) -> Result<()> {
        let repo_url = "https://github.com/divvun/giellakbd-android";

        Command::new("git")
            .arg("clone")
            .arg(repo_url)
            .arg(REPOSITORY_FOLDER)
            .current_dir(output_path)
            .status()
            .expect("to clone a public repo with no hippos");

        Ok(())
    }
}
