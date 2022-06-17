use std::path::Path;

use crate::{build::BuildStep, bundle::KbdgenBundle};
use async_trait::async_trait;

pub struct PodInstall;

#[async_trait(?Send)]
impl BuildStep for PodInstall {
    async fn build(&self, _bundle: &KbdgenBundle, output_path: &Path) {
        let path = output_path.join("repo");

        tracing::debug!("Run pod install on {:?}", &path);

        std::process::Command::new("pod")
            .arg("install")
            .arg(format!("--project-directory={}", path.to_str().unwrap()))
            .status()
            .unwrap();

        tracing::debug!("Pod install finished");
    }
}
