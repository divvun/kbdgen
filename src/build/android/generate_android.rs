use std::collections::HashSet;
use std::fs::File;
use std::path::Path;
use std::process::Command;
use std::str::FromStr;

use anyhow::{Context, Result, bail};
use async_trait::async_trait;

use indexmap::IndexMap;
use language_tags::LanguageTag;

use qname::{QName, qname};
use serde::Serialize;
use url::Url;
use xmlem::{Document, NewElement, Node, Selector};

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
    // Each run removes only the first template subtype but appends one per
    // layout, so running twice on the same checkout duplicates subtypes.
    async fn build(&self, bundle: &KbdgenBundle, output_path: &Path) -> Result<()> {
        if !bundle
            .layouts
            .values()
            .any(|layout| layout.android.is_some())
        {
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
        let mut method_doc = read_without_first_subtype(&method_path)?;
        let spellchecker_path = main_xml_path.join("spellchecker.xml");
        let mut spellchecker_doc = read_without_first_subtype(&spellchecker_path)?;

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

            let subtype_name = format!(
                "subtype_{}",
                language_tag.as_str().replace('-', "_").to_lowercase()
            );

            create_and_write_values_strings(
                &main_values_path,
                default_display_name,
                &subtype_name,
            )?;

            for (locale, display_name) in &layout.display_names {
                if !supported_values_locales.contains(locale.as_str()) {
                    tracing::debug!(
                        "No res/values-{locale}; skipping the {locale} name of {language_tag}"
                    );
                    continue;
                }
                let strings_path = resources_path
                    .join(format!("values-{locale}"))
                    .join("strings.xml");
                let mut strings_doc = read_or_new_resources(&strings_path)?;
                let root = strings_doc.root();

                // [spec:kbdgen:req:android.metadata.strings]
                let exists = root.children(&strings_doc).into_iter().any(|child| {
                    child.name(&strings_doc) == "string"
                        && child.attribute(&strings_doc, "name") == Some(subtype_name.as_str())
                });
                if !exists {
                    let string = root.append_new_element(
                        &mut strings_doc,
                        ("string", [("name", subtype_name.as_str())]),
                    );
                    string.set_text(&mut strings_doc, display_name);
                }

                write_xml(&strings_path, &strings_doc)?;
            }

            append_method_subtype(&mut method_doc, language_tag, &keyboard_name, &subtype_name);

            // [spec:kbdgen:req:android.metadata+1]
            spellchecker_doc.root().append_new_element(
                &mut spellchecker_doc,
                (
                    "subtype",
                    [
                        ("android:label", format!("@string/{subtype_name}")),
                        ("android:subtypeLocale", language_tag.to_string()),
                    ],
                ),
            );
        }

        // [spec:kbdgen:req:android.metadata.strings]
        for (locale, LocaleProjectDescription { name, .. }) in &bundle.project.locales {
            if !supported_values_locales.contains(locale) {
                tracing::debug!("No res/values-{locale}; skipping the {locale} app name");
                continue;
            }
            let strings_appname_path = resources_path
                .join(format!("values-{locale}"))
                .join("strings-appname.xml");
            let mut strings_doc = read_or_new_resources(&strings_appname_path)?;
            let app_name = strings_doc
                .root()
                .append_new_element(&mut strings_doc, ("string", [("name", "english_ime_name")]));
            app_name.set_text(&mut strings_doc, name);
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

// [spec:kbdgen:req:android.metadata+1]
fn read_without_first_subtype(path: &Path) -> Result<Document> {
    let mut doc = read_xml(path)?;
    let selector = Selector::new("subtype").expect("subtype is a valid selector");
    let subtype = doc
        .root()
        .query_selector(&doc, &selector)
        .with_context(|| format!("{} has no <subtype>", path.display()))?;
    doc.root().remove_child(&mut doc, Node::Element(subtype));
    Ok(doc)
}

fn write_file(path: &Path, contents: impl AsRef<[u8]>) -> Result<()> {
    std::fs::write(path, contents).with_context(|| format!("write {}", path.display()))?;
    tracing::debug!("Wrote {}", path.display());
    Ok(())
}

fn write_xml(path: &Path, doc: &Document) -> Result<()> {
    write_file(path, doc.to_string_pretty())
}

// [spec:kbdgen:req:android.layout-xml.rows]
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

// [spec:kbdgen:req:android.metadata.icons+1]
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

        let output = Command::new("convert")
            .arg("-resize")
            .arg(format!("{size}x{size}"))
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

// [spec:kbdgen:req:android.metadata.strings]
fn create_and_write_values_strings(
    main_values_path: &Path,
    default_display_name: &str,
    subtype_name: &str,
) -> Result<()> {
    let strings_appname_path = main_values_path.join("strings-appname.xml");
    let mut strings_appname_doc = read_xml(&strings_appname_path)?;

    let ime_selector =
        Selector::new(r#"string[name="english_ime_name"]"#).expect("the IME name selector parses");
    let ime = strings_appname_doc
        .root()
        .query_selector(&strings_appname_doc, &ime_selector)
        .with_context(|| {
            format!(
                "{} has no string named english_ime_name",
                strings_appname_path.display()
            )
        })?;

    ime.set_text(
        &mut strings_appname_doc,
        &format!("{default_display_name} Keyboard"),
    );

    write_xml(&strings_appname_path, &strings_appname_doc)?;

    let strings_path = main_values_path.join("strings.xml");
    let mut strings_doc = read_xml(&strings_path)?;

    let subtype = strings_doc
        .root()
        .append_new_element(&mut strings_doc, ("string", [("name", subtype_name)]));
    subtype.set_text(&mut strings_doc, default_display_name);

    write_xml(&strings_path, &strings_doc)
}

// [spec:kbdgen:req:android.metadata+1]
fn append_method_subtype(
    method_doc: &mut Document,
    language_tag: &LanguageTag,
    keyboard_name: &str,
    subtype_name: &str,
) {
    method_doc.root().append_new_element(
        method_doc,
        (
            "subtype",
            [
                ("android:icon", "@drawable/ic_ime_switcher_dark".to_owned()),
                ("android:imeSubtypeMode", "keyboard".to_owned()),
                ("android:label", format!("@string/{subtype_name}")),
                (
                    "android:imeSubtypeLocale",
                    language_tag.as_str().replace('-', "_"),
                ),
                (
                    "android:imeSubtypeExtraValue",
                    format!("KeyboardLayoutSet={keyboard_name},AsciiCapable,EmojiCapable"),
                ),
                ("android:isAsciiCapable", "true".to_owned()),
            ],
        ),
    );
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

    // [spec:kbdgen:req:android.metadata+1/test]
    #[test]
    fn method_files_lose_their_first_subtype_and_must_have_one() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("method.xml");
        let subtype = Selector::new("subtype").unwrap();

        std::fs::write(
            &path,
            r#"<input-method><subtype id="1"/><subtype id="2"/></input-method>"#,
        )
        .unwrap();
        let doc = read_without_first_subtype(&path).unwrap();
        let left: Vec<_> = doc
            .root()
            .query_selector_all(&doc, &subtype)
            .into_iter()
            .map(|element| element.attribute(&doc, "id").unwrap().to_owned())
            .collect();
        assert_eq!(left, ["2"]);

        std::fs::write(&path, "<input-method/>").unwrap();
        assert_eq!(
            read_without_first_subtype(&path).unwrap_err().to_string(),
            format!("{} has no <subtype>", path.display())
        );

        let missing = dir.path().join("spellchecker.xml");
        assert_eq!(
            read_without_first_subtype(&missing)
                .unwrap_err()
                .to_string(),
            format!("open {}", missing.display())
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
