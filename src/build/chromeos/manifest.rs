//! `manifest.json` and `_locales/*/messages.json` (`chromeos.manifest`,
//! `chromeos.locales`).

use anyhow::{Result, bail};
use indexmap::{IndexMap, IndexSet};
use language_tags::LanguageTag;
use serde::Serialize;

use super::source::ChromeOsLayout;
use crate::bundle::project::{LocaleProjectDescription, Project};

pub const DEFAULT_LOCALE: &str = "en";
pub const SERVICE_WORKER: &str = "background.js";
pub const ICON: &str = "icon.png";

/// The least Content Security Policy Chrome allows extension pages and
/// workers, plus `'wasm-unsafe-eval'`, without which a Manifest V3
/// extension cannot compile WebAssembly.
const EXTENSION_PAGES_CSP: &str = "script-src 'self' 'wasm-unsafe-eval'; object-src 'self';";

/// Chrome's `_locales` directory names and `__MSG_…__` message names
/// cannot contain `-`.
pub fn chrome_locale(tag: &LanguageTag) -> String {
    tag.as_str().replace('-', "_")
}

#[derive(Debug, Serialize)]
pub struct Background {
    service_worker: &'static str,
}

#[derive(Debug, Serialize)]
pub struct ContentSecurityPolicy {
    extension_pages: &'static str,
}

#[derive(Debug, Serialize)]
pub struct InputComponent {
    name: String,
    id: String,
    language: String,
    layouts: [String; 1],
}

#[derive(Debug, Serialize)]
pub struct Icons {
    #[serde(rename = "16")]
    icon_16: &'static str,
    #[serde(rename = "48")]
    icon_48: &'static str,
    #[serde(rename = "128")]
    icon_128: &'static str,
}

#[derive(Debug, Serialize)]
pub struct Manifest {
    manifest_version: u8,
    name: &'static str,
    description: &'static str,
    version: String,
    version_name: String,
    default_locale: &'static str,
    background: Background,
    permissions: [&'static str; 1],
    content_security_policy: ContentSecurityPolicy,
    input_components: Vec<InputComponent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    icons: Option<Icons>,
}

/// Whether `version` is a Chrome extension version: one to four
/// dot-separated integers from 0 to 65535, none but 0 starting with 0.
fn is_chrome_version(version: &str) -> bool {
    let parts: Vec<&str> = version.split('.').collect();
    (1..=4).contains(&parts.len())
        && parts.iter().all(|part| {
            !part.is_empty()
                && part.bytes().all(|b| b.is_ascii_digit())
                && (part == &"0" || !part.starts_with('0'))
                && part.parse::<u16>().is_ok()
        })
}

// [spec:kbdgen:req:chromeos.manifest+2]
pub fn manifest(
    build: &str,
    version: &str,
    layouts: &[ChromeOsLayout],
    icon: bool,
) -> Result<Manifest> {
    if !is_chrome_version(build) {
        bail!(
            "targets/chromeos.yaml: build {build:?} is not a Chrome extension version: one to four dot-separated integers from 0 to 65535, without leading zeros"
        );
    }
    let input_components = layouts
        .iter()
        .map(|layout| InputComponent {
            name: format!("__MSG_{}__", chrome_locale(&layout.tag)),
            id: layout.tag.to_string(),
            language: layout.language.clone(),
            layouts: [layout.xkb_layout.clone()],
        })
        .collect();
    Ok(Manifest {
        manifest_version: 3,
        name: "__MSG_name__",
        description: "__MSG_description__",
        version: build.to_owned(),
        version_name: version.to_owned(),
        default_locale: DEFAULT_LOCALE,
        background: Background {
            service_worker: SERVICE_WORKER,
        },
        permissions: ["input"],
        content_security_policy: ContentSecurityPolicy {
            extension_pages: EXTENSION_PAGES_CSP,
        },
        input_components,
        icons: icon.then_some(Icons {
            icon_16: ICON,
            icon_48: ICON,
            icon_128: ICON,
        }),
    })
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct LocaleMessage {
    message: String,
}

/// The contents of `_locales/{language}/messages.json`.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct Messages {
    #[serde(flatten)]
    layout_names: IndexMap<String, LocaleMessage>,
    name: LocaleMessage,
    description: LocaleMessage,
}

fn keyboard_word(language: &LanguageTag) -> &'static str {
    match language.as_str() {
        "nb" | "no" | "nn" | "da" => "tastatur",
        "sv" => "tangentbord",
        "fi" => "näppäimistö",
        _ => "keyboard",
    }
}

/// The name a layout has in `language`. Every message must exist in the
/// default locale, so there a layout without an English name falls back to
/// its autonym, then to its first name.
fn layout_name<'a>(layout: &'a ChromeOsLayout, language: &LanguageTag) -> Option<&'a String> {
    layout.display_names.get(language).or_else(|| {
        (language.as_str() == DEFAULT_LOCALE)
            .then(|| {
                layout
                    .display_names
                    .get(&layout.tag)
                    .or_else(|| layout.display_names.values().next())
            })
            .flatten()
    })
}

/// The messages of each locale, keyed by directory name: the default
/// locale, then every language a layout has a display name in, in layout
/// order.
// [spec:kbdgen:req:chromeos.locales]
pub fn locales(
    project: &Project,
    default: &LocaleProjectDescription,
    layouts: &[ChromeOsLayout],
) -> IndexMap<String, Messages> {
    let english = LanguageTag::parse(DEFAULT_LOCALE).ok();
    let languages: IndexSet<&LanguageTag> = english
        .iter()
        .chain(layouts.iter().flat_map(|l| l.display_names.keys()))
        .collect();
    languages
        .into_iter()
        .map(|language| {
            let metadata = project.locales.get(language.as_str()).unwrap_or(default);
            let layout_names = layouts
                .iter()
                .filter_map(|layout| {
                    let name = layout_name(layout, language)?;
                    let message = format!("{name} {}", keyboard_word(language));
                    Some((chrome_locale(&layout.tag), LocaleMessage { message }))
                })
                .collect();
            let messages = Messages {
                layout_names,
                name: LocaleMessage {
                    message: metadata.name.clone(),
                },
                description: LocaleMessage {
                    message: metadata.description.clone(),
                },
            };
            (chrome_locale(language), messages)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // [spec:kbdgen:req:chromeos.manifest+2/test]
    #[test]
    fn chrome_versions() {
        for good in ["0", "7", "1.2", "1.2.3.4", "65535.0.10"] {
            assert!(is_chrome_version(good), "{good}");
        }
        for bad in [
            "",
            "1.",
            ".1",
            "01",
            "1.02",
            "1.2.3.4.5",
            "65536",
            "1a",
            "-1",
            "1 2",
        ] {
            assert!(!is_chrome_version(bad), "{bad}");
        }
    }
}
