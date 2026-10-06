//! The macOS generate step: every layout of the bundle with macOS output,
//! v3 and v4 side by side, written into one keyboard-layout bundle.

use std::path::Path;

use anyhow::{Context, Result, bail};
use async_trait::async_trait;
use language_tags::LanguageTag;

use super::input::KeylayoutInput;
use super::macos_bundle::MacOsBundle;
use super::{adapter, v3, writer};
use crate::build::BuildStep;
use crate::bundle::KbdgenBundle;

/// One layout's input and its `.keylayout` text.
#[derive(Debug, Clone)]
pub struct GeneratedLayout {
    pub input: KeylayoutInput,
    pub xml: String,
}

/// The input of one layout, or `None` when it has no macOS output: a v3
/// layout without a `macOS` section, or a v4 layout without a `macOS`
/// document.
// [spec:kbdgen:req:ldml.macos.target]
fn layout_input(bundle: &KbdgenBundle, tag: &LanguageTag) -> Result<Option<KeylayoutInput>> {
    if let Some(layout) = bundle.layouts.get(tag) {
        return Ok(v3::layout_input(tag, layout));
    }
    let Some((_, path)) = bundle.v4_layouts.iter().find(|(t, _)| t == tag) else {
        return Ok(None);
    };
    let Some(model) = adapter::load(tag, path)? else {
        return Ok(None);
    };
    Ok(Some(adapter::adapt(&model)?.input))
}

/// Every layout of the bundle with macOS output, v3 and v4 alike, in bundle
/// layout order. Two layouts with one keyboard name are fatal, since they
/// would write the same file.
// [spec:kbdgen:req:ldml.macos.target]
pub fn generate_bundle(bundle: &KbdgenBundle) -> Result<Vec<GeneratedLayout>> {
    let mut tags: Vec<&LanguageTag> = bundle
        .layouts
        .keys()
        .chain(bundle.v4_layouts.iter().map(|(tag, _)| tag))
        .collect();
    tags.sort_by(|a, b| a.as_str().cmp(b.as_str()));
    let mut generated: Vec<GeneratedLayout> = Vec::new();
    for tag in tags {
        let Some(input) =
            layout_input(bundle, tag).with_context(|| format!("macOS layout for {tag}"))?
        else {
            continue;
        };
        if generated.iter().any(|other| other.input.name == input.name) {
            bail!(
                "two macOS layouts are named {}; their .keylayout files would collide",
                input.name
            );
        }
        let xml = writer::write(&input);
        generated.push(GeneratedLayout { input, xml });
    }
    Ok(generated)
}

pub struct GenerateMacOs;

#[async_trait(?Send)]
impl BuildStep for GenerateMacOs {
    // [spec:kbdgen:req:ldml.macos.target]
    // [spec:kbdgen:req:macbundle.plist+1]
    async fn build(&self, bundle: &KbdgenBundle, output_path: &Path) -> Result<()> {
        let layouts = generate_bundle(bundle)?;
        let mut macos_bundle = MacOsBundle::new(output_path.to_path_buf(), bundle.name(), bundle)
            .expect("the bundle directory can be created");
        for layout in layouts {
            macos_bundle.add_key_layout(&layout.input, layout.xml);
        }
        macos_bundle.write_all()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEYS_48: &str = "a b c d e f g h i j k l m n o p q r s t u v w x y z 1 2 3 4 5 6 7 8 9 0 A B C D E F G H I J K L";

    fn read(path: &std::path::Path) -> String {
        std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
    }

    /// Generates the macOS bundle for a fixture whose layout files are
    /// written in a scrambled order and returns its `Contents` directory.
    async fn generate_ordered_bundle(out: &std::path::Path) -> std::path::PathBuf {
        let layout = |tag: &str, primary: &str, name: &str| {
            let name = format!("{name}-{tag}");
            let english = if primary == "en" {
                String::new()
            } else {
                format!("  en: {name} (en)\n")
            };
            format!(
                "displayNames:\n  {primary}: {name}\n{english}macOS:\n  primary:\n    layers:\n      default: {KEYS_48}\n"
            )
        };
        let layouts = [
            ("sme", layout("sme", "sme", "Sme")),
            ("se-FI", layout("se-FI", "se", "Se")),
            ("sma", layout("sma", "sma", "Sma")),
            ("se", layout("se", "se", "Se")),
            ("en", layout("en", "en", "En")),
        ];
        let layouts: Vec<(&str, &str)> = layouts.iter().map(|(t, y)| (*t, y.as_str())).collect();
        let fixture = tempfile::tempdir().unwrap();
        let path = crate::bundle::fixture::write_bundle(
            fixture.path(),
            "ordered",
            &layouts,
            &[(
                "macos",
                "codeSignId: X\npackageId: com.example\nbundleName: Ordered\nversion: 1.0.0\nbuild: \"1\"\n",
            )],
            &["macos"],
        );
        let bundle = crate::bundle::read_kbdgen_bundle(&path).unwrap();
        GenerateMacOs.build(&bundle, out).await.unwrap();
        out.join("com.example.keyboardlayout.ordered.bundle")
            .join("Contents")
    }

    // [spec:kbdgen:req:bundle.layouts+1/test]
    // [spec:kbdgen:req:macbundle.plist+1/test]
    // [spec:kbdgen:req:macbundle.plist.strings/test]
    #[tokio::test]
    async fn info_plist_and_strings_follow_bundle_layout_order() {
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        let contents = generate_ordered_bundle(first.path()).await;
        let again = generate_ordered_bundle(second.path()).await;

        let plist = read(&contents.join("Info.plist"));
        let positions: Vec<usize> = ["en", "se", "seFI", "sma", "sme"]
            .iter()
            .map(|name| {
                plist
                    .find(&format!("<key>KLInfo_{name}</key>"))
                    .unwrap_or_else(|| panic!("KLInfo_{name} missing from {plist}"))
            })
            .collect();
        assert!(positions.is_sorted(), "{plist}");
        assert!(plist.find("<key>CFBundleShortVersionString</key>").unwrap() < positions[0]);

        let strings = read(&contents.join("Resources/en.lproj/InfoPlist.strings"));
        assert_eq!(
            strings,
            [
                "\"en\" = \"En-en\";",
                "\"se\" = \"Se-se (en)\";",
                "\"seFI\" = \"Se-se-FI (en)\";",
                "\"sma\" = \"Sma-sma (en)\";",
                "\"sme\" = \"Sme-sme (en)\";",
            ]
            .join("\n")
        );

        for file in ["Info.plist", "Resources/en.lproj/InfoPlist.strings"] {
            assert_eq!(
                read(&contents.join(file)),
                read(&again.join(file)),
                "{file}"
            );
        }
    }
}
