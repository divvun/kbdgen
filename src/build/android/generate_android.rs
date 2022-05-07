use std::path::Path;

use async_trait::async_trait;

use crate::{build::BuildStep, bundle::KbdgenBundle};

pub struct GenerateAndroid;

#[async_trait(?Send)]
impl BuildStep for GenerateAndroid {
    async fn build(&self, bundle: &KbdgenBundle, output_path: &Path) {}
}
