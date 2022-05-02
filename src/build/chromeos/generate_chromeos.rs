use std::path::Path;

use async_trait::async_trait;

use crate::{build::BuildStep, bundle::KbdgenBundle};

pub struct GenerateChromeOs;

#[async_trait(?Send)]
impl BuildStep for GenerateChromeOs {
    async fn build(&self, bundle: &KbdgenBundle, output_path: &Path) {
        // One .klc file per language with Windows primary platform
        for (language_tag, layout) in &bundle.layouts {
            if let Some(chromeos_target) = &layout.chrome_os {}
        }
    }
}
