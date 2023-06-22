use std::collections::HashSet;
use std::process::{Command, Stdio};
use std::str::FromStr;
use std::{fs::File, path::Path};

use anyhow::Result;
use async_trait::async_trait;
use fs_extra::dir::CopyOptions;
use futures::stream::Select;
use indexmap::IndexMap;
use language_tags::LanguageTag;
use qname::qname;
use serde::Serialize;
use url::Url;
use xmlem::{Document, NewElement, Node, Selector};

use crate::build::pahkat;
use crate::bundle::project::{self, LocaleProjectDescription};
use crate::bundle::target;
use crate::{
    build::BuildStep,
    bundle::{layout::android::AndroidKbdLayer, KbdgenBundle},
    util::split_keys,
};

use super::REPOSITORY_FOLDER;

const ROWS_TEMPLATE: &str = include_str!("../../../resources/template-android-rows.xml");
const ROWKEYS_TEMPLATE: &str = include_str!("../../../resources/template-android-rowkeys.xml");

const TOP_FOLDER: &str = "app/src/main";
const ASSETS_LAYOUTS_PART: &str = "assets/layouts";
const RESOURCES_PART: &str = "res";
const MAIN_VALUES_PART: &str = "values";
const MAIN_XML_PART: &str = "xml";
const TABLET_600_XML_PART: &str = "xml-sw600dp";

const DEFAULT_LOCALE: &str = "en";

const DEFAULT_ROWKEYS_TAG: &str = "default";
const SHIFT_ROWKEYS_TAG: &str = "case";

const LONGPRESS_JOIN_CHARACTER: &str = ",";

const PRETTY_CONFIG: xmlem::display::Config = xmlem::display::Config {
    is_pretty: true,
    indent: 4,
    max_line_length: 120,
    entity_mode: xmlem::display::EntityMode::Standard,
    indent_text_nodes: false,
};

#[derive(Default, Serialize)]
pub struct AndroidLayout {
    pub transforms: IndexMap<String, String>,
    pub speller: Option<AndroidSpeller>,
}

#[derive(Serialize)]
pub struct AndroidSpeller {
    pub path: String,
    pub package_url: Url,
}

pub struct GenerateAndroid;

#[async_trait(?Send)]
impl BuildStep for GenerateAndroid {
    // The generator will currently create extra subtypes in some files
    // if ran more than once
    async fn build(&self, bundle: &KbdgenBundle, output_path: &Path) -> Result<()> {
        let mut android_targets = false;

        for (language_tag, layout) in &bundle.layouts {
            if let Some(android_target) = &layout.android {
                android_targets = true;
                break;
            }
        }

        if !android_targets {
            tracing::warn!("No Android targets found in the supplied kbdgen bundle!");
            return Ok(());
        }

        let output_path = output_path.join(Path::new(REPOSITORY_FOLDER));
        let top_path = output_path.join(Path::new(TOP_FOLDER));
        let assets_layouts_path = top_path.join(Path::new(ASSETS_LAYOUTS_PART));
        let resources_path = top_path.join(Path::new(RESOURCES_PART));

        let main_values_path = resources_path.join(Path::new(MAIN_VALUES_PART));

        let main_xml_path = resources_path.join(Path::new(MAIN_XML_PART));
        let tablet_600_xml_path = resources_path.join(Path::new(TABLET_600_XML_PART));

        let default_language_tag =
            LanguageTag::parse(DEFAULT_LOCALE).expect("default language tag must parse");

        std::fs::create_dir_all(&assets_layouts_path).unwrap();
        std::fs::create_dir_all(&main_xml_path).unwrap();
        std::fs::create_dir_all(&tablet_600_xml_path).unwrap();

        let supported_values_locales = std::fs::read_dir(&resources_path)
            .unwrap()
            .filter_map(Result::ok)
            .map(|dir_entry| {
                dir_entry
                    .path()
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_string()
            })
            .filter(|x| x.starts_with("values-"))
            .map(|x| x.replace("values-", ""))
            .collect::<HashSet<_>>();

        let subtype_selector = Selector::new("subtype").expect("subtype selector");

        // Method

        let method_path = main_xml_path.join(Path::new("method.xml"));
        let file = File::open(method_path.clone()).expect(&format!(
            "method.xml to exist in {:?} and open without issues",
            &main_xml_path
        ));

        let mut method_doc = Document::from_file(file).expect("can't read strings file");

        let subtype_selector = Selector::new("subtype").expect("subtype selector");

        let method_subtype = method_doc
            .root()
            .query_selector(&mut method_doc, &subtype_selector)
            .expect("there should be a subtype");

        method_doc
            .root()
            .remove_child(&mut method_doc, Node::Element(method_subtype));

        // Spellchecker

        let spellchecker_path = main_xml_path.join(Path::new("spellchecker.xml"));
        let file = File::open(spellchecker_path.clone()).expect(&format!(
            "spellchecker.xml to exist in {:?} and open without issues",
            &main_xml_path
        ));

        let mut spellchecker_doc = Document::from_file(file).expect("can't read strings file");

        let spellchecker_subtype = spellchecker_doc
            .root()
            .query_selector(&mut spellchecker_doc, &subtype_selector)
            .expect("there should be a subtype");

        spellchecker_doc
            .root()
            .remove_child(&mut spellchecker_doc, Node::Element(spellchecker_subtype));

        // One set of rowkeys_{displayName}{count}.xml file per language with an Android platform
        // x files for lines (should be 3)
        // (pretending we're following the primary approach for start)
        for (language_tag, layout) in &bundle.layouts {
            if let Some(android_target) = &layout.android {
                let assets_layout = if let Some(config) = android_target.config.as_ref() {
                    AndroidLayout {
                        transforms: IndexMap::new(), // should this be more? can mobile keys have transforms?
                        speller: Some(AndroidSpeller {
                            path: config
                                .speller_path
                                .as_ref()
                                .expect("no speller path supplid for android!")
                                .to_string(),
                            package_url: Url::parse(
                                &config
                                    .speller_package_key
                                    .as_ref()
                                    .expect("no speller package key provided for android!")
                                    .to_string(),
                            )
                            .expect("the speller package url to be parseable"),
                        }),
                    }
                } else {
                    Default::default()
                };

                std::fs::write(
                    assets_layouts_path.join(format!("{}.json", language_tag.to_string())),
                    serde_json::to_string_pretty(&assets_layout)
                        .expect("the generated assets layout to serialize correctly"),
                )
                .unwrap();

                let longpress = &layout.longpress;

                let default_display_name = layout
                    .display_names
                    .get(&default_language_tag)
                    .expect(&format!("no '{}' displayName!", DEFAULT_LOCALE));

                let snake_case_display_name = default_display_name
                    .to_lowercase()
                    .replace(" ", "_")
                    .replace("(", "")
                    .replace(")", "");

                let primary_layers = &android_target.primary.layers;
                let tablet_600_layers = &android_target.tablet_600.layers;

                create_and_write_rows_keys_for_layer(
                    false,
                    primary_layers,
                    longpress.as_ref(),
                    &default_display_name,
                    &snake_case_display_name,
                    &main_xml_path,
                );
                create_and_write_rows_keys_for_layer(
                    true,
                    tablet_600_layers,
                    longpress.as_ref(),
                    &default_display_name,
                    &snake_case_display_name,
                    &tablet_600_xml_path,
                );

                create_and_write_kbd(&main_xml_path, &snake_case_display_name);
                create_and_write_layout_set(&main_xml_path, &snake_case_display_name);

                let subtype_language_tag =
                    language_tag.to_string().replace("-", "_").to_lowercase();
                let current_language_tag_subtype = format!("subtype_{}", &subtype_language_tag);

                create_and_write_values_strings(
                    &main_values_path,
                    &default_display_name,
                    &current_language_tag_subtype,
                );

                for (language_tag, display_name) in &layout.display_names {
                    if !supported_values_locales.contains(&language_tag.to_string()) {
                        tracing::trace!("Skipping name strings for {}", language_tag);
                        continue;
                    }
                    let folder = resources_path
                        .join(Path::new(&format!("values-{}", language_tag)).to_owned());
                    let strings_path = folder.join(Path::new("strings.xml"));

                    let mut strings_doc;

                    if strings_path.is_file() {
                        let file = File::open(strings_path.clone()).expect(&format!(
                            "strings to exist in {:?} and open without issues",
                            &folder
                        ));
                        strings_doc = Document::from_file(file).expect("can't read strings file");
                    } else {
                        strings_doc = Document::new("resources");
                        std::fs::create_dir_all(&folder).unwrap();
                    }

                    let subtype = strings_doc.root().append_new_element(
                        &mut strings_doc,
                        NewElement {
                            name: qname!("string"),
                            attrs: [(qname!("name"), current_language_tag_subtype.clone())].into(),
                        },
                    );

                    subtype.set_text(&mut strings_doc, &display_name);

                    std::fs::write(
                        strings_path,
                        strings_doc.to_string_pretty_with_config(&PRETTY_CONFIG),
                    )
                    .unwrap();
                }

                update_method_file(
                    &main_xml_path,
                    &mut method_doc,
                    language_tag,
                    &snake_case_display_name,
                    &subtype_language_tag,
                );

                // Spellchecker
                let _subtype = spellchecker_doc.root().append_new_element(
                    &mut spellchecker_doc,
                    NewElement {
                        name: qname!("subtype"),
                        attrs: [
                            (
                                qname!("android:label"),
                                format!("@string/{current_language_tag_subtype}"),
                            ),
                            (qname!("android:subtypeLocale"), language_tag.to_string()),
                        ]
                        .into(),
                    },
                );
            }
        }

        for (locale, LocaleProjectDescription { name, .. }) in &bundle.project.locales {
            if !supported_values_locales.contains(&locale.to_string()) {
                tracing::trace!("Skipping locales for {}", locale);
                continue;
            }

            let folder = resources_path
                .join(Path::new(&format!("values-{}", locale.to_string())).to_owned());

            let strings_appname_path = folder.join(Path::new("strings-appname.xml"));

            let mut strings_doc;

            if strings_appname_path.is_file() {
                let file = File::open(strings_appname_path.clone()).expect(&format!(
                    "strings-appname to exist in {:?} and open without issues",
                    &folder
                ));
                strings_doc = Document::from_file(file).expect("can't read strings file");
            } else {
                strings_doc = Document::new("resources");
                std::fs::create_dir_all(&folder).unwrap();
            }

            let subtype = strings_doc.root().append_new_element(
                &mut strings_doc,
                NewElement {
                    name: qname!("string"),
                    attrs: [(qname!("name"), "english_ime_name".to_owned())].into(),
                },
            );

            subtype.set_text(&mut strings_doc, &name);

            std::fs::write(
                strings_appname_path,
                strings_doc.to_string_pretty_with_config(&PRETTY_CONFIG),
            )
            .unwrap();
        }

        std::fs::write(
            method_path,
            method_doc.to_string_pretty_with_config(&PRETTY_CONFIG),
        )
        .unwrap();
        std::fs::write(
            spellchecker_path,
            spellchecker_doc.to_string_pretty_with_config(&PRETTY_CONFIG),
        )
        .unwrap();

        let pahkat_dir = pahkat::prefix_dir("android").join("pkg");
        let libpahkat_client_path = pahkat_dir.join("libpahkat_client").join("lib");
        let libdivvunspell_path = pahkat_dir.join("libdivvunspell").join("lib");

        let jni_libs_path = top_path.join("jniLibs");
        std::fs::create_dir_all(&jni_libs_path).expect("failed to make jniLibs directory");

        dircpy::copy_dir(libpahkat_client_path, &jni_libs_path)
            .expect("failed to copy libpahkat_client from Pahkat repo");
        dircpy::copy_dir(libdivvunspell_path, &jni_libs_path)
            .expect("failed to copy libdivvunspell from Pahkat repo");

        generate_icons(bundle, &resources_path);
        if let Some(target) = bundle.targets.android.as_ref() {
            generate_gradle_local(target, &output_path.join("app"));

            let gradle_executable_path = std::fs::canonicalize(&output_path.join("gradlew"))
                .expect("valid gradle executable path");

            let gradle_assemble = if cfg!(target_os = "windows") {
                Command::new("cmd")
                    .current_dir(output_path)
                    .arg("/C")
                    .arg("gradlew")
                    .arg("assembleRelease")
                    .arg("-Dorg.gradle.jvmargs=-Xmx4096M")
                    .arg("--info")
                    .arg("--stacktrace")
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .output()
                    .expect("failed to build android project")
            } else {
                Command::new(gradle_executable_path)
                    .current_dir(output_path)
                    .arg("assembleRelease")
                    .arg("-Dorg.gradle.jvmargs=-Xmx4096M")
                    .arg("--info")
                    .arg("--stacktrace")
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .output()
                    .expect("failed to build android project")
            };

            let stdout = String::from_utf8(gradle_assemble.stdout).unwrap();
            let stderr = String::from_utf8(gradle_assemble.stderr).unwrap();

            println!("out {}", stdout);
            println!("err {}", stderr);
        } else {
            tracing::warn!("No target configuration found; no package identifier set.");
        }

        Ok(())
    }
}

fn create_and_write_rows_keys_for_layer(
    tablet_600: bool,
    layers: &IndexMap<AndroidKbdLayer, String>,
    longpress: Option<&IndexMap<String, Vec<String>>>,
    default_display_name: &str,
    snake_case_display_name: &str,
    xml_path: &Path,
) {
    let mut rows_document = Document::from_str(ROWS_TEMPLATE).expect("invalid rows template");

    let include_selector = Selector::new("include").expect("this selector is fine");

    let rows_include = rows_document
        .root()
        .query_selector(&mut rows_document, &include_selector)
        .expect("there should be an include");

    let rowkeys_document = Document::from_str(ROWKEYS_TEMPLATE).expect("invalid rowkeys template");

    let mut rowkeys_docs_map = IndexMap::new();

    let default_layer = layers.get(&AndroidKbdLayer::Default).unwrap();
    let longest_row_count = default_layer
        .split("\n")
        .map(|line| split_keys(line).len())
        .max()
        .unwrap();

    let key_width = if tablet_600 {
        100.0f64 / longest_row_count as f64
    } else {
        90.0f64 / longest_row_count as f64
    };

    for (layer_key, layer) in layers {
        let selector_string;

        match layer_key {
            AndroidKbdLayer::Default => {
                selector_string = DEFAULT_ROWKEYS_TAG;
            }
            AndroidKbdLayer::Shift => {
                selector_string = SHIFT_ROWKEYS_TAG;
            }
        };

        for (line_index, line) in layer.lines().enumerate() {
            let mut new_rowkeys_document = rowkeys_docs_map
                .entry(line_index)
                .or_insert(rowkeys_document.clone());
            let new_rowkeys_root = new_rowkeys_document.root();

            let inner_selector = Selector::new(selector_string).unwrap();

            let default_row_keys = new_rowkeys_root
                .query_selector(&new_rowkeys_document, &inner_selector)
                .expect(&format!(
                    "The template document should the inner {} tag",
                    selector_string
                ));

            let key_map: Vec<String> = split_keys(line);
            let current_keys_count = key_map.len();
            let special_keys_count = key_map.iter().filter(|x| x.starts_with("\\s")).count();

            for (key_index, key) in key_map.iter().enumerate() {
                let longpress = match longpress {
                    Some(longpress) => longpress.get(key),
                    None => None,
                };
                let new_elem;
                if line_index == 0 {
                    new_elem = create_numbered_key_xml_element(
                        &key,
                        compute_key_hint_label_index(key_index),
                        longpress,
                    );
                } else {
                    new_elem = create_key_xml_element(
                        &key,
                        longpress,
                        key_width,
                        current_keys_count,
                        special_keys_count,
                    );
                }

                default_row_keys.append_new_element(&mut new_rowkeys_document, new_elem);
            }
        }
    }

    let mut row_append = rows_include;

    for (line_index, rowkey_doc) in rowkeys_docs_map {
        let file_name_attr = format!("rowkeys_{}{}", snake_case_display_name, line_index + 1);
        let file_name = format!("{}.xml", file_name_attr);

        row_append = row_append.append_new_element_after(
            &mut rows_document,
            NewElement {
                name: qname!("Row"),
                attrs: [].into(),
            },
        );

        row_append.append_new_element(
            &mut rows_document,
            NewElement {
                name: qname!("include"),
                attrs: [
                    (
                        qname!("latin:keyboardLayout"),
                        format!("@xml/{}", &file_name_attr),
                    ),
                    (qname!("latin:keyWidth"), format!("{key_width}%p")),
                ]
                .into(),
            },
        );

        std::fs::write(
            xml_path.join(file_name),
            rowkey_doc.to_string_pretty_with_config(&PRETTY_CONFIG),
        )
        .unwrap();
    }

    let rows_file_name = format!("rows_{}.xml", snake_case_display_name);

    std::fs::write(
        xml_path.join(&rows_file_name),
        rows_document.to_string_pretty_with_config(&PRETTY_CONFIG),
    )
    .unwrap();
}

fn escape_quotes(input: Option<&str>) -> Option<String> {
    match input {
        Some(v) => Some(v.replace("\"", "\\\"")),
        None => None,
    }
}

fn generate_gradle_local(target: &target::Android, app_path: &Path) {
    let store_file = target
        .key_store
        .as_ref()
        .and_then(|x| {
            std::fs::canonicalize(x)
                .ok()
                .map(|x| x.to_str().unwrap().to_string().replace("\\", "\\\\"))
        })
        .unwrap_or_else(|| "".to_string());
    let key_alias = escape_quotes(target.key_alias.as_deref()).unwrap_or_else(|| "".to_string());
    let store_pw =
        escape_quotes(target.store_password.as_deref()).unwrap_or_else(|| "".to_string());
    let key_pw = escape_quotes(target.key_password.as_deref()).unwrap_or_else(|| "".to_string());
    let play_email =
        escape_quotes(target.play_store_account.as_deref()).unwrap_or_else(|| "".to_string());
    let play_creds =
        escape_quotes(target.play_store_p12.as_deref()).unwrap_or_else(|| "".to_string());
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
    .replace("$", "\\$");

    std::fs::write(app_path.join("gradle.local"), text).expect("Failed to write gradle.local file");
}

fn generate_icons(bundle: &KbdgenBundle, resources_path: &Path) {
    const ICON_SIZES: &[(&str, usize)] = &[
        ("mdpi", 48),
        ("hdpi", 72),
        ("xhdpi", 96),
        ("xxhdpi", 144),
        ("xxxhdpi", 192),
    ];

    let icon = match bundle
        .resources
        .android
        .as_ref()
        .and_then(|x| x.icon.as_ref())
    {
        Some(v) => v,
        None => {
            tracing::warn!("No icon found; skipping.");
            return;
        }
    };

    for (suffix, dimension) in ICON_SIZES {
        tracing::debug!("Generating {} - {}x{}", suffix, dimension, dimension);

        let mipmap_path = format!("drawable-{suffix}");

        std::process::Command::new("convert")
            .args(&["convert", "-resize"])
            .arg(format!("{dimension}x{dimension}"))
            .arg(icon)
            .arg(
                resources_path
                    .join(mipmap_path)
                    .join("ic_launcher_keyboard.png"),
            )
            .output()
            .unwrap();
    }
}

fn create_and_write_kbd(main_xml_path: &Path, snake_case_display_name: &str) {
    let mut kbd_document = Document::new("Keyboard");
    let kbd_root = kbd_document.root();

    kbd_root.set_attribute(
        &mut kbd_document,
        "xmlns:latin",
        "http://schemas.android.com/apk/res-auto",
    );

    kbd_root.append_new_element(
        &mut kbd_document,
        NewElement {
            name: qname!("include"),
            attrs: [(
                qname!("latin:keyboardLayout"),
                format!("@xml/rows_{}", snake_case_display_name),
            )]
            .into(),
        },
    );

    std::fs::write(
        main_xml_path.join(format!("kbd_{}.xml", snake_case_display_name)),
        kbd_document.to_string_pretty(),
    )
    .unwrap();
}

fn create_and_write_layout_set(main_xml_path: &Path, snake_case_display_name: &str) {
    let mut layout_set_document = Document::new("KeyboardLayoutSet");
    let layout_root = layout_set_document.root();

    layout_root.set_attribute(
        &mut layout_set_document,
        "xmlns:latin",
        "http://schemas.android.com/apk/res-auto",
    );

    let keyboard_ref = format!("@xml/kbd_{}", snake_case_display_name);

    layout_root.append_new_element(
        &mut layout_set_document,
        NewElement {
            name: qname!("Element"),
            attrs: [
                (qname!("latin:elementName"), "alphabet".to_owned()),
                (qname!("latin:elementKeyboard"), keyboard_ref.clone()),
                (
                    qname!("latin:enableProximityCharsCorrection"),
                    "true".to_owned(),
                ),
            ]
            .into(),
        },
    );

    layout_root.append_new_element(
        &mut layout_set_document,
        make_layout_set_element("alphabetAutomaticShifted", &keyboard_ref),
    );

    layout_root.append_new_element(
        &mut layout_set_document,
        make_layout_set_element("alphabetManualShifted", &keyboard_ref),
    );

    layout_root.append_new_element(
        &mut layout_set_document,
        make_layout_set_element("alphabetShiftLocked", &keyboard_ref),
    );

    layout_root.append_new_element(
        &mut layout_set_document,
        make_layout_set_element("alphabetShiftLockShifted", &keyboard_ref),
    );

    layout_root.append_new_element(
        &mut layout_set_document,
        make_layout_set_element("symbols", "@xml/kbd_symbols"),
    );

    layout_root.append_new_element(
        &mut layout_set_document,
        make_layout_set_element("symbolsShifted", "@xml/kbd_symbols_shift"),
    );

    layout_root.append_new_element(
        &mut layout_set_document,
        make_layout_set_element("phone", "@xml/kbd_phone"),
    );

    layout_root.append_new_element(
        &mut layout_set_document,
        make_layout_set_element("phoneSymbols", "@xml/kbd_phone_symbols"),
    );

    layout_root.append_new_element(
        &mut layout_set_document,
        make_layout_set_element("number", "@xml/kbd_number"),
    );

    std::fs::write(
        main_xml_path.join(format!(
            "keyboard_layout_set_{}.xml",
            snake_case_display_name,
        )),
        layout_set_document.to_string_pretty_with_config(&PRETTY_CONFIG),
    )
    .unwrap();
}

fn create_and_write_values_strings(
    main_values_path: &Path,
    default_display_name: &str,
    current_language_tag_subtype: &str,
) {
    let strings_appname_path = main_values_path.join(Path::new("strings-appname.xml"));
    let file = File::open(strings_appname_path.clone()).expect(&format!(
        "strings-appname to exist in {:?} and open without issues",
        &main_values_path
    ));
    let mut strings_appname_doc =
        Document::from_file(file).expect("can't read strings-appname file");

    let ime_selector =
        Selector::new(r#"string[name="english_ime_name"]"#).expect("css selector do work");

    let ime = strings_appname_doc
        .root()
        .query_selector(&strings_appname_doc, &ime_selector)
        .expect("The strings file should have ime attr");

    ime.set_text(
        &mut strings_appname_doc,
        &format!("{} Keyboard", default_display_name),
    );

    std::fs::write(
        strings_appname_path,
        strings_appname_doc.to_string_pretty_with_config(&PRETTY_CONFIG),
    )
    .unwrap();

    let strings_path = main_values_path.join(Path::new("strings.xml"));
    let file = File::open(strings_path.clone()).expect(&format!(
        "strings to exist in {:?} and open without issues",
        &main_values_path
    ));
    let mut strings_doc = Document::from_file(file).expect("can't read strings file");

    let subtype = strings_doc.root().append_new_element(
        &mut strings_doc,
        NewElement {
            name: qname!("string"),
            attrs: [(qname!("name"), current_language_tag_subtype.to_string())].into(),
        },
    );

    subtype.set_text(&mut strings_doc, &default_display_name);

    std::fs::write(
        strings_path,
        strings_doc.to_string_pretty_with_config(&PRETTY_CONFIG),
    )
    .unwrap();
}

fn update_method_file(
    main_xml_path: &Path,
    method_doc: &mut Document,
    language_tag: &LanguageTag,
    snake_case_display_name: &str,
    subtype_language_tag: &str,
) {
    let mut subtype = method_doc.root().append_new_element(
        method_doc,
        NewElement {
            name: qname!("subtype"),
            attrs: [
                (
                    qname!("android:icon"),
                    "@drawable/ic_ime_switcher_dark".to_string(),
                ),
                (qname!("android:imeSubtypeMode"), "keyboard".to_string()),
            ]
            .into(),
        },
    );

    subtype.set_attribute(
        method_doc,
        "android:label",
        &format!("@string/subtype_{}", subtype_language_tag),
    );
    subtype.set_attribute(
        method_doc,
        "android:imeSubtypeLocale",
        &language_tag.to_string().replace("-", "_"),
    );
    subtype.set_attribute(
        method_doc,
        "android:imeSubtypeExtraValue",
        &format!(
            "KeyboardLayoutSet={},AsciiCapable,EmojiCapable",
            snake_case_display_name
        ),
    );
    subtype.set_attribute(method_doc, "android:isAsciiCapable", "true");
}

fn create_numbered_key_xml_element(
    key: &str,
    key_hint_label_index: Option<usize>,
    longpress: Option<&Vec<String>>,
) -> NewElement {
    let mut attrs = IndexMap::new();

    if key == "\\s{shift}" {
        attrs.insert(qname!("latin:keyStyle"), "shiftKeyStyle".to_owned());
    } else if key == "\\s{backspace}" {
        attrs.insert(qname!("latin:keyStyle"), "deleteKeyStyle".to_owned());
    } else {
        attrs.insert(qname!("latin:keySpec"), key.to_owned());

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

        if let Some(longpress) = longpress.as_ref() {
            let joined_longpress = longpress.join(LONGPRESS_JOIN_CHARACTER);

            attrs.insert(qname!("latin:moreKeys"), joined_longpress.clone());
        }
    }

    NewElement {
        name: qname!("Key"),
        attrs,
    }
}

fn create_key_xml_element(
    key: &str,
    longpress: Option<&Vec<String>>,
    key_width: f64,
    keys_count: usize,
    special_keys_count: usize,
) -> NewElement {
    let mut attrs = IndexMap::new();

    if key == "\\s{shift}" {
        attrs.insert(qname!("latin:keyStyle"), "shiftKeyStyle".to_owned());
        let normal_keys = keys_count - special_keys_count;
        let total_width = key_width * normal_keys as f64;
        let remaining_space = 100f64 - total_width;
        let fill_left = remaining_space / special_keys_count as f64;
        tracing::debug!("Shift fill left: {:.2}%", fill_left);
        attrs.insert(qname!("latin:keyWidth"), format!("{fill_left:.2}%"));
    } else if key == "\\s{backspace}" {
        attrs.insert(qname!("latin:keyStyle"), "deleteKeyStyle".to_owned());
        attrs.insert(qname!("latin:keyWidth"), "fillRight".to_owned());
    } else {
        if let Some(longpress) = longpress {
            let joined_longpress = longpress.join(LONGPRESS_JOIN_CHARACTER);

            let longpress_hint = longpress
                .first()
                .expect("longpress to actually have at least one key");

            attrs.insert(qname!("latin:keyHintLabel"), longpress_hint.to_owned());

            attrs.insert(qname!("latin:moreKeys"), joined_longpress.clone());
        }

        attrs.insert(qname!("latin:keySpec"), key.to_owned());
    }

    NewElement {
        name: qname!("Key"),
        attrs,
    }
}

fn compute_key_hint_label_index(key_index: usize) -> Option<usize> {
    let mut key_hint_label_index = key_index + 1;

    if key_index == 9 {
        key_hint_label_index = 0;
    }

    if key_hint_label_index > 9 {
        return None;
    } else {
        return Some(key_hint_label_index);
    }
}

fn make_layout_set_element(element_name: &str, keyboard: &str) -> NewElement {
    NewElement {
        name: qname!("Element"),
        attrs: [
            (qname!("latin:elementName"), element_name.parse().unwrap()),
            (qname!("latin:elementKeyboard"), keyboard.to_owned()),
        ]
        .into(),
    }
}
