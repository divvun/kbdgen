use std::{fs, path::Path};

use anyhow::{Context, Result, bail};
use async_trait::async_trait;
use indexmap::{IndexMap, IndexSet};
use language_tags::LanguageTag;
use serde::Serialize;

use super::keymap::CHROMEOS_KEYS;
use super::manifest::{
    ChromeOsManifest, ManifestBackground, ManifestIcons, ManifestInputComponent, chrome_locale,
};
use crate::build::BuildStep;
use crate::bundle::KbdgenBundle;
use crate::bundle::layout::chrome::ChromeOsKbdLayer;
use crate::bundle::layout::{ChromeOsTarget, Layout};
use crate::bundle::project::LocaleProjectDescription;
use crate::bundle::target::ChromeOS;
use crate::util::split_keys;

const BACKGROUND_FILE_NAME: &str = "background.js";
const MANIFEST_FILE_NAME: &str = "manifest.json";
// [spec:kbdgen:req:chromeos.descriptor+1]
// [spec:kbdgen:sem:chromeos.descriptor.runtime]
const KEYBOARD_TEMPLATE: &str = include_str!("../../../resources/template-chromeos-keyboard.js");
const DEFAULT_LOCALE: &str = "en";

// [spec:kbdgen:req:chromeos.manifest+1]
const DEFAULT_LONG_LOCALE: &str = "en-US";
const DEFAULT_XKB_LAYOUT: &str = "us";

type ChromeOsLayout<'a> = (&'a LanguageTag, &'a Layout, &'a ChromeOsTarget);

// [spec:kbdgen:req:chromeos.manifest+1]
fn keyboard_word(language: &LanguageTag) -> &'static str {
    match language.as_str() {
        "nb" | "no" | "nn" | "da" => "tastatur",
        "sv" => "tangentbord",
        "fi" => "näppäimistö",
        _ => "keyboard",
    }
}

#[derive(Serialize)]
struct LocaleMessage {
    message: String,
}

/// The contents of `_locales/{language}/messages.json`.
#[derive(Serialize)]
struct Messages {
    #[serde(flatten)]
    layout_names: IndexMap<String, LocaleMessage>,
    name: LocaleMessage,
    description: LocaleMessage,
}

// [spec:kbdgen:req:chromeos.descriptor+1]
#[derive(Serialize)]
struct ChromeOsDescriptor {
    dead_keys: IndexMap<ChromeOsKbdLayer, Vec<String>>,
    transforms: IndexMap<String, IndexMap<String, String>>,
    layers: IndexMap<ChromeOsKbdLayer, IndexMap<&'static str, String>>,
}

fn write(path: &Path, contents: &str) -> Result<()> {
    fs::write(path, contents).with_context(|| format!("write {}", path.display()))?;
    tracing::debug!("Wrote {}", path.display());
    Ok(())
}

fn write_json(path: &Path, value: &impl Serialize) -> Result<()> {
    let json = serde_json::to_string_pretty(value)
        .with_context(|| format!("serialise {}", path.display()))?;
    write(path, &json)
}

// [spec:kbdgen:req:chromeos.descriptor+1]
// [spec:kbdgen:sem:chromeos.descriptor.runtime]
fn create_background(
    descriptor: &IndexMap<&LanguageTag, ChromeOsDescriptor>,
    output_path: &Path,
) -> Result<()> {
    let descriptor =
        serde_json::to_string_pretty(descriptor).context("serialise the descriptor")?;
    write(
        &output_path.join(BACKGROUND_FILE_NAME),
        &format!(
            "{KEYBOARD_TEMPLATE}\n\nconst descriptor = {descriptor}\n\nKeyboard.install(descriptor)"
        ),
    )
}

// [spec:kbdgen:req:chromeos.manifest+1]
fn create_manifest(
    target: &ChromeOS,
    input_components: Vec<ManifestInputComponent>,
    output_path: &Path,
) -> Result<()> {
    let manifest = ChromeOsManifest {
        name: "__MSG_name__",
        version: &target.build,
        version_name: &target.version,
        manifest_version: 2,
        description: "__MSG_description__",
        background: ManifestBackground {
            scripts: [BACKGROUND_FILE_NAME],
        },
        permissions: ["input"],
        input_components,
        default_locale: DEFAULT_LOCALE,
        icons: ManifestIcons {
            icon_16: "icon16.png",
            icon_48: "icon48.png",
            icon_128: "icon128.png",
        },
    };
    write_json(&output_path.join(MANIFEST_FILE_NAME), &manifest)
}

// [spec:kbdgen:req:chromeos.manifest+1]
fn create_locales(
    bundle: &KbdgenBundle,
    default_metadata: &LocaleProjectDescription,
    layouts: &[ChromeOsLayout],
    output_path: &Path,
) -> Result<()> {
    let locales_path = output_path.join("_locales");
    fs::create_dir_all(&locales_path)
        .with_context(|| format!("create {}", locales_path.display()))?;

    let languages: IndexSet<&LanguageTag> = layouts
        .iter()
        .flat_map(|(_, layout, _)| layout.display_names.keys())
        .collect();

    for language in languages {
        let metadata = bundle
            .project
            .locales
            .get(language.as_str())
            .unwrap_or(default_metadata);

        let layout_names = layouts
            .iter()
            .filter_map(|(tag, layout, _)| {
                let name = layout.display_names.get(language)?;
                let message = format!("{name} {}", keyboard_word(language));
                Some((chrome_locale(tag), LocaleMessage { message }))
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

        let locale_path = locales_path.join(chrome_locale(language));
        fs::create_dir_all(&locale_path)
            .with_context(|| format!("create {}", locale_path.display()))?;
        write_json(&locale_path.join("messages.json"), &messages)?;
    }

    Ok(())
}

// [spec:kbdgen:req:chromeos.descriptor+1]
fn generate_transforms(
    bundle: &KbdgenBundle,
    tag: &LanguageTag,
    layout: &Layout,
) -> Result<IndexMap<String, IndexMap<String, String>>> {
    if layout.transforms.is_none() {
        return Ok(IndexMap::new());
    }

    let path = bundle.path.join("layouts").join(format!("{tag}.yaml"));
    let yaml =
        fs::read_to_string(&path).with_context(|| format!("{tag}: read {}", path.display()))?;
    let mut yaml: serde_yaml::Mapping =
        serde_yaml::from_str(&yaml).with_context(|| format!("{tag}: parse {}", path.display()))?;
    let transforms = yaml
        .remove("transforms")
        .with_context(|| format!("{tag}: {} has no transforms", path.display()))?;

    serde_yaml::from_value(transforms).with_context(|| {
        format!("{tag}: chromeOS transforms must map each dead key to a map of strings")
    })
}

/// The layer's name as layout files spell it.
fn layer_name(layer: &ChromeOsKbdLayer) -> &'static str {
    match layer {
        ChromeOsKbdLayer::Default => "default",
        ChromeOsKbdLayer::Shift => "shift",
        ChromeOsKbdLayer::Caps => "caps",
        ChromeOsKbdLayer::CapsAndShift => "caps+shift",
        ChromeOsKbdLayer::Alt => "alt",
        ChromeOsKbdLayer::AltAndShift => "alt+shift",
        ChromeOsKbdLayer::Ctrl => "ctrl",
    }
}

// [spec:kbdgen:req:chromeos.descriptor+1]
fn generate_layers(
    tag: &LanguageTag,
    chromeos: &ChromeOsTarget,
) -> Result<IndexMap<ChromeOsKbdLayer, IndexMap<&'static str, String>>> {
    chromeos
        .primary
        .layers
        .iter()
        .map(|(layer, keys)| {
            let keys = split_keys(keys);

            // [spec:kbdgen:req:keys.iso-order.desktop-layers+1]
            if keys.len() < CHROMEOS_KEYS.len() {
                bail!(
                    "{tag}: chromeOS {} layer has {} keys but needs at least {}",
                    layer_name(layer),
                    keys.len(),
                    CHROMEOS_KEYS.len()
                );
            }

            let codes = CHROMEOS_KEYS.iter().map(|&(_, code)| code).zip(keys);
            Ok((layer.clone(), codes.collect()))
        })
        .collect()
}

pub struct GenerateChromeOs;

#[async_trait(?Send)]
impl BuildStep for GenerateChromeOs {
    async fn build(&self, bundle: &KbdgenBundle, output_path: &Path) -> Result<()> {
        let target = bundle
            .targets
            .chromeos
            .as_ref()
            .context("targets/chromeos.yaml is missing")?;
        let default_metadata = bundle
            .project
            .locales
            .get(DEFAULT_LOCALE)
            .with_context(|| format!("project.yaml: locales has no {DEFAULT_LOCALE} entry"))?;

        let layouts: Vec<ChromeOsLayout> = bundle
            .layouts
            .iter()
            .filter_map(|(tag, layout)| match &layout.chrome_os {
                Some(chromeos) => Some((tag, layout, chromeos)),
                None => {
                    tracing::debug!("Skipping {tag}: no chromeOS section");
                    None
                }
            })
            .collect();
        if layouts.is_empty() {
            bail!("no layout has a chromeOS section");
        }

        let mut descriptor = IndexMap::new();
        let mut input_components = Vec::new();
        for &(tag, layout, chromeos) in &layouts {
            tracing::info!("Generating {tag}");

            let config = chromeos.config.as_ref();
            let language = config
                .and_then(|config| config.locale.as_ref())
                .map_or(DEFAULT_LONG_LOCALE, LanguageTag::as_str);
            let xkb_layout = config
                .and_then(|config| config.xkb_layout.as_deref())
                .unwrap_or(DEFAULT_XKB_LAYOUT);
            input_components.push(ManifestInputComponent::new(tag, language, xkb_layout));

            descriptor.insert(
                tag,
                ChromeOsDescriptor {
                    // [spec:kbdgen:req:chromeos.descriptor+1]
                    dead_keys: chromeos.dead_keys.clone().unwrap_or_default(),
                    transforms: generate_transforms(bundle, tag, layout)?,
                    layers: generate_layers(tag, chromeos)?,
                },
            );
        }

        create_background(&descriptor, output_path)?;
        create_manifest(target, input_components, output_path)?;
        create_locales(bundle, default_metadata, &layouts, output_path)?;

        tracing::info!("Wrote the ChromeOS extension to {}", output_path.display());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    use serde_json::json;

    const KEYS_48: &str = "a b c d e f g h i j k l m n o p q r s t u v w x y z 1 2 3 4 5 6 7 8 9 0 A B C D E F G H I J K L";
    const TARGET: &str = "appId: abc\nbuild: \"7\"\nversion: 2.0.0\n";

    #[derive(Deserialize)]
    struct Descriptor {
        dead_keys: IndexMap<String, Vec<String>>,
        transforms: IndexMap<String, IndexMap<String, String>>,
        layers: IndexMap<String, IndexMap<String, String>>,
    }

    fn sma() -> String {
        format!(
            "displayNames:\n  sma: Åarjel\n  en: Southern Sami\n  sv: Sydsamiska\nchromeOS:\n  primary:\n    layers:\n      default: {KEYS_48} EXTRA\n"
        )
    }

    fn smj(shift: &str) -> String {
        format!(
            "displayNames:\n  smj: Julev\n  en: Lule Sami\n  nb: Lulesamisk\nchromeOS:\n  config:\n    locale: nb\n    xkbLayout: \"no\"\n  primary:\n    layers:\n      default: {KEYS_48}\n      shift: {shift}\n  deadKeys:\n    default: ['´']\ntransforms:\n  '´':\n    a: á\n    ' ': '´'\n"
        )
    }

    const FI: &str = "displayNames:\n  fi: Suomi\n  en: Finnish\n";

    async fn generate(
        root: &Path,
        layouts: &[(&str, &str)],
        targets: &[(&str, &str)],
    ) -> Result<std::path::PathBuf> {
        let path = crate::bundle::fixture::write_bundle(root, "test", layouts, targets, &[]);
        let bundle = crate::bundle::read_kbdgen_bundle(&path).unwrap();
        let out = root.join("out");
        fs::create_dir_all(&out).unwrap();
        GenerateChromeOs.build(&bundle, &out).await?;
        Ok(out)
    }

    fn read(path: &Path) -> String {
        fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
    }

    fn read_json(path: &Path) -> serde_json::Value {
        serde_json::from_str(&read(path)).unwrap()
    }

    // [spec:kbdgen:req:chromeos.descriptor+1/test]
    // [spec:kbdgen:req:chromeos.manifest+1/test]
    #[tokio::test]
    async fn writes_the_extension_for_chromeos_layouts() {
        let root = tempfile::tempdir().unwrap();
        let smj = smj(KEYS_48);
        let out = generate(
            root.path(),
            &[("fi", FI), ("sma", &sma()), ("smj", &smj)],
            &[("chromeos", TARGET)],
        )
        .await
        .unwrap();

        let background = read(&out.join("background.js"));
        let json = background
            .strip_prefix(KEYBOARD_TEMPLATE)
            .and_then(|rest| rest.strip_prefix("\n\nconst descriptor = "))
            .and_then(|rest| rest.strip_suffix("\n\nKeyboard.install(descriptor)"))
            .expect("background.js frames the descriptor with the template");
        let descriptor: IndexMap<String, Descriptor> = serde_json::from_str(json).unwrap();
        assert_eq!(descriptor.keys().collect::<Vec<_>>(), ["sma", "smj"]);

        let sma = &descriptor["sma"];
        assert!(sma.dead_keys.is_empty());
        assert!(sma.transforms.is_empty());
        let default = &sma.layers["default"];
        let codes: Vec<&str> = CHROMEOS_KEYS.iter().map(|&(_, code)| code).collect();
        assert_eq!(default.keys().collect::<Vec<_>>(), codes);
        assert_eq!(
            default.values().collect::<Vec<_>>(),
            KEYS_48.split(' ').collect::<Vec<_>>()
        );

        let smj = &descriptor["smj"];
        assert_eq!(smj.layers.keys().collect::<Vec<_>>(), ["default", "shift"]);
        assert_eq!(smj.dead_keys["default"], ["´"]);
        assert_eq!(smj.transforms["´"]["a"], "á");
        assert_eq!(smj.transforms["´"][" "], "´");

        assert_eq!(
            read_json(&out.join("manifest.json")),
            json!({
                "name": "__MSG_name__",
                "version": "7",
                "version_name": "2.0.0",
                "manifest_version": 2,
                "description": "__MSG_description__",
                "background": { "scripts": ["background.js"] },
                "permissions": ["input"],
                "input_components": [
                    {
                        "name": "__MSG_sma__",
                        "type": "ime",
                        "id": "sma",
                        "description": "__MSG_sma__",
                        "language": "en-US",
                        "layouts": ["us"],
                    },
                    {
                        "name": "__MSG_smj__",
                        "type": "ime",
                        "id": "smj",
                        "description": "__MSG_smj__",
                        "language": "nb",
                        "layouts": ["no"],
                    },
                ],
                "default_locale": "en",
                "icons": { "16": "icon16.png", "48": "icon48.png", "128": "icon128.png" },
            })
        );

        let locales = out.join("_locales");
        let test = json!({ "message": "Test" });
        assert_eq!(
            read_json(&locales.join("en/messages.json")),
            json!({
                "sma": { "message": "Southern Sami keyboard" },
                "smj": { "message": "Lule Sami keyboard" },
                "name": test,
                "description": test,
            })
        );
        assert_eq!(
            read_json(&locales.join("sv/messages.json")),
            json!({
                "sma": { "message": "Sydsamiska tangentbord" },
                "name": test,
                "description": test,
            })
        );
        assert_eq!(
            read_json(&locales.join("nb/messages.json")),
            json!({
                "smj": { "message": "Lulesamisk tastatur" },
                "name": test,
                "description": test,
            })
        );
        assert!(!locales.join("fi").exists());
    }

    #[tokio::test]
    async fn a_layer_with_too_few_keys_is_an_error() {
        let root = tempfile::tempdir().unwrap();
        let short = KEYS_48.rsplit_once(' ').unwrap().0;
        let error = generate(
            root.path(),
            &[("smj", &smj(short))],
            &[("chromeos", TARGET)],
        )
        .await
        .unwrap_err();

        assert_eq!(
            error.to_string(),
            "smj: chromeOS shift layer has 47 keys but needs at least 48"
        );
        assert!(!root.path().join("out/background.js").exists());
    }

    #[tokio::test]
    async fn a_missing_target_or_layout_is_an_error() {
        let root = tempfile::tempdir().unwrap();
        let error = generate(root.path(), &[("sma", &sma())], &[])
            .await
            .unwrap_err();
        assert_eq!(error.to_string(), "targets/chromeos.yaml is missing");
        assert!(!root.path().join("out/background.js").exists());

        let root = tempfile::tempdir().unwrap();
        let error = generate(root.path(), &[("fi", FI)], &[("chromeos", TARGET)])
            .await
            .unwrap_err();
        assert_eq!(error.to_string(), "no layout has a chromeOS section");
    }
}
