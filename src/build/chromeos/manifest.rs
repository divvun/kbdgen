use language_tags::LanguageTag;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct ManifestBackground {
    pub scripts: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ManifestInputComponent {
    pub name: String,
    #[serde(rename = "type")]
    pub input_type: String,
    pub id: String,
    pub description: String,
    pub language: String,
    pub layouts: Vec<String>,
}

impl ManifestInputComponent {
    pub fn from_config(language_tag: String, locale: LanguageTag, xkb_layout: String) -> Self {
        let underscore_name = format!("__MSG_{}__", language_tag.replace("-", "_"));
        Self {
            name: underscore_name.clone(),
            input_type: "ime".to_string(),
            id: language_tag.to_string(),
            description: underscore_name.clone(),
            language: locale.to_string(),
            layouts: vec![xkb_layout.to_string()], // should somehow be able to get more?
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ManifestIcons {
    #[serde(rename = "16")]
    pub icon_16: String,
    #[serde(rename = "48")]
    pub icon_48: String,
    #[serde(rename = "128")]
    pub icon_128: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ChromeOsManifest {
    pub name: String,
    pub version: String,
    pub version_name: String,
    pub manifest_version: u8,
    pub description: String,
    pub background: ManifestBackground,
    pub permissions: Vec<String>,
    pub input_components: Vec<ManifestInputComponent>,
    pub default_locale: String,
    pub icons: ManifestIcons,
}
