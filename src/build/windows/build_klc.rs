use std::{path::Path, sync::Arc};

use crate::{build::BuildStep, bundle::KbdgenBundle};

pub struct BuildKlc {}

impl BuildStep for BuildKlc {
    fn build(&self, bundle: Arc<KbdgenBundle>, output_path: &Path) {}
}
