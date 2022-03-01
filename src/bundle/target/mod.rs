use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub enum Target {
    Windows(Windows),
    MacOS(MacOS),
    iOS(iOS),
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Windows {
    app_name: String,
    version: String,
    url: String,
    uuid: String,
    build: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MacOS {}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct iOS {
    code_sign_id: String,
    provisioning_profile_id: String,
    package_id: String,
    bundle_name: String,
    version: String,
    build: String,
}