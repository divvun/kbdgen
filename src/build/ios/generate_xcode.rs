use std::{path::Path, process::Command, fs::{read_to_string, File}, fmt};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::to_string_pretty;
use xmlem::Document;

use crate::{build::BuildStep, bundle::KbdgenBundle};

const REPOSITORY: &str = "repo";
const HOSTING_APP: &str = "HostingApp";
const INFO: &str = "Info.plist";

#[derive(Serialize, Deserialize, Debug)]
pub struct XcodeInfoPlist {
    #[serde(rename = "CFBundleName")]
    cf_bundle_name: String,
    #[serde(rename = "CFBundleDisplayName")]
    cf_bundle_display_name: String,
}

impl fmt::Display for XcodeInfoPlist {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.write_str(&format!("\"CFBundleName\" = {:?};\n\"CFBundleDisplayName\" = {:?};", self.cf_bundle_name, self.cf_bundle_display_name))
    }
}

pub struct GenerateXcode;

#[async_trait(?Send)]
impl BuildStep for GenerateXcode {
    async fn build(&self, bundle: &KbdgenBundle, output_path: &Path) {
        let repository_path = output_path.join(REPOSITORY);
        let hosting_app_path = repository_path.join(HOSTING_APP);

        // let file = File::open(info_path).unwrap();
        // let xml_info = Document::from_file(file);

        for (locale_name, locale_info) in &bundle.project.locales {
            // println!("NAME: {:?}, INFO: {:?}", locale_name, locale_info);
            
            // DIRECTORY: {locale}.lproj
            let locale_name = if locale_name == "en" {"Base"} else {locale_name};
            let locale_path = hosting_app_path.join(&format!("{}.lproj", locale_name));
            let info_path = locale_path.join(INFO);
            println!("{:?}", locale_path);

            std::fs::create_dir_all(&locale_path).unwrap();

            // FILE: About.txt

            let info_plist = XcodeInfoPlist {
                cf_bundle_name: locale_info.name.to_string(),
                cf_bundle_display_name: locale_info.name.to_string(),
            };

            // FILE: InfoPlist.strings
            std::fs::write(
                info_path,
                info_plist.to_string(),
            ).unwrap();
        }
        
        println!("TODO: GENERATE .pxbproj");
    }
}
