use std::path::Path;

use crate::{build::BuildStep, bundle::KbdgenBundle};
use async_trait::async_trait;

pub struct PodInstall;

#[async_trait(?Send)]
impl BuildStep for PodInstall {
    async fn build(&self, bundle: &KbdgenBundle, output_path: &Path) {
        let path = output_path.join("repo");

        std::process::Command::new("pod")
            .arg("install")
            .arg(format!("--project-directory={}", path.to_str().unwrap()))
            .status()
            .unwrap();
    }
}
