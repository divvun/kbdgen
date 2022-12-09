use serde::{Deserialize, Serialize};

#[derive(Debug, Default)]
pub struct Targets {
    pub windows: Option<Windows>,
    pub macos: Option<MacOS>,
    pub ios: Option<iOS>,
    pub chromeos: Option<ChromeOS>,
    pub android: Option<Android>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Windows {
    pub(crate) app_name: String,
    pub(crate) version: String,
    pub(crate) url: String,
    pub(crate) uuid: String,
    pub(crate) build: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MacOS {
    pub(crate) code_sign_id: String,
    pub(crate) package_id: String,
    pub(crate) bundle_name: String,
    pub(crate) version: String,
    pub(crate) build: String,
}

#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct iOS {
    #[serde(default)]
    pub(crate) code_sign_id: Option<String>,
    #[serde(default)]
    pub(crate) team_id: Option<String>,
    #[serde(default)]
    pub(crate) provisioning_profile_id: Option<String>,
    pub(crate) package_id: String,
    pub(crate) bundle_name: String,
    pub(crate) version: String,
    pub(crate) build: usize,

    #[serde(default)]
    /// https://docs.fastlane.tools/app-store-connect-api/#using-fastlane-api-key-json-file
    pub(crate) app_store_key_json: Option<String>,
    #[serde(default)]
    pub(crate) match_git_url: Option<String>,
    #[serde(default)]
    pub(crate) match_password: Option<String>,
    #[serde(default)]
    pub(crate) fastlane_user: Option<String>,
    #[serde(default)]
    pub(crate) fastlane_password: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChromeOS {
    pub(crate) app_id: String,
    pub(crate) build: String,
    pub(crate) version: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Android {
    #[serde(default)]
    pub(crate) key_store: Option<String>,
    #[serde(default)]
    pub(crate) key_alias: Option<String>,
    #[serde(default)]
    pub(crate) play_store_account: Option<String>,
    #[serde(default)]
    pub(crate) play_store_p12: Option<String>,
    #[serde(default)]
    pub(crate) store_password: Option<String>,
    #[serde(default)]
    pub(crate) key_password: Option<String>,
    pub(crate) package_id: String,
    pub(crate) build: usize,
    pub(crate) version: String,
}
