use std::collections::HashSet;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str::FromStr;

use anyhow::{Context, Result, bail};
use async_trait::async_trait;

use indexmap::{IndexMap, IndexSet};
use language_tags::LanguageTag;

use qname::{QName, qname};
use serde::Serialize;
use url::Url;
use xmlem::{Document, Element, NewElement, Selector};

use crate::bundle::layout::{AndroidConfig, Transform};
use crate::bundle::project::LocaleProjectDescription;
use crate::bundle::target;
use crate::{
    build::BuildStep,
    bundle::{KbdgenBundle, layout::android::AndroidKbdLayer},
    util::split_keys,
};

use super::{MAIN_FOLDER, REPOSITORY_FOLDER};

const ROWS_TEMPLATE: &str = include_str!("../../../resources/template-android-rows.xml");
const ROWKEYS_TEMPLATE: &str = include_str!("../../../resources/template-android-rowkeys.xml");

const ASSETS_LAYOUTS_PART: &str = "assets/layouts";
const RESOURCES_PART: &str = "res";
const MAIN_VALUES_PART: &str = "values";
const MAIN_XML_PART: &str = "xml";
const TABLET_600_XML_PART: &str = "xml-sw600dp";

const DEFAULT_LOCALE: &str = "en";

const DEFAULT_ROWKEYS_TAG: &str = "default";
const SHIFT_ROWKEYS_TAG: &str = "case";

const LONGPRESS_JOIN_CHARACTER: &str = ",";

// [spec:kbdgen:req:android.transforms+1]
#[derive(Debug, Default, Serialize)]
pub struct AndroidLayout {
    pub transforms: IndexMap<String, IndexMap<String, String>>,
    pub speller: Option<AndroidSpeller>,
}

#[derive(Debug, Serialize)]
pub struct AndroidSpeller {
    pub path: String,
    pub package_url: Option<Url>,
}

pub struct GenerateAndroid;

#[async_trait(?Send)]
impl BuildStep for GenerateAndroid {
    // `build` and `generate` reuse an existing checkout, which may hold an
    // earlier run's output: running this N times must leave the files that
    // running it once does.
    async fn build(&self, bundle: &KbdgenBundle, output_path: &Path) -> Result<()> {
        let has_android_layouts = bundle
            .layouts
            .values()
            .any(|layout| layout.android.is_some());
        if !has_android_layouts {
            tracing::warn!("No layout in the bundle has an android section");
        }

        let repo_path = output_path.join(REPOSITORY_FOLDER);
        let main_path = repo_path.join(MAIN_FOLDER);
        let assets_layouts_path = main_path.join(ASSETS_LAYOUTS_PART);
        let resources_path = main_path.join(RESOURCES_PART);
        let main_values_path = resources_path.join(MAIN_VALUES_PART);
        let main_xml_path = resources_path.join(MAIN_XML_PART);
        let tablet_600_xml_path = resources_path.join(TABLET_600_XML_PART);
        tracing::info!("Generating Android resources in {}", repo_path.display());

        let default_language_tag =
            LanguageTag::parse(DEFAULT_LOCALE).expect("default language tag must parse");

        for dir in [&assets_layouts_path, &main_xml_path, &tablet_600_xml_path] {
            std::fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;
        }

        let supported_values_locales = values_locales(&resources_path)?;

        let method_path = main_xml_path.join("method.xml");
        let mut method_doc = read_xml(&method_path)?;
        let previous_keyboards = strip_subtypes(
            &mut method_doc,
            has_android_layouts,
            generated_method_subtype,
        );
        let spellchecker_path = main_xml_path.join("spellchecker.xml");
        let mut spellchecker_doc = read_xml(&spellchecker_path)?;
        let previous_spellers = strip_subtypes(
            &mut spellchecker_doc,
            has_android_layouts,
            generated_spellchecker_subtype,
        );
        let previous_tags: IndexSet<&str> = previous_keyboards
            .iter()
            .map(|(tag, _)| tag.as_str())
            .chain(previous_spellers.iter().map(String::as_str))
            .collect();

        let mut generated_tags = HashSet::new();
        let mut generated_keyboards = HashSet::new();
        let mut subtype_strings: IndexMap<PathBuf, IndexMap<String, String>> = IndexMap::new();

        for (language_tag, layout) in &bundle.layouts {
            let transforms = layout
                .transforms
                .as_ref()
                .map(|transforms| flatten_transforms(language_tag, transforms))
                .unwrap_or_default();

            let Some(android_target) = &layout.android else {
                continue;
            };

            let default_display_name = layout
                .display_names
                .get(&default_language_tag)
                .with_context(|| {
                    format!("{language_tag}: displayNames has no '{DEFAULT_LOCALE}' entry")
                })?;
            let keyboard_name = keyboard_name(default_display_name);
            tracing::info!("Generating Android keyboard {keyboard_name} for {language_tag}");

            let assets_layout =
                assets_layout(language_tag, android_target.config.as_ref(), transforms)?;
            write_file(
                &assets_layouts_path.join(format!("{language_tag}.json")),
                serde_json::to_string_pretty(&assets_layout)?,
            )?;

            let dead_keys: HashSet<&str> = layout
                .transforms
                .iter()
                .flat_map(|transforms| transforms.keys())
                .map(String::as_str)
                .collect();

            // [spec:kbdgen:def:android.layout-xml]
            create_and_write_rows_keys_for_layer(
                false,
                language_tag,
                &android_target.primary.layers,
                layout.longpress.as_ref(),
                &keyboard_name,
                &main_xml_path,
                &dead_keys,
            )?;
            create_and_write_rows_keys_for_layer(
                true,
                language_tag,
                &android_target.tablet_600.layers,
                layout.longpress.as_ref(),
                &keyboard_name,
                &tablet_600_xml_path,
                &dead_keys,
            )?;

            create_and_write_kbd(&main_xml_path, &keyboard_name)?;
            create_and_write_layout_set(&main_xml_path, &keyboard_name)?;

            set_ime_name(&main_values_path, default_display_name)?;

            // [spec:kbdgen:req:android.metadata.strings+1]
            let subtype_name = subtype_name(language_tag.as_str());
            subtype_strings
                .entry(main_values_path.join("strings.xml"))
                .or_default()
                .insert(subtype_name.clone(), default_display_name.clone());
            for (locale, display_name) in &layout.display_names {
                if !supported_values_locales.contains(locale.as_str()) {
                    tracing::debug!(
                        "No res/values-{locale}; skipping the {locale} name of {language_tag}"
                    );
                    continue;
                }
                subtype_strings
                    .entry(
                        resources_path
                            .join(format!("values-{locale}"))
                            .join("strings.xml"),
                    )
                    .or_default()
                    .insert(subtype_name.clone(), display_name.clone());
            }

            // [spec:kbdgen:req:android.metadata+2]
            method_doc.root().append_new_element(
                &mut method_doc,
                method_subtype(language_tag.as_str(), &keyboard_name),
            );
            spellchecker_doc.root().append_new_element(
                &mut spellchecker_doc,
                spellchecker_subtype(language_tag.as_str()),
            );

            generated_tags.insert(language_tag.to_string());
            generated_keyboards.insert(keyboard_name);
        }

        // [spec:kbdgen:req:android.metadata+2]
        for &tag in &previous_tags {
            if !generated_tags.contains(tag) {
                tracing::info!(
                    "Removing {tag}: an earlier run generated it, and the bundle has no Android layout for it"
                );
                remove_file_if_exists(&assets_layouts_path.join(format!("{tag}.json")))?;
            }
        }
        for (tag, keyboard_name) in &previous_keyboards {
            if !generated_keyboards.contains(keyboard_name) {
                tracing::debug!(
                    "Removing keyboard {keyboard_name}, which an earlier run generated for {tag}"
                );
                remove_keyboard_files(&main_xml_path, &tablet_600_xml_path, keyboard_name)?;
            }
        }

        // [spec:kbdgen:req:android.metadata.strings+1]
        let owned_strings: HashSet<String> = previous_tags
            .iter()
            .copied()
            .chain(generated_tags.iter().map(String::as_str))
            .map(subtype_name)
            .collect();
        write_subtype_strings(
            &main_values_path,
            &resources_path,
            &supported_values_locales,
            &owned_strings,
            subtype_strings,
        )?;

        // [spec:kbdgen:req:android.metadata.strings+1]
        for (locale, LocaleProjectDescription { name, .. }) in &bundle.project.locales {
            if !supported_values_locales.contains(locale) {
                tracing::debug!("No res/values-{locale}; skipping the {locale} app name");
                continue;
            }
            let strings_appname_path = resources_path
                .join(format!("values-{locale}"))
                .join("strings-appname.xml");
            let mut strings_doc = read_or_new_resources(&strings_appname_path)?;
            set_string(&mut strings_doc, "english_ime_name", name);
            write_xml(&strings_appname_path, &strings_doc)?;
        }

        write_xml(&method_path, &method_doc)?;
        write_xml(&spellchecker_path, &spellchecker_doc)?;

        generate_icons(bundle, &resources_path)?;

        match &bundle.targets.android {
            Some(target) => {
                generate_gradle_local(target, &repo_path.join("app"))?;
                run_gradle(&repo_path)?;
            }
            None => tracing::warn!(
                "No targets/android.yaml; skipping local.gradle and the Gradle build"
            ),
        }

        Ok(())
    }
}

/// The `assets/layouts/{tag}.json` content. Without a speller config the
/// transforms are dropped.
// [spec:kbdgen:req:android.transforms+1]
fn assets_layout(
    language_tag: &LanguageTag,
    config: Option<&AndroidConfig>,
    transforms: IndexMap<String, IndexMap<String, String>>,
) -> Result<AndroidLayout> {
    let Some(config) = config else {
        return Ok(AndroidLayout::default());
    };
    let path = config
        .speller_path
        .clone()
        .with_context(|| format!("{language_tag}: android.config has no spellerPath"))?;
    let package_url = config
        .speller_package_key
        .as_deref()
        .map(|url| {
            Url::parse(url).with_context(|| {
                format!("{language_tag}: android.config.spellerPackageKey {url} is not a valid URL")
            })
        })
        .transpose()?;
    Ok(AndroidLayout {
        transforms,
        speller: Some(AndroidSpeller { path, package_url }),
    })
}

fn values_locales(resources_path: &Path) -> Result<HashSet<String>> {
    let entries = std::fs::read_dir(resources_path)
        .with_context(|| format!("read {}", resources_path.display()))?;
    Ok(entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            entry
                .file_name()
                .to_str()?
                .strip_prefix("values-")
                .map(str::to_owned)
        })
        .collect())
}

/// `{dead key: {next key: output}}` for a layout's transforms.
// [spec:kbdgen:req:android.transforms+1]
// [spec:kbdgen:req:layout.transforms.dead-key-entries+1]
fn flatten_transforms(
    language_tag: &LanguageTag,
    transforms: &IndexMap<String, Transform>,
) -> IndexMap<String, IndexMap<String, String>> {
    let mut transforms_by_dead_key = IndexMap::new();

    for (dead_key, transform) in transforms {
        let mut transforms_by_char = IndexMap::new();

        match transform {
            Transform::End(character) => {
                tracing::warn!(
                    "Transform ended too soon for dead key {dead_key} in {language_tag}: it outputs {character} with no following key"
                );
            }
            Transform::More(map) => {
                for (next_char, transform) in map {
                    match transform {
                        Transform::End(end_char) => {
                            transforms_by_char.insert(next_char.clone(), end_char.clone());
                        }
                        Transform::More(_transform) => {
                            todo!("Recursion required ahead");
                        }
                    }
                }
            }
        }

        transforms_by_dead_key.insert(dead_key.clone(), transforms_by_char);
    }

    transforms_by_dead_key
}

/// Android resource names allow only `[a-z0-9_]`.
// [spec:kbdgen:def:android.layout-xml.name+1]
fn keyboard_name(display_name: &str) -> String {
    display_name
        .to_lowercase()
        .chars()
        .filter(|c| !matches!(c, '(' | ')'))
        .map(|c| {
            if c.is_ascii_lowercase() || c.is_ascii_digit() {
                c
            } else {
                'x'
            }
        })
        .collect()
}

fn read_xml(path: &Path) -> Result<Document> {
    let file = File::open(path).with_context(|| format!("open {}", path.display()))?;
    Document::from_file(file).with_context(|| format!("parse {}", path.display()))
}

fn read_or_new_resources(path: &Path) -> Result<Document> {
    if path.is_file() {
        read_xml(path)
    } else {
        Ok(Document::new("resources"))
    }
}

/// Removes the `<subtype>` children of `doc`'s root that `generated`
/// recognises as kbdgen's, and returns what it made of them. The template's
/// first subtype is a placeholder for kbdgen's: an earlier run that
/// generated any subtype has already removed it, and with no layouts to
/// replace it (`replace_placeholder` false) it stays.
// [spec:kbdgen:req:android.metadata+2]
fn strip_subtypes<T>(
    doc: &mut Document,
    replace_placeholder: bool,
    generated: impl Fn(&Document, Element) -> Option<T>,
) -> Vec<T> {
    let root = doc.root();
    let subtypes: Vec<Element> = root
        .children(doc)
        .into_iter()
        .filter(|child| child.name(doc) == "subtype")
        .collect();

    let mut found = Vec::new();
    let mut template = Vec::new();
    for subtype in subtypes {
        match generated(doc, subtype) {
            Some(previous) => {
                found.push(previous);
                root.remove_child(doc, subtype.as_node());
            }
            None => template.push(subtype),
        }
    }

    if found.is_empty()
        && replace_placeholder
        && let Some(placeholder) = template.first()
    {
        root.remove_child(doc, placeholder.as_node());
    }
    found
}

/// `subtype_{sub}`, the string resource naming a layout's subtype.
fn subtype_name(language_tag: &str) -> String {
    format!("subtype_{}", language_tag.replace('-', "_").to_lowercase())
}

const METHOD_SUBTYPE_EXTRA_PREFIX: &str = "KeyboardLayoutSet=";
const METHOD_SUBTYPE_EXTRA_SUFFIX: &str = ",AsciiCapable,EmojiCapable";

// [spec:kbdgen:req:android.metadata+2]
fn method_subtype(language_tag: &str, keyboard_name: &str) -> NewElement {
    (
        "subtype",
        [
            ("android:icon", "@drawable/ic_ime_switcher_dark".to_owned()),
            ("android:imeSubtypeMode", "keyboard".to_owned()),
            (
                "android:label",
                format!("@string/{}", subtype_name(language_tag)),
            ),
            ("android:imeSubtypeLocale", language_tag.replace('-', "_")),
            (
                "android:imeSubtypeExtraValue",
                format!(
                    "{METHOD_SUBTYPE_EXTRA_PREFIX}{keyboard_name}{METHOD_SUBTYPE_EXTRA_SUFFIX}"
                ),
            ),
            ("android:isAsciiCapable", "true".to_owned()),
        ],
    )
        .into()
}

// [spec:kbdgen:req:android.metadata+2]
fn spellchecker_subtype(language_tag: &str) -> NewElement {
    (
        "subtype",
        [
            (
                "android:label",
                format!("@string/{}", subtype_name(language_tag)),
            ),
            ("android:subtypeLocale", language_tag.to_owned()),
        ],
    )
        .into()
}

/// The language tag and keyboard name of a `method.xml` subtype that
/// `method_subtype` wrote: one whose attributes are exactly what it writes
/// for the tag and name the subtype carries.
// [spec:kbdgen:req:android.metadata+2]
fn generated_method_subtype(doc: &Document, subtype: Element) -> Option<(String, String)> {
    // BCP 47 tags never contain `_`, so this undoes the `-`→`_` exactly.
    let language_tag = subtype
        .attribute(doc, "android:imeSubtypeLocale")?
        .replace('_', "-");
    let keyboard_name = subtype
        .attribute(doc, "android:imeSubtypeExtraValue")?
        .strip_prefix(METHOD_SUBTYPE_EXTRA_PREFIX)?
        .strip_suffix(METHOD_SUBTYPE_EXTRA_SUFFIX)?;
    (subtype.attributes(doc) == &method_subtype(&language_tag, keyboard_name).attrs)
        .then(|| (language_tag, keyboard_name.to_owned()))
}

/// The language tag of a `spellchecker.xml` subtype that
/// `spellchecker_subtype` wrote. The template's placeholder has the same
/// attributes, but its label is not the one derived from its locale.
// [spec:kbdgen:req:android.metadata+2]
fn generated_spellchecker_subtype(doc: &Document, subtype: Element) -> Option<String> {
    let language_tag = subtype.attribute(doc, "android:subtypeLocale")?;
    (subtype.attributes(doc) == &spellchecker_subtype(language_tag).attrs)
        .then(|| language_tag.to_owned())
}

/// Sets the text of the `<string name="{name}">` child of `doc`'s root,
/// appending one if there is none. Gradle rejects a resource defined twice,
/// so any further strings of that name, left by older kbdgen versions that
/// appended on every run, are removed.
// [spec:kbdgen:req:android.metadata.strings+1]
fn set_string(doc: &mut Document, name: &str, value: &str) {
    let root = doc.root();
    let existing = strings_named(doc, name);
    let string = match existing.split_first() {
        Some((&string, duplicates)) => {
            for duplicate in duplicates {
                root.remove_child(doc, duplicate.as_node());
            }
            string
        }
        None => root.append_new_element(doc, ("string", [("name", name)])),
    };
    string.set_text(doc, value);
}

fn strings_named(doc: &Document, name: &str) -> Vec<Element> {
    doc.root()
        .children(doc)
        .into_iter()
        .filter(|child| child.name(doc) == "string" && child.attribute(doc, "name") == Some(name))
        .collect()
}

/// Removes the `<string>` children of `doc`'s root whose name `remove`
/// accepts, and says whether there were any.
fn remove_strings(doc: &mut Document, remove: impl Fn(&str) -> bool) -> bool {
    let root = doc.root();
    let strings: Vec<Element> = root
        .children(doc)
        .into_iter()
        .filter(|child| {
            child.name(doc) == "string" && child.attribute(doc, "name").is_some_and(&remove)
        })
        .collect();
    for string in &strings {
        root.remove_child(doc, string.as_node());
    }
    !strings.is_empty()
}

/// Leaves `values/strings.xml` and every `values-*/strings.xml` with exactly
/// the `owned` strings that `wanted` gives the file, with those values. An
/// owned string not wanted in a file belongs to a layout or display name
/// that an earlier run had and the bundle does not. A file with nothing
/// wanted and nothing to remove is not rewritten, so keeps its formatting.
// [spec:kbdgen:req:android.metadata.strings+1]
fn write_subtype_strings(
    main_values_path: &Path,
    resources_path: &Path,
    locales: &HashSet<String>,
    owned: &HashSet<String>,
    wanted: IndexMap<PathBuf, IndexMap<String, String>>,
) -> Result<()> {
    let mut locales: Vec<&String> = locales.iter().collect();
    locales.sort();
    let every_strings_file = std::iter::once(main_values_path.join("strings.xml")).chain(
        locales.into_iter().map(|locale| {
            resources_path
                .join(format!("values-{locale}"))
                .join("strings.xml")
        }),
    );

    // `values-en/strings.xml` is a symlink to `values/strings.xml` in
    // giellakbd-android. Keying by the file a path resolves to keeps one
    // alias from removing what another wants.
    let mut files: IndexMap<PathBuf, (PathBuf, IndexMap<String, String>)> = IndexMap::new();
    for (path, strings) in wanted
        .into_iter()
        .chain(every_strings_file.map(|path| (path, IndexMap::new())))
    {
        let key = std::fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
        let (_, file_strings) = files.entry(key).or_insert_with(|| (path, IndexMap::new()));
        for (name, value) in strings {
            file_strings.entry(name).or_insert(value);
        }
    }

    for (path, strings) in files.into_values() {
        let text = if path.is_file() {
            Some(
                std::fs::read_to_string(&path)
                    .with_context(|| format!("read {}", path.display()))?,
            )
        } else {
            None
        };
        let mentions_owned = text
            .as_deref()
            .is_some_and(|text| owned.iter().any(|name| text.contains(name.as_str())));
        if strings.is_empty() && !mentions_owned {
            continue;
        }

        let mut doc = match &text {
            Some(text) => {
                Document::from_str(text).with_context(|| format!("parse {}", path.display()))?
            }
            None => Document::new("resources"),
        };
        let removed = remove_strings(&mut doc, |name| {
            owned.contains(name) && !strings.contains_key(name)
        });
        if strings.is_empty() && !removed {
            continue;
        }
        for (name, value) in &strings {
            set_string(&mut doc, name, value);
        }
        write_xml(&path, &doc)?;
    }
    Ok(())
}

/// Deletes the layout XML an earlier run wrote for `keyboard_name`.
// [spec:kbdgen:req:android.metadata+2]
fn remove_keyboard_files(
    main_xml_path: &Path,
    tablet_600_xml_path: &Path,
    keyboard_name: &str,
) -> Result<()> {
    remove_file_if_exists(&main_xml_path.join(format!("kbd_{keyboard_name}.xml")))?;
    remove_file_if_exists(&main_xml_path.join(format!("keyboard_layout_set_{keyboard_name}.xml")))?;
    for xml_path in [main_xml_path, tablet_600_xml_path] {
        remove_file_if_exists(&xml_path.join(format!("rows_{keyboard_name}.xml")))?;
        remove_rowkeys_from(xml_path, keyboard_name, 1)?;
    }
    Ok(())
}

/// Deletes `rowkeys_{name}{n}.xml` for `n` from `first` up to the first
/// number with no file; rows are numbered from 1 without gaps.
// [spec:kbdgen:req:android.layout-xml.rows+1]
fn remove_rowkeys_from(xml_path: &Path, keyboard_name: &str, first: usize) -> Result<()> {
    for line_number in first.. {
        let path = xml_path.join(format!("rowkeys_{keyboard_name}{line_number}.xml"));
        if !path.is_file() {
            return Ok(());
        }
        remove_file_if_exists(&path)?;
    }
    Ok(())
}

fn remove_file_if_exists(path: &Path) -> Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => {
            tracing::debug!("Removed {}", path.display());
            Ok(())
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err).with_context(|| format!("remove {}", path.display())),
    }
}

fn write_file(path: &Path, contents: impl AsRef<[u8]>) -> Result<()> {
    std::fs::write(path, contents).with_context(|| format!("write {}", path.display()))?;
    tracing::debug!("Wrote {}", path.display());
    Ok(())
}

fn write_xml(path: &Path, doc: &Document) -> Result<()> {
    write_file(path, doc.to_string_pretty())
}

// [spec:kbdgen:req:android.layout-xml.rows+1]
fn create_and_write_rows_keys_for_layer(
    tablet_600: bool,
    language_tag: &LanguageTag,
    layers: &IndexMap<AndroidKbdLayer, String>,
    longpress: Option<&IndexMap<String, Vec<String>>>,
    keyboard_name: &str,
    xml_path: &Path,
    dead_keys: &HashSet<&str>,
) -> Result<()> {
    let platform = if tablet_600 { "tablet-600" } else { "primary" };

    // keyboard-lut requires keys that appear lowercase when in shift mode.
    // If other keyboards require this in the future, it may be worth making
    // this an attribute of the .kbdgen yaml file.
    // [spec:kbdgen:req:android.keys+1]
    let preserve_case = language_tag.as_str() == "lut";

    let mut rows_document = Document::from_str(ROWS_TEMPLATE).expect("the rows template parses");
    let include_selector = Selector::new("include").expect("include is a valid selector");
    let rows_include = rows_document
        .root()
        .query_selector(&rows_document, &include_selector)
        .expect("the rows template has an <include>");

    let rowkeys_template =
        Document::from_str(ROWKEYS_TEMPLATE).expect("the rowkeys template parses");
    let mut rowkeys_docs = IndexMap::new();

    let default_layer = layers
        .get(&AndroidKbdLayer::Default)
        .with_context(|| format!("{language_tag}: android.{platform} has no default layer"))?;
    let longest_row_count = default_layer
        .lines()
        .map(|line| split_keys(line).len())
        .max()
        .unwrap_or(0);

    let width_base = if tablet_600 { 90.0 } else { 100.0 };
    let key_width = width_base / longest_row_count as f64;

    for (layer_key, layer) in layers {
        let (layer_name, tag) = match layer_key {
            AndroidKbdLayer::Default => ("default", DEFAULT_ROWKEYS_TAG),
            AndroidKbdLayer::Shift => ("shift", SHIFT_ROWKEYS_TAG),
        };
        let layer_selector = Selector::new(tag).expect("the rowkeys tags are valid selectors");

        for (line_index, line) in layer.lines().enumerate() {
            let rowkeys_doc = rowkeys_docs
                .entry(line_index)
                .or_insert_with(|| rowkeys_template.clone());
            let layer_element = rowkeys_doc
                .root()
                .query_selector(rowkeys_doc, &layer_selector)
                .unwrap_or_else(|| panic!("the rowkeys template has a <{tag}>"));

            let keys = split_keys(line);
            let special_keys_count = keys.iter().filter(|key| key.starts_with("\\s")).count();

            for (key_index, key) in keys.iter().enumerate() {
                let longpress = longpress.and_then(|longpress| longpress.get(key));
                let is_dead_key = dead_keys.contains(key.as_str());
                let element = if line_index == 0 {
                    create_numbered_key_xml_element(
                        key,
                        compute_key_hint_label_index(key_index),
                        longpress,
                        is_dead_key,
                        preserve_case,
                    )
                } else {
                    create_key_xml_element(
                        key,
                        longpress,
                        key_width,
                        keys.len(),
                        special_keys_count,
                        is_dead_key,
                        preserve_case,
                    )
                    .with_context(|| {
                        format!(
                            "{language_tag}: android.{platform}.layers.{layer_name}, line {}",
                            line_index + 1
                        )
                    })?
                };

                layer_element.append_new_element(rowkeys_doc, element);
            }
        }
    }

    let mut insert_after = rows_include;
    let line_count = rowkeys_docs.len();

    for (line_index, rowkeys_doc) in rowkeys_docs {
        let rowkeys_name = format!("rowkeys_{keyboard_name}{}", line_index + 1);

        insert_after = insert_after.append_new_element_after(&mut rows_document, "Row");
        insert_after.append_new_element(
            &mut rows_document,
            (
                "include",
                [
                    ("latin:keyboardLayout", format!("@xml/{rowkeys_name}")),
                    ("latin:keyWidth", format!("{key_width}%p")),
                ],
            ),
        );

        write_xml(&xml_path.join(format!("{rowkeys_name}.xml")), &rowkeys_doc)?;
    }
    remove_rowkeys_from(xml_path, keyboard_name, line_count + 1)?;

    write_xml(
        &xml_path.join(format!("rows_{keyboard_name}.xml")),
        &rows_document,
    )
}

fn escape_quotes(input: Option<&str>) -> String {
    input
        .map(|value| value.replace('"', "\\\""))
        .unwrap_or_default()
}

// [spec:kbdgen:req:android.gradle+1]
fn generate_gradle_local(target: &target::Android, app_path: &Path) -> Result<()> {
    let store_file = match target.key_store.as_deref() {
        Some(key_store) => match std::fs::canonicalize(key_store) {
            Ok(path) => path
                .to_str()
                .with_context(|| format!("keyStore {} is not valid UTF-8", path.display()))?
                .replace('\\', "\\\\"),
            Err(err) => {
                tracing::warn!(
                    "Leaving storeFile empty in local.gradle: cannot resolve keyStore {key_store}: {err}"
                );
                String::new()
            }
        },
        None => String::new(),
    };

    let key_alias = escape_quotes(target.key_alias.as_deref());
    let store_pw = escape_quotes(target.store_password.as_deref());
    let key_pw = escape_quotes(target.key_password.as_deref());
    let play_email = escape_quotes(target.play_store_account.as_deref());
    let play_creds = escape_quotes(target.play_store_p12.as_deref());
    let pkg_name = &target.package_id;
    let version = &target.version;
    let build = &target.build;

    let text = format!(
        r#"ext.app = [
    storeFile: "{store_file}",
    keyAlias: "{key_alias}",
    storePassword: "{store_pw}",
    keyPassword: "{key_pw}",
    packageName: "{pkg_name}",
    versionCode: {build},
    versionName: "{version}",
    playEmail: "{play_email}",
    playCredentials: "{play_creds}"
]"#
    )
    .replace('$', "\\$");

    write_file(&app_path.join("local.gradle"), text)
}

// [spec:kbdgen:req:android.gradle+1]
fn run_gradle(repo_path: &Path) -> Result<()> {
    let mut command = if cfg!(target_os = "windows") {
        let mut command = Command::new("cmd");
        command.args(["/C", "gradlew"]);
        command
    } else {
        let gradlew = repo_path.join("gradlew");
        Command::new(
            std::fs::canonicalize(&gradlew)
                .with_context(|| format!("resolve {}", gradlew.display()))?,
        )
    };
    command.current_dir(repo_path).args([
        "assembleRelease",
        "-Dorg.gradle.jvmargs=-Xmx4096M",
        "--info",
        "--stacktrace",
    ]);

    tracing::info!("Running gradlew assembleRelease in {}", repo_path.display());
    let output = command.output().context("gradlew could not be started")?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    tracing::trace!(
        "gradlew stdout:\n{}",
        String::from_utf8_lossy(&output.stdout)
    );
    tracing::trace!("gradlew stderr:\n{stderr}");

    if !output.status.success() {
        bail!(
            "`gradlew assembleRelease` failed with {}: {}",
            output.status,
            stderr.trim()
        );
    }
    Ok(())
}

// [spec:kbdgen:req:android.metadata.icons+2]
fn generate_icons(bundle: &KbdgenBundle, resources_path: &Path) -> Result<()> {
    const ICON_SIZES: &[(&str, usize)] = &[
        ("mdpi", 48),
        ("hdpi", 72),
        ("xhdpi", 96),
        ("xxhdpi", 144),
        ("xxxhdpi", 192),
    ];

    let Some(icon) = bundle
        .resources
        .android
        .as_ref()
        .and_then(|android| android.icon.as_ref())
    else {
        tracing::warn!("No resources/android/icon.png; keeping the app's launcher icon");
        return Ok(());
    };

    tracing::info!("Generating launcher icons from {}", icon.display());

    for (density, size) in ICON_SIZES {
        let output_path = resources_path
            .join(format!("drawable-{density}"))
            .join("ic_launcher_keyboard.png");

        // ImageMagick stamps PNGs with the time of writing, so without
        // this define every run would change all five icons.
        let output = Command::new("convert")
            .arg("-resize")
            .arg(format!("{size}x{size}"))
            .args(["-define", "png:exclude-chunks=date,time"])
            .arg(icon)
            .arg(&output_path)
            .output()
            .context("`convert` could not be started; install ImageMagick and make sure `convert` is on PATH")?;
        if !output.status.success() {
            bail!(
                "`convert` failed to resize {} to {size}x{size} with {}: {}",
                icon.display(),
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        tracing::debug!("Wrote {}", output_path.display());
    }

    Ok(())
}

// [spec:kbdgen:def:android.layout-xml]
fn create_and_write_kbd(main_xml_path: &Path, keyboard_name: &str) -> Result<()> {
    let mut kbd_document = Document::new("Keyboard");
    let kbd_root = kbd_document.root();

    kbd_root.set_attribute(
        &mut kbd_document,
        "xmlns:latin",
        "http://schemas.android.com/apk/res-auto",
    );

    kbd_root.append_new_element(
        &mut kbd_document,
        (
            "include",
            [("latin:keyboardLayout", format!("@xml/rows_{keyboard_name}"))],
        ),
    );

    write_xml(
        &main_xml_path.join(format!("kbd_{keyboard_name}.xml")),
        &kbd_document,
    )
}

// [spec:kbdgen:def:android.layout-xml]
fn create_and_write_layout_set(main_xml_path: &Path, keyboard_name: &str) -> Result<()> {
    let mut layout_set_document = Document::new("KeyboardLayoutSet");
    let layout_root = layout_set_document.root();

    layout_root.set_attribute(
        &mut layout_set_document,
        "xmlns:latin",
        "http://schemas.android.com/apk/res-auto",
    );

    let keyboard_ref = format!("@xml/kbd_{keyboard_name}");
    let keyboard = keyboard_ref.as_str();

    layout_root.append_new_element(
        &mut layout_set_document,
        (
            "Element",
            [
                ("latin:elementName", "alphabet"),
                ("latin:elementKeyboard", keyboard),
                ("latin:enableProximityCharsCorrection", "true"),
            ],
        ),
    );

    for (element_name, element_keyboard) in [
        ("alphabetAutomaticShifted", keyboard),
        ("alphabetManualShifted", keyboard),
        ("alphabetShiftLocked", keyboard),
        ("alphabetShiftLockShifted", keyboard),
        ("symbols", "@xml/kbd_symbols"),
        ("symbolsShifted", "@xml/kbd_symbols_shift"),
        ("phone", "@xml/kbd_phone"),
        ("phoneSymbols", "@xml/kbd_phone_symbols"),
        ("number", "@xml/kbd_number"),
    ] {
        layout_root.append_new_element(
            &mut layout_set_document,
            (
                "Element",
                [
                    ("latin:elementName", element_name),
                    ("latin:elementKeyboard", element_keyboard),
                ],
            ),
        );
    }

    write_xml(
        &main_xml_path.join(format!("keyboard_layout_set_{keyboard_name}.xml")),
        &layout_set_document,
    )
}

// [spec:kbdgen:req:android.metadata.strings+1]
fn set_ime_name(main_values_path: &Path, default_display_name: &str) -> Result<()> {
    let strings_appname_path = main_values_path.join("strings-appname.xml");
    let mut strings_appname_doc = read_xml(&strings_appname_path)?;

    if strings_named(&strings_appname_doc, "english_ime_name").is_empty() {
        bail!(
            "{} has no string named english_ime_name",
            strings_appname_path.display()
        );
    }
    set_string(
        &mut strings_appname_doc,
        "english_ime_name",
        &format!("{default_display_name} Keyboard"),
    );

    write_xml(&strings_appname_path, &strings_appname_doc)
}

// [spec:kbdgen:req:android.keys.number-row]
fn create_numbered_key_xml_element(
    key: &str,
    key_hint_label_index: Option<usize>,
    longpress: Option<&Vec<String>>,
    dead_key: bool,
    preserve_case: bool,
) -> NewElement {
    let mut attrs = IndexMap::new();

    if key == "\\s{shift}" {
        attrs.insert(qname!("latin:keyStyle"), "shiftKeyStyle".to_owned());
    } else if key == "\\s{backspace}" {
        attrs.insert(qname!("latin:keyStyle"), "deleteKeyStyle".to_owned());
    } else {
        add_common_key_attributes(&mut attrs, key, preserve_case);

        if let Some(key_hint_label_index) = key_hint_label_index {
            attrs.insert(
                qname!("latin:keyHintLabel"),
                key_hint_label_index.to_string(),
            );
            attrs.insert(
                qname!("latin:additionalMoreKeys"),
                key_hint_label_index.to_string(),
            );
        }

        if let Some(longpress) = longpress {
            attrs.insert(
                qname!("latin:moreKeys"),
                longpress.join(LONGPRESS_JOIN_CHARACTER),
            );
        }
    }

    if dead_key {
        attrs.insert(qname!("latin:deadKey"), "True".to_owned());
    }

    NewElement {
        name: qname!("Key"),
        attrs,
    }
}

// [spec:kbdgen:req:android.keys+1]
fn create_key_xml_element(
    key: &str,
    longpress: Option<&Vec<String>>,
    key_width: f64,
    keys_count: usize,
    special_keys_count: usize,
    dead_key: bool,
    preserve_case: bool,
) -> Result<NewElement> {
    let mut attrs = IndexMap::new();

    if key == "\\s{shift}" {
        attrs.insert(qname!("latin:keyStyle"), "shiftKeyStyle".to_owned());
        let normal_keys = keys_count - special_keys_count;
        let total_width = key_width * normal_keys as f64;
        let remaining_space = 100f64 - total_width;
        let fill_left = remaining_space / special_keys_count as f64;
        attrs.insert(qname!("latin:keyWidth"), format!("{fill_left:.2}%"));
    } else if key == "\\s{backspace}" {
        attrs.insert(qname!("latin:keyStyle"), "deleteKeyStyle".to_owned());
        attrs.insert(qname!("latin:keyWidth"), "fillRight".to_owned());
    } else {
        add_common_key_attributes(&mut attrs, key, preserve_case);

        if let Some(longpress) = longpress {
            let longpress_hint = longpress
                .first()
                .with_context(|| format!("the longpress entry for {key} is empty"))?;

            attrs.insert(qname!("latin:keyHintLabel"), longpress_hint.to_owned());
            attrs.insert(
                qname!("latin:moreKeys"),
                longpress.join(LONGPRESS_JOIN_CHARACTER),
            );
        }
    }

    if dead_key {
        attrs.insert(qname!("latin:deadKey"), "True".to_owned());
    }

    Ok(NewElement {
        name: qname!("Key"),
        attrs,
    })
}

// [spec:kbdgen:req:android.keys+1]
fn add_common_key_attributes(attrs: &mut IndexMap<QName, String>, key: &str, preserve_case: bool) {
    attrs.insert(qname!("latin:keySpec"), escape_key_spec(key));

    if preserve_case {
        attrs.insert(qname!("latin:keyLabelFlags"), "preserveCase".to_owned());
    }
}

// [spec:kbdgen:req:android.keys+1]
fn escape_key_spec(key: &str) -> String {
    match key {
        "\\" => "\\\\".to_owned(),
        _ => key.to_owned(),
    }
}

// [spec:kbdgen:req:android.keys.number-row]
fn compute_key_hint_label_index(key_index: usize) -> Option<usize> {
    (key_index <= 9).then_some((key_index + 1) % 10)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn tag(tag: &str) -> LanguageTag {
        LanguageTag::parse(tag).unwrap()
    }

    fn attrs(element: &NewElement) -> Vec<(&str, &str)> {
        element
            .attrs
            .iter()
            .map(|(name, value)| (name.prefixed_name(), value.as_str()))
            .collect()
    }

    // [spec:kbdgen:def:android.layout-xml.name+1/test]
    #[test]
    fn keyboard_name_keeps_only_lowercase_ascii_letters_and_digits() {
        assert_eq!(
            keyboard_name("Northern Sami (Norway)"),
            "northernxsamixnorway"
        );
        assert_eq!(keyboard_name("Lule-Sámi 2"), "lulexsxmix2");
        assert_eq!(keyboard_name("snake_case"), "snakexcase");
    }

    #[test]
    fn number_row_hints_run_one_to_nine_then_zero() {
        assert_eq!(compute_key_hint_label_index(0), Some(1));
        assert_eq!(compute_key_hint_label_index(8), Some(9));
        assert_eq!(compute_key_hint_label_index(9), Some(0));
        assert_eq!(compute_key_hint_label_index(10), None);
    }

    // [spec:kbdgen:req:android.keys+1/test]
    #[test]
    fn keys_carry_spec_hint_more_keys_and_dead_key() {
        let longpress = vec!["ä".to_owned(), "å".to_owned()];
        let key = create_key_xml_element("a", Some(&longpress), 10.0, 10, 0, true, false).unwrap();
        assert_eq!(
            attrs(&key),
            [
                ("latin:keySpec", "a"),
                ("latin:keyHintLabel", "ä"),
                ("latin:moreKeys", "ä,å"),
                ("latin:deadKey", "True"),
            ]
        );

        let backslash = create_key_xml_element("\\", None, 10.0, 10, 0, false, true).unwrap();
        assert_eq!(
            attrs(&backslash),
            [
                ("latin:keySpec", "\\\\"),
                ("latin:keyLabelFlags", "preserveCase"),
            ]
        );
    }

    // [spec:kbdgen:req:android.keys+1/test]
    #[test]
    fn an_empty_longpress_list_is_an_error_naming_where_the_key_is() {
        let layers = IndexMap::from([(AndroidKbdLayer::Default, "1 2\nd e\n".to_owned())]);
        let longpress = IndexMap::from([("d".to_owned(), Vec::new())]);
        let dir = tempfile::tempdir().unwrap();

        let error = create_and_write_rows_keys_for_layer(
            true,
            &tag("se"),
            &layers,
            Some(&longpress),
            "name",
            dir.path(),
            &HashSet::new(),
        )
        .unwrap_err();

        assert_eq!(
            format!("{error:#}"),
            "se: android.tablet-600.layers.default, line 2: the longpress entry for d is empty"
        );
    }

    // [spec:kbdgen:req:android.transforms+1/test]
    #[test]
    fn the_assets_layout_needs_a_speller_path_and_a_valid_package_url() {
        let config = |path: Option<&str>, url: Option<&str>| AndroidConfig {
            speller_path: path.map(str::to_owned),
            speller_package_key: url.map(str::to_owned),
        };
        let transforms = IndexMap::from([(
            "´".to_owned(),
            IndexMap::from([("a".to_owned(), "á".to_owned())]),
        )]);
        let json_of =
            |layout: Result<AndroidLayout>| serde_json::to_value(layout.unwrap()).unwrap();

        assert_eq!(
            json_of(assets_layout(
                &tag("se"),
                Some(&config(
                    Some("se.bhfst"),
                    Some("https://example.com/speller")
                )),
                transforms.clone(),
            )),
            json!({
                "transforms": {"´": {"a": "á"}},
                "speller": {"path": "se.bhfst", "package_url": "https://example.com/speller"},
            })
        );
        assert_eq!(
            json_of(assets_layout(
                &tag("se"),
                Some(&config(Some("se.bhfst"), None)),
                IndexMap::new(),
            )),
            json!({"transforms": {}, "speller": {"path": "se.bhfst", "package_url": null}})
        );
        assert_eq!(
            json_of(assets_layout(&tag("se"), None, transforms)),
            json!({"transforms": {}, "speller": null})
        );

        let no_path = assets_layout(&tag("se"), Some(&config(None, None)), IndexMap::new());
        assert_eq!(
            no_path.unwrap_err().to_string(),
            "se: android.config has no spellerPath"
        );
        let bad_url = assets_layout(
            &tag("se"),
            Some(&config(Some("se.bhfst"), Some("not a url"))),
            IndexMap::new(),
        );
        assert_eq!(
            bad_url.unwrap_err().to_string(),
            "se: android.config.spellerPackageKey not a url is not a valid URL"
        );
    }

    fn subtype_labels(doc: &Document) -> Vec<String> {
        doc.root()
            .children(doc)
            .into_iter()
            .filter(|child| child.name(doc) == "subtype")
            .map(|subtype| subtype.attribute(doc, "android:label").unwrap().to_owned())
            .collect()
    }

    fn string_texts(doc: &Document, name: &str) -> Vec<String> {
        strings_named(doc, name)
            .into_iter()
            .map(|string| {
                string
                    .child_nodes(doc)
                    .iter()
                    .filter_map(|node| node.as_text())
                    .map(|text| text.as_str(doc).trim().to_owned())
                    .collect()
            })
            .collect()
    }

    // [spec:kbdgen:req:android.metadata+2/test]
    #[test]
    fn only_the_first_run_removes_the_template_placeholder_subtype() {
        let method = include_str!("fixtures/res/xml/method.xml");
        let placeholder = "@string/subtype_no_language_qwerty";

        let mut doc = Document::from_str(method).unwrap();
        assert!(strip_subtypes(&mut doc, true, generated_method_subtype).is_empty());
        assert_eq!(subtype_labels(&doc), [] as [&str; 0]);

        let mut doc = Document::from_str(method).unwrap();
        assert!(strip_subtypes(&mut doc, false, generated_method_subtype).is_empty());
        assert_eq!(subtype_labels(&doc), [placeholder]);

        // A placeholder kept by a run without layouts, then generated
        // subtypes, one of them twice as older kbdgen versions left them.
        for subtype in [
            method_subtype("se", "northernxsami"),
            method_subtype("smj-SE", "lulexsami"),
            method_subtype("se", "northernxsami"),
        ] {
            doc.root().append_new_element(&mut doc, subtype);
        }
        assert_eq!(
            strip_subtypes(&mut doc, true, generated_method_subtype),
            [
                ("se".to_owned(), "northernxsami".to_owned()),
                ("smj-SE".to_owned(), "lulexsami".to_owned()),
                ("se".to_owned(), "northernxsami".to_owned()),
            ]
        );
        assert_eq!(subtype_labels(&doc), [placeholder]);

        let mut doc =
            Document::from_str(include_str!("fixtures/res/xml/spellchecker.xml")).unwrap();
        doc.root()
            .append_new_element(&mut doc, spellchecker_subtype("smj-SE"));
        assert_eq!(
            strip_subtypes(&mut doc, true, generated_spellchecker_subtype),
            ["smj-SE"]
        );
        assert_eq!(subtype_labels(&doc), ["@string/subtype_generic"]);

        let mut doc = Document::from_str("<input-method/>").unwrap();
        assert!(strip_subtypes(&mut doc, true, generated_method_subtype).is_empty());
    }

    // [spec:kbdgen:req:android.metadata+2/test]
    #[test]
    fn a_subtype_is_kbdgens_only_if_it_has_exactly_the_attributes_kbdgen_writes() {
        let generated = |xml: &str| {
            let doc = Document::from_str(&format!(
                r#"<input-method xmlns:android="http://schemas.android.com/apk/res/android">{xml}</input-method>"#
            ))
            .unwrap();
            let subtype = doc.root().children(&doc)[0];
            generated_method_subtype(&doc, subtype)
        };
        let attributes = |extra: &str| {
            format!(
                r#"<subtype android:icon="@drawable/ic_ime_switcher_dark" android:imeSubtypeMode="keyboard" android:label="@string/subtype_se_fi" android:imeSubtypeLocale="se_FI" android:imeSubtypeExtraValue="KeyboardLayoutSet=northernxsami,AsciiCapable,EmojiCapable" android:isAsciiCapable="true"{extra}/>"#
            )
        };

        assert_eq!(
            generated(&attributes("")),
            Some(("se-FI".to_owned(), "northernxsami".to_owned()))
        );
        assert_eq!(generated(&attributes(r#" android:subtypeId="0x1""#)), None);
        assert_eq!(
            generated(&attributes("").replace("subtype_se_fi", "subtype_se")),
            None
        );
        assert_eq!(
            generated(&attributes("").replace(",EmojiCapable", "")),
            None
        );
    }

    // [spec:kbdgen:req:android.metadata.strings+1/test]
    #[test]
    fn setting_a_string_replaces_it_and_drops_its_duplicates() {
        let mut doc = Document::from_str(
            r#"<resources><string name="a">old</string><string name="b">kept</string><string name="a">older</string></resources>"#,
        )
        .unwrap();

        set_string(&mut doc, "a", "new");
        set_string(&mut doc, "c", "added");

        assert_eq!(string_texts(&doc, "a"), ["new"]);
        assert_eq!(string_texts(&doc, "b"), ["kept"]);
        assert_eq!(string_texts(&doc, "c"), ["added"]);
        let names: Vec<_> = doc
            .root()
            .children(&doc)
            .into_iter()
            .map(|string| string.attribute(&doc, "name").unwrap().to_owned())
            .collect();
        assert_eq!(names, ["a", "b", "c"]);
    }

    fn android_layout(display_names: &[(&str, &str)], default_layer: &str) -> String {
        let mut yaml = String::from("displayNames:\n");
        for (tag, name) in display_names {
            yaml.push_str(&format!("  {tag}: {name}\n"));
        }
        yaml.push_str("android:\n  config:\n    spellerPath: speller.bhfst\n");
        for platform in ["primary", "tablet-600"] {
            yaml.push_str(&format!("  {platform}:\n    layers:\n      default: |\n"));
            for line in default_layer.lines() {
                yaml.push_str(&format!("        {line}\n"));
            }
        }
        yaml
    }

    /// Lays the fixture's trimmed giellakbd-android resources out under
    /// `{output}/repo`, with `values-en/strings.xml` a link to
    /// `values/strings.xml` as in the app.
    fn fixture_checkout(output_path: &Path) -> PathBuf {
        fn copy_dir(from: &Path, to: &Path) {
            std::fs::create_dir_all(to).unwrap();
            for entry in std::fs::read_dir(from).unwrap() {
                let entry = entry.unwrap();
                let target = to.join(entry.file_name());
                if entry.file_type().unwrap().is_dir() {
                    copy_dir(&entry.path(), &target);
                } else {
                    std::fs::copy(entry.path(), target).unwrap();
                }
            }
        }

        let resources_path = output_path
            .join(REPOSITORY_FOLDER)
            .join(MAIN_FOLDER)
            .join(RESOURCES_PART);
        copy_dir(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("src/build/android/fixtures/res"),
            &resources_path,
        );
        let values_en = resources_path.join("values-en");
        std::fs::create_dir_all(&values_en).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink("../values/strings.xml", values_en.join("strings.xml")).unwrap();
        resources_path
    }

    /// Every file under `dir` by relative path: its text, or for a symlink
    /// its target. With `normalize_xml`, XML files are parsed and printed
    /// again, so that only their content counts.
    fn snapshot(dir: &Path, normalize_xml: bool) -> std::collections::BTreeMap<PathBuf, String> {
        fn walk(
            root: &Path,
            dir: &Path,
            normalize_xml: bool,
            files: &mut std::collections::BTreeMap<PathBuf, String>,
        ) {
            for entry in std::fs::read_dir(dir).unwrap() {
                let path = entry.unwrap().path();
                let file_type = std::fs::symlink_metadata(&path).unwrap().file_type();
                let relative = path.strip_prefix(root).unwrap().to_owned();
                if file_type.is_symlink() {
                    let target = std::fs::read_link(&path).unwrap();
                    files.insert(relative, format!("-> {}", target.display()));
                } else if file_type.is_dir() {
                    walk(root, &path, normalize_xml, files);
                } else {
                    let text = std::fs::read_to_string(&path).unwrap();
                    let text = if normalize_xml && path.extension().is_some_and(|ext| ext == "xml")
                    {
                        Document::from_str(&text).unwrap().to_string_pretty()
                    } else {
                        text
                    };
                    files.insert(relative, text);
                }
            }
        }
        let mut files = std::collections::BTreeMap::new();
        walk(dir, dir, normalize_xml, &mut files);
        files
    }

    async fn generate(bundle_path: &Path, output_path: &Path) {
        let bundle = crate::bundle::read_kbdgen_bundle(bundle_path).unwrap();
        GenerateAndroid.build(&bundle, output_path).await.unwrap();
    }

    const THREE_ROWS: &str = "á š e r t y\na s d f g h\n\\s{shift} ž z č \\s{backspace}\n";
    const TWO_ROWS: &str = "á š e r t y\n\\s{shift} ž z č \\s{backspace}\n";

    // [spec:kbdgen:req:android.metadata+2/test]
    // [spec:kbdgen:req:android.metadata.strings+1/test]
    #[tokio::test]
    async fn generating_again_leaves_the_checkout_that_generating_once_does() {
        let root = tempfile::tempdir().unwrap();
        let bundle_path = crate::bundle::fixture::write_bundle(
            root.path(),
            "t",
            &[
                (
                    "se",
                    &android_layout(
                        &[
                            ("en", "Northern Sami"),
                            ("nb", "Nordsamisk"),
                            ("se", "Davvisámegiella"),
                        ],
                        THREE_ROWS,
                    ),
                ),
                (
                    "smj-SE",
                    &android_layout(
                        &[
                            ("en", "Lule Sami"),
                            ("nb", "Lulesamisk"),
                            ("smj", "Julevsámegiella"),
                        ],
                        THREE_ROWS,
                    ),
                ),
                ("fi", "displayNames:\n  fi: Suomi\n  en: Finnish\n"),
            ],
            &[],
            &[],
        );
        let output_path = root.path().join("out");
        let resources_path = fixture_checkout(&output_path);

        generate(&bundle_path, &output_path).await;
        let once = snapshot(&output_path, false);
        for _ in 0..2 {
            generate(&bundle_path, &output_path).await;
            assert_eq!(snapshot(&output_path, false), once);
        }

        let read = |relative: &str| read_xml(&resources_path.join(relative)).unwrap();
        assert_eq!(
            subtype_labels(&read("xml/method.xml")),
            ["@string/subtype_se", "@string/subtype_smj_se"]
        );
        assert_eq!(
            subtype_labels(&read("xml/spellchecker.xml")),
            ["@string/subtype_se", "@string/subtype_smj_se"]
        );
        let values = read("values/strings.xml");
        assert_eq!(string_texts(&values, "subtype_se"), ["Northern Sami"]);
        assert_eq!(string_texts(&values, "subtype_smj_se"), ["Lule Sami"]);
        let values_nb = read("values-nb/strings.xml");
        assert_eq!(string_texts(&values_nb, "subtype_se"), ["Nordsamisk"]);
        assert_eq!(string_texts(&values_nb, "subtype_smj_se"), ["Lulesamisk"]);
        assert_eq!(
            string_texts(&read("values/strings-appname.xml"), "english_ime_name"),
            ["Lule Sami Keyboard"]
        );
        assert_eq!(
            string_texts(&read("values-en/strings-appname.xml"), "english_ime_name"),
            ["Test"]
        );
        assert_eq!(
            std::fs::read_to_string(resources_path.join("values-fi/strings.xml")).unwrap(),
            include_str!("fixtures/res/values-fi/strings.xml"),
            "a strings file kbdgen has nothing for keeps its formatting"
        );
    }

    // [spec:kbdgen:req:android.metadata+2/test]
    // [spec:kbdgen:req:android.metadata.strings+1/test]
    // [spec:kbdgen:req:android.layout-xml.rows+1/test]
    #[tokio::test]
    async fn generating_a_changed_bundle_leaves_what_generating_it_once_does() {
        let root = tempfile::tempdir().unwrap();
        let write_bundle = |name: &str, layouts: &[(&str, &str)]| {
            crate::bundle::fixture::write_bundle(root.path(), name, layouts, &[], &[])
        };
        let before = write_bundle(
            "before",
            &[
                (
                    "se",
                    &android_layout(
                        &[
                            ("en", "Northern Sami"),
                            ("nb", "Nordsamisk"),
                            ("fi", "Pohjoissaame"),
                            ("se", "Davvisámegiella"),
                        ],
                        THREE_ROWS,
                    ),
                ),
                (
                    "smj-SE",
                    &android_layout(
                        &[
                            ("en", "Lule Sami"),
                            ("nb", "Lulesamisk"),
                            ("se", "Julevsámegiella"),
                            ("smj", "Julevsámegiella"),
                        ],
                        THREE_ROWS,
                    ),
                ),
            ],
        );
        let after = write_bundle(
            "after",
            &[(
                "se",
                &android_layout(
                    &[
                        ("en", "North Sami"),
                        ("nb", "Davvisamisk"),
                        ("se", "Davvisámegiella"),
                    ],
                    TWO_ROWS,
                ),
            )],
        );

        let regenerated = root.path().join("regenerated");
        fixture_checkout(&regenerated);
        generate(&before, &regenerated).await;
        generate(&after, &regenerated).await;

        let fresh = root.path().join("fresh");
        fixture_checkout(&fresh);
        generate(&after, &fresh).await;

        let regenerated_files = snapshot(&regenerated, true);
        assert_eq!(regenerated_files, snapshot(&fresh, true));
        let main = Path::new(REPOSITORY_FOLDER).join(MAIN_FOLDER);
        for removed in [
            "assets/layouts/smj-SE.json",
            "res/xml/kbd_lulexsami.xml",
            "res/xml/keyboard_layout_set_northernxsami.xml",
            "res/xml/rows_northernxsami.xml",
            "res/xml-sw600dp/rowkeys_lulexsami1.xml",
            "res/xml/rowkeys_northxsami3.xml",
        ] {
            assert!(
                !regenerated_files.contains_key(&main.join(removed)),
                "{removed} is left"
            );
        }
        assert!(regenerated_files.contains_key(&main.join("res/xml/rowkeys_northxsami2.xml")));
    }

    // [spec:kbdgen:req:android.metadata+2/test]
    #[tokio::test]
    async fn generating_without_a_checkout_fails_on_method_xml() {
        let root = tempfile::tempdir().unwrap();
        let bundle_path = crate::bundle::fixture::write_bundle(root.path(), "t", &[], &[], &[]);
        let output_path = root.path().join("out");
        let resources_path = output_path
            .join(REPOSITORY_FOLDER)
            .join(MAIN_FOLDER)
            .join(RESOURCES_PART);
        std::fs::create_dir_all(&resources_path).unwrap();

        let bundle = crate::bundle::read_kbdgen_bundle(&bundle_path).unwrap();
        let error = GenerateAndroid
            .build(&bundle, &output_path)
            .await
            .unwrap_err();
        assert_eq!(
            error.to_string(),
            format!("open {}", resources_path.join("xml/method.xml").display())
        );
    }

    // [spec:kbdgen:req:android.gradle+1/test]
    #[cfg(unix)]
    #[test]
    fn a_failing_gradlew_is_an_error_with_its_stderr() {
        use std::os::unix::fs::PermissionsExt;

        let repo = tempfile::tempdir().unwrap();
        let gradlew = repo.path().join("gradlew");
        assert_eq!(
            run_gradle(repo.path()).unwrap_err().to_string(),
            format!("resolve {}", gradlew.display())
        );

        let write_gradlew = |exit_code: u8| {
            std::fs::write(
                &gradlew,
                format!("#!/bin/sh\necho built\necho boom >&2\nexit {exit_code}\n"),
            )
            .unwrap();
            std::fs::set_permissions(&gradlew, std::fs::Permissions::from_mode(0o755)).unwrap();
        };

        write_gradlew(0);
        run_gradle(repo.path()).unwrap();

        write_gradlew(3);
        assert_eq!(
            run_gradle(repo.path()).unwrap_err().to_string(),
            "`gradlew assembleRelease` failed with exit status: 3: boom"
        );
    }
}
