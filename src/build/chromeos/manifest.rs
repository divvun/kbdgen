use language_tags::LanguageTag;
use serde::Serialize;

/// Chrome's `_locales` directory names and `__MSG_…__` message names
/// cannot contain `-`.
pub fn chrome_locale(tag: &LanguageTag) -> String {
    tag.as_str().replace('-', "_")
}

#[derive(Serialize)]
pub struct ManifestBackground {
    pub scripts: [&'static str; 1],
}

#[derive(Serialize)]
pub struct ManifestInputComponent<'a> {
    name: String,
    #[serde(rename = "type")]
    input_type: &'static str,
    id: &'a str,
    description: String,
    language: &'a str,
    layouts: [&'a str; 1],
}

// [spec:kbdgen:req:chromeos.manifest+1]
impl<'a> ManifestInputComponent<'a> {
    pub fn new(tag: &'a LanguageTag, language: &'a str, xkb_layout: &'a str) -> Self {
        let message = format!("__MSG_{}__", chrome_locale(tag));
        Self {
            name: message.clone(),
            input_type: "ime",
            id: tag.as_str(),
            description: message,
            language,
            layouts: [xkb_layout],
        }
    }
}

#[derive(Serialize)]
pub struct ManifestIcons {
    #[serde(rename = "16")]
    pub icon_16: &'static str,
    #[serde(rename = "48")]
    pub icon_48: &'static str,
    #[serde(rename = "128")]
    pub icon_128: &'static str,
}

// [spec:kbdgen:req:chromeos.manifest+1]
#[derive(Serialize)]
pub struct ChromeOsManifest<'a> {
    pub name: &'static str,
    pub version: &'a str,
    pub version_name: &'a str,
    pub manifest_version: u8,
    pub description: &'static str,
    pub background: ManifestBackground,
    pub permissions: [&'static str; 1],
    pub input_components: Vec<ManifestInputComponent<'a>>,
    pub default_locale: &'static str,
    pub icons: ManifestIcons,
}
