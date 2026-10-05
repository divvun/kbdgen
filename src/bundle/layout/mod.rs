use indexmap::IndexMap;
use language_tags::LanguageTag;
use serde::{Deserialize, Deserializer, Serialize};
use serde_yaml::Value;

use android::AndroidKbdLayer;
use chrome::ChromeOsKbdLayer;
use ios::IOsKbdLayer;
use macos::MacOsKbdLayer;
use windows::WindowsKbdLayer;

use crate::util::split_keys;

pub mod android;
pub mod chrome;
pub mod ios;
pub mod macos;
pub mod windows;

// [spec:kbdgen:def:layout.transforms]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum Transform {
    End(String),
    More(IndexMap<String, Transform>),
}

// [spec:kbdgen:def:layout.schema]
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Layout {
    pub language_tag: LanguageTag,

    pub display_names: IndexMap<LanguageTag, String>,

    pub decimal: Option<String>,

    pub windows: Option<WindowsTarget>,
    #[serde(rename = "chromeOS")]
    pub chrome_os: Option<ChromeOsTarget>,
    #[serde(rename = "macOS")]
    pub mac_os: Option<MacOsTarget>,
    #[serde(rename = "iOS")]
    pub i_os: Option<IOsTarget>,
    pub android: Option<AndroidTarget>,

    #[serde(default, deserialize_with = "from_mapped_sequence")]
    pub longpress: Option<IndexMap<String, Vec<String>>>,

    #[serde(default, deserialize_with = "from_nested_sequence")]
    pub transforms: Option<IndexMap<String, Transform>>,

    pub key_names: Option<KeyNames>,
}

impl Layout {
    pub fn autonym(&self) -> &str {
        let temp: LanguageTag = self.language_tag.primary_language().parse().unwrap();
        &self
            .display_names
            .get(&temp)
            .expect("autonym must be present")
    }
}

// [spec:kbdgen:req:layout.schema.targets]
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowsTarget {
    pub config: Option<WindowsConfig>,
    pub primary: WindowsPrimaryPlatform,
    pub dead_keys: Option<IndexMap<WindowsKbdLayer, Vec<String>>>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowsPrimaryPlatform {
    pub layers: IndexMap<WindowsKbdLayer, String>,
}

// [spec:kbdgen:req:layout.schema.targets]
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChromeOsTarget {
    pub config: Option<ChromeConfig>,
    pub primary: ChromeOsPrimaryPlatform,
    pub dead_keys: Option<IndexMap<ChromeOsKbdLayer, Vec<String>>>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChromeOsPrimaryPlatform {
    pub layers: IndexMap<ChromeOsKbdLayer, String>,
}

// [spec:kbdgen:req:layout.schema.targets]
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MacOsTarget {
    pub primary: MacOsPrimaryPlatform,
    pub dead_keys: Option<IndexMap<MacOsKbdLayer, Vec<String>>>,
    #[serde(default)]
    pub space: IndexMap<MacOsKbdLayer, String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MacOsPrimaryPlatform {
    pub layers: IndexMap<MacOsKbdLayer, String>,
}

// [spec:kbdgen:req:layout.schema.targets]
#[derive(Debug, Serialize, Deserialize)]
pub struct IOsTarget {
    #[serde(default)]
    pub config: IOsConfig,
    pub primary: Option<IOsPlatform>,
    #[serde(rename = "iPad-9in")]
    pub i_pad_9in: Option<IOsPlatform>,
    #[serde(rename = "iPad-12in")]
    pub i_pad_12in: Option<IOsPlatform>,
    #[serde(rename = "deadKeys")]
    pub dead_keys: Option<IndexMap<IOsKbdLayer, Vec<String>>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct IOsPlatform {
    pub layers: IndexMap<IOsKbdLayer, String>,
}

// [spec:kbdgen:req:layout.schema.targets]
#[derive(Debug, Serialize, Deserialize)]
pub struct AndroidTarget {
    pub config: Option<AndroidConfig>,
    pub primary: AndroidPlatform,
    #[serde(rename = "tablet-600")]
    pub tablet_600: AndroidPlatform,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AndroidPlatform {
    pub layers: IndexMap<AndroidKbdLayer, String>,
}

// [spec:kbdgen:def:layout.schema]
fn from_mapped_sequence<'de, D>(
    deserializer: D,
) -> Result<Option<IndexMap<String, Vec<String>>>, D::Error>
where
    D: Deserializer<'de>,
{
    let map: IndexMap<String, String> = Deserialize::deserialize(deserializer)?;

    Ok(Some(
        map.into_iter()
            .map(|(key, value)| (key, split_keys(&value)))
            .collect(),
    ))
}

fn from_nested_sequence<'de, D>(
    deserializer: D,
) -> Result<Option<IndexMap<String, Transform>>, D::Error>
where
    D: Deserializer<'de>,
{
    let mut output_map: IndexMap<String, Transform> = IndexMap::new();

    let transform_map: IndexMap<String, Value> = Deserialize::deserialize(deserializer)?;

    for (key, transform) in transform_map {
        let transform = process_transform(transform);

        output_map.insert(key.clone(), transform);
    }

    Ok(Some(output_map))
}

// [spec:kbdgen:def:layout.transforms]
fn process_transform(value: Value) -> Transform {
    match value {
        Value::String(character) => Transform::End(character),
        Value::Mapping(mapping) => {
            let mut output_map: IndexMap<String, Transform> = IndexMap::new();

            for (map_key, map_value) in mapping {
                let key_character = match map_key {
                    Value::String(key_character) => key_character,
                    _ => panic!("Only Strings are supported within map transforms!"),
                };

                let inner_transform = process_transform(map_value);
                output_map.insert(key_character, inner_transform);
            }

            Transform::More(output_map)
        }
        _ => panic!("Only Strings and Maps are supported within transforms!"),
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct KeyNames {
    pub space: String,
    pub r#return: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WindowsConfig {
    pub locale: Option<LanguageTag>,
    pub id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChromeConfig {
    pub locale: Option<LanguageTag>,
    pub xkb_layout: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IOsConfig {
    pub speller_package_key: Option<String>,
    pub speller_path: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AndroidConfig {
    pub speller_package_key: Option<String>,
    pub speller_path: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bundle::{Error, KbdgenBundle, fixture, read_kbdgen_bundle};

    const AUTONYM: &str = "displayNames:\n  se: Davvisámegiella\n";

    fn load(yaml: &str) -> Result<KbdgenBundle, Error> {
        let root = tempfile::tempdir().unwrap();
        let path = fixture::write_bundle(
            root.path(),
            "sme",
            &[("se", &format!("{AUTONYM}{yaml}"))],
            &[],
            &[],
        );
        read_kbdgen_bundle(&path)
    }

    fn layout(yaml: &str) -> Layout {
        let bundle = load(yaml).unwrap();
        bundle.layouts.0.into_values().next().unwrap()
    }

    fn assert_yaml_error(yaml: &str) {
        assert!(
            matches!(load(yaml), Err(Error::Yaml(..))),
            "expected a YAML error for:\n{yaml}"
        );
    }

    fn leaf(transform: &Transform) -> &str {
        match transform {
            Transform::End(output) => output,
            Transform::More(_) => panic!("expected a leaf, got {transform:?}"),
        }
    }

    fn branch(transform: &Transform) -> &IndexMap<String, Transform> {
        match transform {
            Transform::More(children) => children,
            Transform::End(_) => panic!("expected a branch, got {transform:?}"),
        }
    }

    // [spec:kbdgen:def:layout.schema/test]
    #[test]
    fn absent_optional_fields_stay_absent() {
        let layout = layout("unknownField: [1, 2]\n");

        assert_eq!(layout.language_tag.as_str(), "se");
        assert_eq!(layout.decimal, None);
        assert!(layout.windows.is_none());
        assert!(layout.chrome_os.is_none());
        assert!(layout.mac_os.is_none());
        assert!(layout.i_os.is_none());
        assert!(layout.android.is_none());
        assert!(layout.longpress.is_none());
        assert!(layout.transforms.is_none());
        assert!(layout.key_names.is_none());
    }

    // [spec:kbdgen:def:layout.schema/test]
    #[test]
    fn display_names_are_required_and_case_normalised() {
        let root = tempfile::tempdir().unwrap();
        let path =
            fixture::write_bundle(root.path(), "sme", &[("se", "decimal: \",\"\n")], &[], &[]);
        assert!(matches!(read_kbdgen_bundle(&path), Err(Error::Yaml(..))));

        let root = tempfile::tempdir().unwrap();
        let path = fixture::write_bundle(
            root.path(),
            "sme",
            &[(
                "se",
                "displayNames:\n  SE: Davvisámegiella\n  EN-gb: Northern Sami\n",
            )],
            &[],
            &[],
        );
        let bundle = read_kbdgen_bundle(&path).unwrap();
        let keys: Vec<String> = bundle
            .layouts
            .values()
            .next()
            .unwrap()
            .display_names
            .keys()
            .map(LanguageTag::to_string)
            .collect();
        assert_eq!(keys, ["se", "en-GB"]);
    }

    // [spec:kbdgen:def:layout.schema/test]
    #[test]
    fn unquoted_numbers_are_not_strings() {
        assert_yaml_error("decimal: 1\n");
        assert_yaml_error("keyNames:\n  space: 1\n  return: Enter\n");
        assert_yaml_error("longpress:\n  a: 1\n");
    }

    // [spec:kbdgen:def:layout.schema/test]
    #[test]
    fn key_names_require_space_and_return() {
        assert_yaml_error("keyNames:\n  space: Space\n");
        assert_yaml_error("keyNames:\n  return: Enter\n");

        let names = layout("keyNames:\n  space: Space\n  return: Enter\n")
            .key_names
            .unwrap();
        assert_eq!(
            (names.space.as_str(), names.r#return.as_str()),
            ("Space", "Enter")
        );
    }

    // [spec:kbdgen:def:layout.schema/test]
    #[test]
    fn longpress_splits_on_whitespace_in_order() {
        let longpress =
            layout("longpress:\n  o: \"ó  ò\\tô\\nö\"\n  a: á\n  e: \"\"\n  s: '\\u{20} ŝ'\n")
                .longpress
                .unwrap();

        let entries: Vec<(&str, Vec<&str>)> = longpress
            .iter()
            .map(|(k, v)| (k.as_str(), v.iter().map(String::as_str).collect()))
            .collect();
        assert_eq!(
            entries,
            [
                ("o", vec!["ó", "ò", "ô", "ö"]),
                ("a", vec!["á"]),
                ("e", vec![]),
                ("s", vec!["\\u{20}", "ŝ"]),
            ]
        );
    }

    // [spec:kbdgen:def:layout.schema/test]
    #[test]
    fn loader_keeps_escapes_verbatim() {
        let layout = layout(
            "keyNames:\n  space: '\\u{20}'\n  return: '\\u{21B5}'\nwindows:\n  primary:\n    layers:\n      default: '\\u{E1} b'\n  deadKeys:\n    default: ['\\u{B4}']\ntransforms:\n  '\\u{B4}':\n    a: '\\u{E1}'\n",
        );

        let names = layout.key_names.unwrap();
        assert_eq!(names.space, "\\u{20}");
        assert_eq!(names.r#return, "\\u{21B5}");
        let windows = layout.windows.unwrap();
        assert_eq!(
            windows.primary.layers[&windows::WindowsKbdLayer::Default],
            "\\u{E1} b"
        );
        assert_eq!(
            windows.dead_keys.unwrap()[&windows::WindowsKbdLayer::Default],
            ["\\u{B4}"]
        );
        let transforms = layout.transforms.unwrap();
        assert_eq!(leaf(&branch(&transforms["\\u{B4}"])["a"]), "\\u{E1}");
    }

    // [spec:kbdgen:req:layout.schema.targets/test]
    #[test]
    fn target_layers_keep_yaml_order() {
        let layout = layout(
            "windows:\n  config:\n    locale: se-fi\n    id: SE01\n  primary:\n    layers:\n      shift: B\n      default: b\n      caps+shift: c\n      alt+caps: d\n      ctrl: e\nmacOS:\n  primary:\n    layers:\n      cmd+alt+shift: x\n      default: y\n  space:\n    default: ' '\nchromeOS:\n  config:\n    xkbLayout: fi\n  primary:\n    layers:\n      alt+shift: z\n      default: w\niOS:\n  iPad-9in:\n    layers:\n      symbols-2: s\n      default: d\nandroid:\n  primary:\n    layers:\n      shift: Q\n      default: q\n  tablet-600:\n    layers:\n      default: q\n",
        );

        let windows = layout.windows.unwrap();
        let config = windows.config.unwrap();
        assert_eq!(config.locale.unwrap().as_str(), "se-FI");
        assert_eq!(config.id.as_deref(), Some("SE01"));
        use windows::WindowsKbdLayer as W;
        let windows_layers: Vec<_> = windows.primary.layers.keys().collect();
        assert_eq!(
            windows_layers,
            [
                &W::Shift,
                &W::Default,
                &W::CapsAndShift,
                &W::AltAndCaps,
                &W::Ctrl
            ]
        );
        assert!(windows.dead_keys.is_none());

        let mac_os = layout.mac_os.unwrap();
        assert_eq!(mac_os.primary.layers.len(), 2);
        assert_eq!(mac_os.primary.layers.values().next().unwrap(), "x");
        assert_eq!(mac_os.space.len(), 1);

        let chrome_os = layout.chrome_os.unwrap();
        assert_eq!(chrome_os.config.unwrap().xkb_layout.as_deref(), Some("fi"));
        assert_eq!(chrome_os.primary.layers.values().next().unwrap(), "z");

        let i_os = layout.i_os.unwrap();
        assert!(i_os.primary.is_none());
        assert!(i_os.i_pad_12in.is_none());
        assert!(i_os.config.speller_path.is_none());
        let ipad: Vec<String> = i_os.i_pad_9in.unwrap().layers.into_values().collect();
        assert_eq!(ipad, ["s", "d"]);

        let android = layout.android.unwrap();
        assert_eq!(android.primary.layers.values().next().unwrap(), "Q");
        assert_eq!(android.tablet_600.layers.len(), 1);
    }

    // [spec:kbdgen:req:layout.schema.targets/test]
    #[test]
    fn unknown_layer_names_are_errors() {
        for section in [
            "windows:\n  primary:\n    layers:\n      cmd: x\n",
            "chromeOS:\n  primary:\n    layers:\n      alt+caps: x\n",
            "macOS:\n  primary:\n    layers:\n      symbols-1: x\n",
            "iOS:\n  primary:\n    layers:\n      ctrl: x\n",
            "android:\n  primary:\n    layers:\n      caps: x\n  tablet-600:\n    layers:\n      default: x\n",
        ] {
            assert_yaml_error(section);
        }
    }

    // [spec:kbdgen:req:layout.schema.targets/test]
    #[test]
    fn required_platforms_must_be_present() {
        for section in [
            "windows:\n  config:\n    id: X\n",
            "chromeOS: {}\n",
            "macOS: {}\n",
            "android:\n  primary:\n    layers:\n      default: x\n",
            "android:\n  tablet-600:\n    layers:\n      default: x\n",
        ] {
            assert_yaml_error(section);
        }

        let i_os = layout("iOS: {}\n").i_os.unwrap();
        assert!(i_os.primary.is_none() && i_os.i_pad_9in.is_none() && i_os.i_pad_12in.is_none());
    }

    // [spec:kbdgen:def:layout.transforms/test]
    #[test]
    fn transforms_form_ordered_leaf_and_branch_tree() {
        let transforms = layout(
            "transforms:\n  '´':\n    e: é\n    ' ': '´'\n    a: á\n  '^': ê\n  '¨':\n    '~':\n      o: ṏ\n",
        )
        .transforms
        .unwrap();

        let keys: Vec<&str> = transforms.keys().map(String::as_str).collect();
        assert_eq!(keys, ["´", "^", "¨"]);
        let acute = branch(&transforms["´"]);
        let children: Vec<(&str, &str)> =
            acute.iter().map(|(k, v)| (k.as_str(), leaf(v))).collect();
        assert_eq!(children, [("e", "é"), (" ", "´"), ("a", "á")]);
        assert_eq!(leaf(&transforms["^"]), "ê");
        assert_eq!(leaf(&branch(&branch(&transforms["¨"])["~"])["o"]), "ṏ");
    }

    // [spec:kbdgen:def:layout.transforms/test]
    #[test]
    fn non_string_top_level_transform_key_is_error() {
        assert_yaml_error("transforms:\n  1:\n    a: b\n");
    }

    // [spec:kbdgen:def:layout.transforms/test]
    #[test]
    #[should_panic(expected = "Only Strings are supported within map transforms!")]
    fn non_string_nested_transform_key_panics() {
        let _ = load("transforms:\n  '´':\n    1: x\n");
    }

    // [spec:kbdgen:def:layout.transforms/test]
    #[test]
    fn non_string_transform_values_panic() {
        for value in ["1", "true", "~", "[a, b]"] {
            let yaml = format!("transforms:\n  '´':\n    a: {value}\n");
            let panic = std::panic::catch_unwind(|| load(&yaml)).unwrap_err();
            let message = panic
                .downcast_ref::<&str>()
                .copied()
                .or_else(|| panic.downcast_ref::<String>().map(String::as_str))
                .unwrap();
            assert_eq!(
                message, "Only Strings and Maps are supported within transforms!",
                "{value}"
            );
        }
    }
}
