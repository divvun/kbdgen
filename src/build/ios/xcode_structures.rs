use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct XcodeHostingInfoStrings {
    #[serde(rename = "CFBundleName")]
    pub cf_bundle_name: String,
    #[serde(rename = "CFBundleDisplayName")]
    pub cf_bundle_display_name: String,
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
    pub is_ascii_capable: bool,
    #[serde(rename = "PrefersRightToLeft")]
    pub prefers_right_to_left: bool,
    #[serde(rename = "PrimaryLanguage")]
    pub primary_language: String,
    #[serde(rename = "RequestsOpenAccess")]
    pub requests_open_access: bool,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct KeyboardInfoPlistExtension {
    #[serde(rename = "NSExtensionAttributes")]
    pub ns_extension_attributes: LayoutInfoPlistExtensionAttributes,
    #[serde(rename = "NSExtensionPointIdentifier")]
    pub ns_extension_point_identifier: String,
    #[serde(rename = "NSExtensionPrincipalClass")]
    pub ns_extension_principal_class: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct KeyboardInfoPlist {
    #[serde(rename = "DivvunSpellerPath")]
    pub divvun_speller_path: Option<String>,
    #[serde(rename = "DivvunSpellerPackageKey")]
    pub divvun_speller_package_key: Option<String>,
    #[serde(rename = "DivvunContactEmail")]
    pub divvun_contact_email: String,
    #[serde(rename = "CFBundleDevelopmentRegion")]
    pub cf_bundle_development_region: String,
    #[serde(rename = "CFBundleDisplayName")]
    pub cf_bundle_display_name: String,
    #[serde(rename = "CFBundleExecutable")]
    pub cf_bundle_executable: String,
    #[serde(rename = "CFBundleIdentifier")]
    pub cf_bundle_identifier: String,
    #[serde(rename = "CFBundleInfoDictionaryVersion")]
    pub cf_bundle_info_dictionary_version: String,
    #[serde(rename = "CFBundleName")]
    pub cf_bundle_name: String,
    #[serde(rename = "CFBundlePackageType")]
    pub cf_bundle_package_type: String,
    #[serde(rename = "CFBundleShortVersionString")]
    pub cf_bundle_short_version_string: String,
    #[serde(rename = "CFBundleSignature")]
    pub cf_bundle_signature: String,
    #[serde(rename = "CFBundleVersion")]
    pub cf_bundle_version: String,
    #[serde(rename = "DivvunKeyboardIndex")]
    pub divvun_keyboard_index: usize,
    #[serde(rename = "ITSAppUsesNonExemptEncryption")]
    pub its_app_uses_non_exempt_encryption: bool,
    #[serde(rename = "LSApplicationQueriesSchemes")]
    pub ls_application_queries_schemes: Vec<String>,
    #[serde(rename = "NSExtension")]
    pub ns_extension: KeyboardInfoPlistExtension,
}

// LAYOUT PLIST END

#[derive(Serialize, Deserialize, Debug)]
pub struct EntitlementsDict {
    #[serde(rename = "com.apple.security.application-groups")]
    pub com_apple_security_application_groups: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PreferenceSpecifier {
    #[serde(rename = "Type")]
    pub preference_type: String,
    #[serde(rename = "Title")]
    pub title: String,
    #[serde(rename = "Key")]
    pub key: String,
    #[serde(rename = "DefaultValue")]
    pub default_value: bool,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SettingsRootDict {
    #[serde(rename = "StringsTable")]
    pub strings_table: String,
    #[serde(rename = "PreferenceSpecifiers")]
    pub preference_specifiers: Vec<PreferenceSpecifier>,
    #[serde(rename = "ApplicationGroupContainerIdentifier")]
    pub application_group_container_identifier: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BundleSchemes {
    #[serde(rename = "CFBundleURLSchemes")]
    pub cf_bundle_url_schemes: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct HostingPlist {
    #[serde(rename = "CFBundleDevelopmentRegion")]
    pub cf_bundle_development_region: String,
    #[serde(rename = "CFBundleDisplayName")]
    pub cf_bundle_display_name: String,
    #[serde(rename = "CFBundleExecutable")]
    pub cf_bundle_executable: String,
    #[serde(rename = "CFBundleIdentifier")]
    pub cf_bundle_identifier: String,
    #[serde(rename = "CFBundleInfoDictionaryVersion")]
    pub cf_bundle_info_dictionary_version: String,
    #[serde(rename = "CFBundleName")]
    pub cf_bundle_name: String,
    #[serde(rename = "CFBundlePackageType")]
    pub cf_bundle_package_type: String,
    #[serde(rename = "CFBundleShortVersionString")]
    pub cf_bundle_short_version_string: String,
    #[serde(rename = "CFBundleSignature")]
    pub cf_bundle_signature: String,
    #[serde(rename = "CFBundleURLTypes")]
    pub cf_bundle_url_types: Vec<BundleSchemes>,
    #[serde(rename = "CFBundleVersion")]
    pub cf_bundle_version: String,
    #[serde(rename = "ITSAppUsesNonExemptEncryption")]
    pub its_app_uses_non_exempt_encryption: bool,
    #[serde(rename = "LSApplicationQueriesSchemes")]
    pub ls_application_queries_schemes: Vec<String>,
    #[serde(rename = "LSRequiresIPhoneOS")]
    ls_requires_iphone_os: bool,
    #[serde(rename = "UIBackgroundModes")]
    pub ui_background_modes: Vec<String>,
    #[serde(rename = "UILaunchStoryboardName")]
    pub ui_launch_storyboard_name: String,
    #[serde(rename = "UIRequiredDeviceCapabilities")]
    pub ui_required_device_capabilities: Vec<String>,
    #[serde(rename = "UISupportedInterfaceOrientations")]
    pub ui_supported_interface_orientations: Vec<String>,
    #[serde(rename = "UIUserInterfaceStyle")]
    pub ui_user_interface_style: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct IconDefinition {
    pub idiom: String,
    pub size: String,
    pub scale: String,
    pub filename: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AppIconSetContents {
    pub images: Vec<IconDefinition>,
    pub info: serde_json::Value,
}
