use std::{cmp::Ordering, fmt, path::Path};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::{build::BuildStep, bundle::KbdgenBundle};

const REPOSITORY: &str = "repo";
const HOSTING_APP: &str = "HostingApp";
const INFO_PLIST_STRINGS: &str = "InfoPlist.strings";

const KEYBOARD: &str = "Keyboard";
const INFO_PLIST: &str = "Info.plist";

const SETTINGS_BUNDLE: &str = "Settings.bundle";
const ROOT_PLIST: &str = "Root.plist";
const ENTITLEMENTS_EXTENSION: &str = ".entitlements";

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

#[derive(Serialize, Deserialize, Debug)]
pub struct EntitlementsDict {
    #[serde(rename = "com.apple.security.application-groups")]
    com_apple_security_application_groups: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PreferenceSpecifier {
    #[serde(rename = "Type")]
    preference_type: String,
    #[serde(rename = "Title")]
    title: String,
    #[serde(rename = "Key")]
    key: String,
    #[serde(rename = "DefaultValue")]
    default_value: bool,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SettingsRootDict {
    #[serde(rename = "StringsTable")]
    strings_table: String,
    #[serde(rename = "PreferenceSpecifiers")]
    preference_specifiers: Vec<PreferenceSpecifier>,
    #[serde(rename = "ApplicationGroupContainerIdentifier")]
    application_group_container_identifier: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BundleSchemes {
    CFBundleURLSchemes: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct HostingPlist {
    CFBundleDevelopmentRegion: String,
    CFBundleDisplayName: String,
    CFBundleExecutable: String,
    CFBundleIdentifier: String,
    CFBundleInfoDictionaryVersion: String,
    CFBundleName: String,
    CFBundlePackageType: String,
    CFBundleShortVersionString: String,
    CFBundleSignature: String,
    CFBundleURLTypes: Vec<BundleSchemes>,
    CFBundleVersion: String,
    ITSAppUsesNonExemptEncryption: bool,
    LSApplicationQueriesSchemes: Vec<String>,
    LSRequiresIPhoneOS: bool,
    UIBackgroundModes: Vec<String>,
    UILaunchStoryboardName: String,
    UIRequiredDeviceCapabilities: Vec<String>,
    UISupportedInterfaceOrientations: Vec<String>,
    UIUserInterfaceStyle: String,
}

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
        let hosting_app_path = repository_path.join(HOSTING_APP);
        let keyboard_path = repository_path.join(KEYBOARD);

        for (layout_index, (language_tag, layout)) in bundle.layouts.iter().enumerate() {
            if let Some(target) = &bundle.targets.ios {
                if let Some(_ios_layout) = &layout.i_os {
                    // GENERATE LOCALES
                    for (locale_name, locale_info) in &bundle.project.locales {
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

                    // KEYBOARD PLIST
                    let keyboard_folder_name = replace_all_occurances(
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

                    let info_plist_template = keyboard_path.join(INFO_PLIST);

                    let layout_path = keyboard_path.join(keyboard_folder_name);
                    let layout_info_plist_path = layout_path.join(INFO_PLIST);

                    std::fs::create_dir_all(&layout_path).unwrap();

                    let mut parsed_keyboard_plist: LayoutInfoPlist =
                        plist::from_file(info_plist_template.clone()).expect("valid stuff");

                    parsed_keyboard_plist.cf_bundle_display_name = layout.autonym().to_string();
                    parsed_keyboard_plist.cf_bundle_short_version_string = target.version.clone();
                    parsed_keyboard_plist.cf_bundle_version = target.build.clone();
                    parsed_keyboard_plist.ls_application_queries_schemes[0] =
                        target.package_id.clone();
                    parsed_keyboard_plist
                        .ns_extension
                        .ns_extension_attributes
                        .primary_language = language_tag.to_string();
                    parsed_keyboard_plist.divvun_keyboard_index = layout_index;

                    plist::to_file_xml(layout_info_plist_path.clone(), &parsed_keyboard_plist)
                        .unwrap();

                    // HOSTING APP PLIST
                    let hosting_app_plist_path = hosting_app_path.join(INFO_PLIST);
                    let mut hosting_app_plist: HostingPlist =
                        plist::from_file(hosting_app_plist_path.clone()).expect("valid stuff");

                    hosting_app_plist.CFBundleDisplayName =
                        bundle.project.locales.get("en").unwrap().name.clone();
                    hosting_app_plist.CFBundleShortVersionString = target.version.clone();
                    hosting_app_plist.CFBundleVersion = target.build.clone();
                    hosting_app_plist.CFBundleURLTypes[0].CFBundleURLSchemes[0] =
                        target.package_id.clone();
                    hosting_app_plist.LSApplicationQueriesSchemes[0] = target.package_id.clone();

                    plist::to_file_xml(hosting_app_plist_path.clone(), &hosting_app_plist).unwrap();

                    // UPDATE ENTITLEMENTS
                    let new_entitlements = format!("{}.{}", "group", target.package_id);

                    // KEYBOARD
                    let keyboard_entitlements_path =
                        keyboard_path.join(format!("{}{}", KEYBOARD, ENTITLEMENTS_EXTENSION));
                    let mut keyboard_entitlements: EntitlementsDict =
                        plist::from_file(keyboard_entitlements_path.clone()).expect("valid stuff");
                    keyboard_entitlements.com_apple_security_application_groups =
                        vec![new_entitlements.clone()];
                    plist::to_file_xml(keyboard_entitlements_path.clone(), &keyboard_entitlements)
                        .unwrap();

                    // HOSTING APP
                    let hosting_app_entitlements_path =
                        hosting_app_path.join(format!("{}{}", HOSTING_APP, ENTITLEMENTS_EXTENSION));
                    let mut hosting_app_entitlements: EntitlementsDict =
                        plist::from_file(hosting_app_entitlements_path.clone())
                            .expect("valid stuff");
                    hosting_app_entitlements.com_apple_security_application_groups =
                        vec![new_entitlements.clone()];
                    plist::to_file_xml(
                        hosting_app_entitlements_path.clone(),
                        &hosting_app_entitlements,
                    )
                    .unwrap();

                    // ROOT PLIST
                    let root_plist_path = hosting_app_path.join(SETTINGS_BUNDLE).join(ROOT_PLIST);
                    let mut root_plist: SettingsRootDict =
                        plist::from_file(root_plist_path.clone()).expect("valid stuff");
                    root_plist.application_group_container_identifier = new_entitlements.clone();
                    plist::to_file_xml(root_plist_path.clone(), &root_plist).unwrap();
                }
            }
        }
    }
}
