use std::{path::Path, process::Command};

use anyhow::{Context, Result, bail};
use async_trait::async_trait;

use crate::{build::BuildStep, bundle::KbdgenBundle};

use super::REPOSITORY_FOLDER;

const REPOSITORY_URL: &str = "https://github.com/divvun/giellakbd-android";

pub struct CloneGiellaKbd;

// [spec:kbdgen:req:android.dependencies+1]
#[async_trait(?Send)]
impl BuildStep for CloneGiellaKbd {
    async fn build(&self, _bundle: &KbdgenBundle, output_path: &Path) -> Result<()> {
        let repo_path = output_path.join(REPOSITORY_FOLDER);
        tracing::info!("Cloning {REPOSITORY_URL} into {}", repo_path.display());

        let output = Command::new("git")
            .args(["clone", REPOSITORY_URL, REPOSITORY_FOLDER])
            .current_dir(output_path)
            .output()
            .context("`git` could not be started")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // The clone fails when an earlier run left a checkout behind,
            // which the later steps reuse.
            if !repo_path.is_dir() {
                bail!(
                    "`git clone {REPOSITORY_URL}` failed with {}: {}",
                    output.status,
                    stderr.trim()
                );
            }
            tracing::info!("Using the existing checkout at {}", repo_path.display());
            tracing::debug!("git clone: {}", stderr.trim());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bundle::{fixture, read_kbdgen_bundle};

    // [spec:kbdgen:req:android.dependencies+1/test]
    #[tokio::test]
    async fn a_failed_clone_over_an_existing_checkout_reuses_it() {
        let root = tempfile::tempdir().unwrap();
        let bundle =
            read_kbdgen_bundle(&fixture::write_bundle(root.path(), "t", &[], &[], &[])).unwrap();
        let output_path = root.path().join("out");
        let kept = output_path.join(REPOSITORY_FOLDER).join("kept");
        std::fs::create_dir_all(kept.parent().unwrap()).unwrap();
        std::fs::write(&kept, "").unwrap();

        CloneGiellaKbd.build(&bundle, &output_path).await.unwrap();

        assert!(kept.is_file());
    }
}
