use std::{path::Path, sync::Arc};

use async_trait::async_trait;
use xmlem::Document;

use crate::{build::BuildStep, bundle::KbdgenBundle};

const TOP_FOLDER: &str = "Contents";
const RESOURCES_FOLDER: &str = "Resources";
const PLIST_FILENAME: &str = "Info.plist";

const PLIST_TEMPLATE: &str = include_str!("../../../resources/template-macos-plist.xml");

pub struct GenerateMacOs {}

#[async_trait(?Send)]
impl BuildStep for GenerateMacOs {
    async fn build(&self, bundle: Arc<KbdgenBundle>, output_path: &Path) {
        let contents_path = output_path.join(TOP_FOLDER);
        let resources_path = contents_path.join(RESOURCES_FOLDER);
        std::fs::create_dir_all(contents_path).unwrap();
        std::fs::create_dir_all(resources_path).unwrap();

        let document = Document::from_str(PLIST_TEMPLATE).unwrap();
    }
}
