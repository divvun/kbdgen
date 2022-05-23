use std::{
    cmp::Ordering,
    fmt,
    path::{Path, PathBuf},
    str::FromStr,
};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::{
    build::{ios::pbxproj::Pbxproj, BuildStep},
    bundle::KbdgenBundle,
};

use super::pbxproj::{self, convert_pbxproj_to_json};

const REPOSITORY: &str = "repo";
const HOSTING_APP: &str = "HostingApp";
const HOSTING_INFO_STRINGS: &str = "InfoPlist.strings";

const KEYBOARD: &str = "Keyboard";
const INFO_PLIST: &str = "Info.plist";

const SETTINGS_BUNDLE: &str = "Settings.bundle";
const ROOT_PLIST: &str = "Root.plist";
const ENTITLEMENTS_EXTENSION: &str = ".entitlements";

#[derive(Serialize, Deserialize, Debug)]
pub struct XcodeHostingInfoStrings {
    #[serde(rename = "CFBundleName")]
    cf_bundle_name: String,
    #[serde(rename = "CFBundleDisplayName")]
    cf_bundle_display_name: String,
}

impl fmt::Display for XcodeHostingInfoStrings {
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
pub struct KeyboardInfoPlistExtension {
    #[serde(rename = "NSExtensionAttributes")]
    ns_extension_attributes: LayoutInfoPlistExtensionAttributes,
    #[serde(rename = "NSExtensionPointIdentifier")]
    ns_extension_point_identifier: String,
    #[serde(rename = "NSExtensionPrincipalClass")]
    ns_extension_principal_class: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct KeyboardInfoPlist {
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
    ns_extension: KeyboardInfoPlistExtension,
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
    #[serde(rename = "CFBundleURLSchemes")]
    cf_bundle_url_schemes: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct HostingPlist {
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
    #[serde(rename = "CFBundleURLTypes")]
    cf_bundle_url_types: Vec<BundleSchemes>,
    #[serde(rename = "CFBundleVersion")]
    cf_bundle_version: String,
    #[serde(rename = "ITSAppUsesNonExemptEncryption")]
    its_app_uses_non_exempt_encryption: bool,
    #[serde(rename = "LSApplicationQueriesSchemes")]
    ls_application_queries_schemes: Vec<String>,
    #[serde(rename = "LSRequiresIPhoneOS")]
    ls_requires_iphone_os: bool,
    #[serde(rename = "UIBackgroundModes")]
    ui_background_modes: Vec<String>,
    #[serde(rename = "UILaunchStoryboardName")]
    ui_launch_storyboard_name: String,
    #[serde(rename = "UIRequiredDeviceCapabilities")]
    ui_required_device_capabilities: Vec<String>,
    #[serde(rename = "UISupportedInterfaceOrientations")]
    ui_supported_interface_orientations: Vec<String>,
    #[serde(rename = "UIUserInterfaceStyle")]
    ui_user_interface_style: String,
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

pub fn generate_keyboard_plist(
    template_path: PathBuf,
    value: IosKeyboardSettings,
    output_path: PathBuf,
) {
    let mut keyboard_plist: KeyboardInfoPlist =
        plist::from_file(template_path.clone()).expect("valid stuff");

    keyboard_plist.cf_bundle_display_name = value.display_name;
    keyboard_plist.cf_bundle_short_version_string = value.short_version;
    keyboard_plist.cf_bundle_version = value.build_version;
    keyboard_plist.ls_application_queries_schemes[0] = value.package_id;
    keyboard_plist
        .ns_extension
        .ns_extension_attributes
        .primary_language = value.primary_language;
    keyboard_plist.divvun_keyboard_index = value.keyboard_index;

    plist::to_file_xml(output_path, &keyboard_plist).unwrap();
}

pub fn generate_hosting_plist(in_out_path: PathBuf, value: IosKeyboardSettings) {
    let mut hosting_app_plist: HostingPlist =
        plist::from_file(in_out_path.clone()).expect("valid stuff");

    hosting_app_plist.cf_bundle_display_name = value.display_name;
    hosting_app_plist.cf_bundle_short_version_string = value.short_version;
    hosting_app_plist.cf_bundle_version = value.build_version;
    hosting_app_plist.cf_bundle_url_types[0].cf_bundle_url_schemes[0] = value.package_id.clone();
    hosting_app_plist.ls_application_queries_schemes[0] = value.package_id;

    plist::to_file_xml(in_out_path, &hosting_app_plist).unwrap();
}

pub fn update_entitlements(entitlements_path: PathBuf, new_entitlements: Vec<String>) {
    let mut keyboard_entitlements: EntitlementsDict =
        plist::from_file(entitlements_path.clone()).expect("valid stuff");
    keyboard_entitlements.com_apple_security_application_groups = new_entitlements;
    plist::to_file_xml(entitlements_path, &keyboard_entitlements).unwrap();
}

#[derive(Clone)]
pub struct IosKeyboardSettings {
    display_name: String,
    short_version: String,
    build_version: String,
    package_id: String,
    primary_language: String,
    keyboard_index: usize,
}

pub fn path_to_relative(path: &Path, relative_to: &str) -> PathBuf {
    let mut path_string = path.to_str().unwrap().to_string();
    path_string.replace_range(
        0..path_string.find(relative_to).unwrap() + relative_to.len() + 1,
        "",
    );
    return PathBuf::from_str(&path_string).unwrap();
}

pub struct GenerateXcode;

#[async_trait(?Send)]
impl BuildStep for GenerateXcode {
    async fn build(&self, bundle: &KbdgenBundle, output_path: &Path) {
        let repository_path = output_path.join(REPOSITORY);
        let hosting_app_path = repository_path.join(HOSTING_APP);
        let keyboard_path = repository_path.join(KEYBOARD);

        let xcodeproj_path = repository_path.join("GiellaKeyboard.xcodeproj");
        let pbxproj_path = xcodeproj_path.join("project.pbxproj");
        let mut pbxproj = Pbxproj::from_path(&pbxproj_path);

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
                        let info_path = locale_path.join(HOSTING_INFO_STRINGS);
                        println!("{:?}", locale_path);

                        std::fs::create_dir_all(&locale_path).unwrap();

                        let info_strings = XcodeHostingInfoStrings {
                            cf_bundle_name: locale_info.name.to_string(),
                            cf_bundle_display_name: locale_info.name.to_string(),
                        };
                        std::fs::write(info_path, info_strings.to_string()).unwrap();
                    }

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

                    let keyboard_plist_template = keyboard_path.join(INFO_PLIST);
                    let current_layout_path = keyboard_path.join(keyboard_folder_name.clone());

                    std::fs::create_dir_all(&current_layout_path).unwrap();

                    let ios_keyboard_settings = IosKeyboardSettings {
                        display_name: layout.autonym().to_string(),
                        short_version: target.version.clone(),
                        build_version: target.build.clone(),
                        package_id: target.package_id.clone(),
                        primary_language: language_tag.to_string(),
                        keyboard_index: layout_index,
                    };

                    // KEYBOARD PLIST
                    let layout_info_plist_path = current_layout_path.join(INFO_PLIST);
                    generate_keyboard_plist(
                        keyboard_plist_template,
                        ios_keyboard_settings.clone(),
                        layout_info_plist_path.clone(),
                    );

                    // GENERATE .pbxproj
                    let temp = pbxproj.create_plist_file(&PathBuf::from_str(INFO_PLIST).unwrap());
                    pbxproj.add_path(&path_to_relative(&current_layout_path, REPOSITORY));
                    pbxproj.add_ref_to_group(
                        &temp,
                        &path_to_relative(&current_layout_path, REPOSITORY),
                    );
                    pbxproj.duplicate_target(
                        KEYBOARD.to_string(),
                        keyboard_folder_name,
                        &path_to_relative(&layout_info_plist_path, REPOSITORY),
                    );

                    // HOSTING APP PLIST
                    let hosting_app_plist_path = hosting_app_path.join(INFO_PLIST);
                    generate_hosting_plist(hosting_app_plist_path, ios_keyboard_settings);

                    // NEW ENTITLEMENTS
                    let new_entitlements = format!("{}.{}", "group", target.package_id);

                    // UPDATE KEYBOARD ENTITLEMENTS
                    let keyboard_entitlements_path =
                        keyboard_path.join(format!("{}{}", KEYBOARD, ENTITLEMENTS_EXTENSION));
                    update_entitlements(keyboard_entitlements_path, vec![new_entitlements.clone()]);

                    // UPDATE HOSTING APP ENTITLEMENTS
                    let hosting_app_entitlements_path =
                        hosting_app_path.join(format!("{}{}", HOSTING_APP, ENTITLEMENTS_EXTENSION));
                    update_entitlements(
                        hosting_app_entitlements_path,
                        vec![new_entitlements.clone()],
                    );

                    // UPDATE ENTITLEMENTS IN SETTINGS BUNDLE PLIST
                    let root_plist_path = hosting_app_path.join(SETTINGS_BUNDLE).join(ROOT_PLIST);
                    let mut root_plist: SettingsRootDict =
                        plist::from_file(root_plist_path.clone()).expect("valid stuff");
                    root_plist.application_group_container_identifier = new_entitlements.clone();
                    plist::to_file_xml(root_plist_path.clone(), &root_plist).unwrap();
                }
            }
        }

        std::fs::write(
            pbxproj_path.clone(),
            serde_json::to_string_pretty(&pbxproj).unwrap(),
        )
        .unwrap();
    }
}
