use std::{cmp::Ordering, fmt, path::Path};

use async_trait::async_trait;
use language_tags::LanguageTag;
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::{build::BuildStep, bundle::KbdgenBundle};

const REPOSITORY: &str = "repo";
const HOSTING_APP: &str = "HostingApp";
const INFO_PLIST_STRINGS: &str = "InfoPlist.strings";

const KEYBOARD: &str = "Keyboard";
const NORTHERN_SAMI: &str = "northern-sami-keyboard";
const INFO_PLIST: &str = "Info.plist";

#[derive(Serialize, Deserialize, Debug)]
pub struct XcodeInfoPlist {
    #[serde(rename = "CFBundleName")]
    cf_bundle_name: String,
    #[serde(rename = "CFBundleDisplayName")]
    cf_bundle_display_name: String,
}

impl fmt::Display for XcodeInfoPlist {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.write_str(&format!(
            "\"CFBundleName\" = {:?};\n\"CFBundleDisplayName\" = {:?};",
            self.cf_bundle_name, self.cf_bundle_display_name
        ))
    }
}

// LAYOUT PLIST START

#[derive(Serialize, Deserialize, Debug)]
pub struct LayoutInfoPlistExtensionAttributes {
    #[serde(rename = "IsASCIICapable")]
    is_ascii_capable: bool,
    #[serde(rename = "PrefersRightToLeft")]
    prefers_right_to_left: bool,
    #[serde(rename = "PrimaryLanguage")]
    primary_language: String,
    #[serde(rename = "RequestsOpenAccess")]
    requests_open_access: bool,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct LayoutInfoPlistExtension {
    #[serde(rename = "NSExtensionAttributes")]
    ns_extension_attributes: LayoutInfoPlistExtensionAttributes,
    #[serde(rename = "NSExtensionPointIdentifier")]
    ns_extension_point_identifier: String,
    #[serde(rename = "NSExtensionPrincipalClass")]
    ns_extension_principal_class: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct LayoutInfoPlist {
    #[serde(rename = "DivvunSpellerPath")]
    divvun_speller_path: String,
    #[serde(rename = "DivvunSpellerPackageKey")]
    divvun_speller_package_key: String,
    #[serde(rename = "CFBundleDevelopmentRegion")]
    cf_bundle_development_region: String,
    #[serde(rename = "CFBundleDisplayName")]
    cf_bundle_display_name: String,
    #[serde(rename = "CFBundleExecutable")]
    cf_bundle_executable: String,
    #[serde(rename = "CFBundleIdentifier")]
    cf_bundle_identifier: String,
    #[serde(rename = "CFBundleInfoDictionaryVersion")]
    cf_bundle_info_dictionary_version: String,
    #[serde(rename = "CFBundleName")]
    cf_bundle_name: String,
    #[serde(rename = "CFBundlePackageType")]
    cf_bundle_package_type: String,
    #[serde(rename = "CFBundleShortVersionString")]
    cf_bundle_short_version_string: String,
    #[serde(rename = "CFBundleSignature")]
    cf_bundle_signature: String,
    #[serde(rename = "CFBundleVersion")]
    cf_bundle_version: String,
    #[serde(rename = "DivvunKeyboardIndex")]
    divvun_keyboard_index: usize,
    #[serde(rename = "ITSAppUsesNonExemptEncryption")]
    its_app_uses_non_exempt_encryption: bool,
    #[serde(rename = "LSApplicationQueriesSchemes")]
    ls_application_queries_schemes: Vec<String>,
    #[serde(rename = "NSExtension")]
    ns_extension: LayoutInfoPlistExtension,
}

// LAYOUT PLIST END

pub fn replace_all_occurances(input: String, character: char, replace_with: char) -> String {
    input
        .as_str()
        .chars()
        .map(|curr| {
            if curr.cmp(&character) != Ordering::Equal {
                curr
            } else {
                replace_with
            }
        })
        .into_iter()
        .collect::<String>()
}

pub struct GenerateXcode;

#[async_trait(?Send)]
impl BuildStep for GenerateXcode {
    async fn build(&self, bundle: &KbdgenBundle, output_path: &Path) {
        let repository_path = output_path.join(REPOSITORY);

        for (layout_index, (language_tag, layout)) in bundle.layouts.iter().enumerate() {
            if let Some(target) = &bundle.targets.ios {
                if let Some(_ios_layout) = &layout.i_os {
                    // GENERATE LOCALES
                    for (locale_name, locale_info) in &bundle.project.locales {
                        let hosting_app_path = repository_path.join(HOSTING_APP);
                        let locale_name = if locale_name == "en" {
                            "Base"
                        } else {
                            locale_name
                        };
                        let locale_path = hosting_app_path.join(&format!("{}.lproj", locale_name));
                        let info_path = locale_path.join(INFO_PLIST_STRINGS);
                        println!("{:?}", locale_path);

                        std::fs::create_dir_all(&locale_path).unwrap();

                        let info_plist = XcodeInfoPlist {
                            cf_bundle_name: locale_info.name.to_string(),
                            cf_bundle_display_name: locale_info.name.to_string(),
                        };
                        std::fs::write(info_path, info_plist.to_string()).unwrap();
                    }

                    // GENERATE LAYOUTS

                    let keyboard_name = replace_all_occurances(
                        bundle
                            .project
                            .locales
                            .get("en")
                            .unwrap()
                            .name
                            .to_lowercase(),
                        ' ',
                        '-',
                    );

                    let keyboard_path = repository_path.join(KEYBOARD);
                    let info_plist_template = keyboard_path.join(INFO_PLIST);

                    let layout_path = keyboard_path.join(keyboard_name);
                    let layout_info_plist_path = layout_path.join(INFO_PLIST);

                    std::fs::create_dir_all(&layout_path).unwrap();

                    let mut parsed_plist: LayoutInfoPlist =
                        plist::from_file(info_plist_template.clone()).expect("valid stuff");

                    parsed_plist.cf_bundle_display_name = layout.autonym().to_string();
                    parsed_plist.cf_bundle_short_version_string = target.version.clone();
                    parsed_plist.cf_bundle_version = target.build.clone();
                    parsed_plist.ls_application_queries_schemes[0] = target.package_id.clone();
                    parsed_plist
                        .ns_extension
                        .ns_extension_attributes
                        .primary_language = language_tag.to_string();
                    parsed_plist.divvun_keyboard_index = layout_index;

                    plist::to_file_xml(layout_info_plist_path.clone(), &parsed_plist).unwrap();
                }
            }
        }
    }
}
