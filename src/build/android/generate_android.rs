use std::str::FromStr;
use std::{fs::File, path::Path};

use async_trait::async_trait;
use futures::stream::Select;
use indexmap::IndexMap;
use language_tags::LanguageTag;
use serde::Serialize;
use url::Url;
use xmlem::{Document, NewElement, Selector};

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
const SHORT_WIDTH_XML_PART: &str = "xml-sw600dp";

const DEFAULT_LOCALE: &str = "en";

const DEFAULT_ROWKEYS_TAG: &str = "default";
const SHIFT_ROWKEYS_TAG: &str = "case";

const LONGPRESS_JOIN_CHARACTER: &str = ",";

#[derive(Serialize)]
pub struct AndroidLayout {
    pub transforms: IndexMap<String, String>,
    pub speller: AndroidSpeller,
}

#[derive(Serialize)]
pub struct AndroidSpeller {
    pub path: String,
    pub package_url: Url,
}

pub struct GenerateAndroid;

#[async_trait(?Send)]
impl BuildStep for GenerateAndroid {
    async fn build(&self, bundle: &KbdgenBundle, output_path: &Path) {
        let output_path = output_path.join(Path::new(REPOSITORY_FOLDER));
        let top_path = output_path.join(Path::new(TOP_FOLDER));
        let assets_layouts_path = top_path.join(Path::new(ASSETS_LAYOUTS_PART));
        let resources_path = top_path.join(Path::new(RESOURCES_PART));

        let main_values_path = resources_path.join(Path::new(MAIN_VALUES_PART));

        let main_xml_path = resources_path.join(Path::new(MAIN_XML_PART));
        let short_width_xml_path = resources_path.join(Path::new(SHORT_WIDTH_XML_PART));

        let default_language_tag =
            LanguageTag::parse(DEFAULT_LOCALE).expect("default language tag must parse");

        std::fs::create_dir_all(&assets_layouts_path).unwrap();
        std::fs::create_dir_all(&main_xml_path).unwrap();
        std::fs::create_dir_all(&short_width_xml_path).unwrap();

        // One set of rowkeys_{displayName}_keyboard{count}.xml file per language with an Android platform
        // x files for lines (should be 3)
        // (pretending we're following the primary approach for start)
        for (language_tag, layout) in &bundle.layouts {
            if let Some(android_target) = &layout.android {
                let assets_layout = AndroidLayout {
                    transforms: IndexMap::new(), // should this be more? can mobile keys have transforms?
                    speller: AndroidSpeller {
                        path: android_target
                            .config
                            .speller_path
                            .as_ref()
                            .expect("no speller path supplid for android!")
                            .to_string(),
                        package_url: Url::parse(
                            &android_target
                                .config
                                .speller_package_key
                                .as_ref()
                                .expect("no speller package key provided for android!")
                                .to_string(),
                        )
                        .expect("the speller package url to be parseable"),
                    },
                };

                std::fs::write(
                    assets_layouts_path.join(format!("{}.json", language_tag.to_string(),)),
                    serde_json::to_string_pretty(&assets_layout)
                        .expect("the generated assets layout to serialize correctly"),
                )
                .unwrap();

                let longpress = &layout.longpress;

                let rowkeys_display_name = layout
                    .display_names
                    .get(&default_language_tag)
                    .expect(&format!("no '{}' displayName!", DEFAULT_LOCALE));

                let layers = &android_target.primary.layers;

                let _rows_document =
                    Document::from_str(ROWS_TEMPLATE).expect("invalid rows template");

                let rowkeys_document =
                    Document::from_str(ROWKEYS_TEMPLATE).expect("invalid rowkeys template");

                let mut rowkeys_docs_map = IndexMap::new();

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

                        for (key_index, key) in key_map.iter().enumerate() {
                            let longpress = match longpress {
                                Some(longpress) => match longpress.get(key) {
                                    Some(longpress_keys) => {
                                        Some(longpress_keys.join(LONGPRESS_JOIN_CHARACTER))
                                    }
                                    None => None,
                                },
                                None => None,
                            };

                            let new_elem = create_key_xml_element(
                                &key,
                                // incorrect for keyboard beyond 1 - review python code
                                compute_key_hint_label_index(key_index),
                                longpress,
                            );

                            default_row_keys
                                .append_new_element(&mut new_rowkeys_document, new_elem);
                        }
                    }
                }

                for (line_index, mut rowkey_doc) in rowkeys_docs_map {
                    std::fs::write(
                        main_xml_path.join(format!(
                            "rowkeys_{}_keyboard{}.xml",
                            rowkeys_display_name.to_lowercase(),
                            line_index + 1
                        )),
                        rowkey_doc.to_string_pretty(),
                    )
                    .unwrap();

                    for (layer_key, _layer) in layers {
                        let selector_string;

                        match layer_key {
                            AndroidKbdLayer::Default => {
                                selector_string = DEFAULT_ROWKEYS_TAG;
                            }
                            AndroidKbdLayer::Shift => {
                                selector_string = SHIFT_ROWKEYS_TAG;
                            }
                        };

                        let rowkey_doc_root = rowkey_doc.root();

                        let inner_selector = Selector::new(selector_string).unwrap();

                        let default_row_keys = rowkey_doc_root
                            .query_selector(&rowkey_doc, &inner_selector)
                            .expect(&format!(
                                "Document should the inner {} tag",
                                selector_string
                            ));

                        default_row_keys.append_new_element(
                            &mut rowkey_doc,
                            NewElement {
                                name: "key".to_string(),
                                attrs: [
                                    ("latin:keyStyle".to_string(), "deleteKeyStyle".to_string()),
                                    ("latin:keyWidth".to_string(), "fillRight".to_string()),
                                ]
                                .into(),
                            },
                        );

                        std::fs::write(
                            short_width_xml_path.join(format!(
                                "rowkeys_{}_keyboard{}.xml",
                                rowkeys_display_name.to_lowercase(),
                                line_index + 1
                            )),
                            rowkey_doc.to_string_pretty(),
                        )
                        .unwrap();
                    }
                }

                create_and_write_kbd(&main_xml_path, &rowkeys_display_name.to_lowercase());
                create_and_write_layout_set(&main_xml_path, &rowkeys_display_name.to_lowercase());

                // let after_selector = Selector::new("include::after").expect("should be able to select after the include");

                // for each LAYOUT add a rows_northern_sami_keyboard -> points to these

                // add strings here

                let strings_appname_path = main_values_path.join(Path::new("strings-appname.xml"));
                let file = File::open(strings_appname_path).expect(&format!(
                    "strings-appname to exist in {:?} and open without issues",
                    &main_values_path
                ));
                let strings_appname_doc =
                    Document::from_file(file).expect("can't read strings-appname file");

                let ime_selector = Selector::new(r#"string[name="english_ime_name"]"#)
                    .expect("css selector do work");

                let ime = strings_appname_doc
                    .root()
                    .query_selector(&strings_appname_doc, &ime_selector)
                    .expect("The strings file should have ime attr");

                let child = ime.children(&strings_appname_doc).first().expect("aa");
                //ime.remove_child(document, child)

                let strings_path = main_values_path.join(Path::new("strings.xml"));
                let file = File::open(strings_path).expect(&format!(
                    "strings to exist in {:?} and open without issues",
                    &main_values_path
                ));
                let mut strings_doc = Document::from_file(file).expect("can't read strings file");

                let subtype = strings_doc.root().append_new_element(
                    &mut strings_doc,
                    NewElement {
                        name: "string".to_owned(),
                        attrs: [(
                            "name".to_owned(),
                            format!("subtype_{}", rowkeys_display_name),
                        )]
                        .into(),
                    },
                );
            }
        }

        /*
          (use "git add <file>..." to include in what will be committed)
            app/src/main/assets/
            app/src/main/jniLibs/arm64-v8a/
            app/src/main/jniLibs/armeabi-v7a/
            app/src/main/res/values-en/strings-appname.xml
            app/src/main/res/values-nb/strings-appname.xml
            app/src/main/res/values-nn/
            app/src/main/res/values-no/
            app/src/main/res/values-se/
            app/src/main/res/xml-sw600dp/rowkeys_northern_sami_keyboard1.xml
            app/src/main/res/xml-sw600dp/rowkeys_northern_sami_keyboard2.xml
            app/src/main/res/xml-sw600dp/rowkeys_northern_sami_keyboard3.xml
            app/src/main/res/xml-sw600dp/rows_northern_sami_keyboard.xml
            app/src/main/res/xml/kbd_northern_sami_keyboard.xml
            app/src/main/res/xml/keyboard_layout_set_northern_sami_keyboard.xml
            app/src/main/res/xml/rowkeys_northern_sami_keyboard1.xml
            app/src/main/res/xml/rowkeys_northern_sami_keyboard2.xml
            app/src/main/res/xml/rowkeys_northern_sami_keyboard3.xml
            app/src/main/res/xml/rows_northern_sami_keyboard.xml
        */

        // Musings

        // there are a lot of different folders, some of which seem to contain similar files

        // Modified
        // app/src/main/res/values/strings.xml
        // app/src/main/res/values-da/strings.xml
        // app/src/main/res/values-fi/strings.xml
        // app/src/main/res/values-nb/strings.xml
        // app/src/main/res/values-sv/strings.xml -> these seem to be based on display name
        // entries
        // subtle changes
        // change in xml namespace that brendon said isn't super valid

        // modified app/src/main/res/values/strings-appname.xml

        // added
        // app/src/main/res/values-en/strings-appname.xml
        // app/src/main/res/values-nb/strings-appname.xml

        // seem to be per major folder?
        // just seem like names for things. probably derived from project.yaml
        // since only en and nb got added

        // modified:
        // modified:   app/src/main/res/xml/method.xml
        // may just be comment removal

        // modified:   app/src/main/res/xml/spellchecker.xml
        // may just be comment removal

        // added: app/src/main/assets/
        // main thing added here seems to be a layouts.json inside of assets
        // no info here just link to the bhdfst and pahkat sme stuff

        // added app/src/main/jniLibs/arm64-v8a/
        // 2 .so files... oi...

        // added app/src/main/jniLibs/armeabi-v7a/
        // 2 .so files... oi...

        // added app/src/main/res/xml/rows_northern_sami_keyboard.xml
        // same as above but for the non "-sw600dp" version
    }
}

fn create_and_write_kbd(main_xml_path: &Path, rowkeys_display_name: &str) {
    let mut kbd_document = Document::new("KeyboardLayoutSet");
    let kbd_root = kbd_document.root();

    kbd_root.set_attribute(
        &mut kbd_document,
        "xmlns:latin",
        "http://schemas.android.com/apk/res-auto",
    );

    kbd_root.append_new_element(
        &mut kbd_document,
        NewElement {
            name: "include".to_owned(),
            attrs: [(
                "latin:keyboardLayout".to_owned(),
                format!("@xml/rows_{}_keyboard", rowkeys_display_name),
            )]
            .into(),
        },
    );

    std::fs::write(
        main_xml_path.join(format!("kbd_{}_keyboard.xml", rowkeys_display_name,)),
        kbd_document.to_string_pretty(),
    )
    .unwrap();
}

fn create_and_write_layout_set(main_xml_path: &Path, rowkeys_display_name: &str) {
    let mut layout_set_document = Document::new("KeyboardLayoutSet");
    let layout_root = layout_set_document.root();

    layout_root.set_attribute(
        &mut layout_set_document,
        "xmlns:latin",
        "http://schemas.android.com/apk/res-auto",
    );

    let keyboard_ref = format!("@xml/kbd_{}_keyboard", rowkeys_display_name);

    layout_root.append_new_element(
        &mut layout_set_document,
        NewElement {
            name: "Element".to_owned(),
            attrs: [
                ("latin:elementName".to_owned(), "alphabet".to_owned()),
                ("latin:elementKeyboard".to_owned(), keyboard_ref.clone()),
                (
                    "latin:enableProximityCharsCorrection".to_owned(),
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
            "keyboard_layout_set_{}_keyboard.xml",
            rowkeys_display_name,
        )),
        layout_set_document.to_string_pretty(),
    )
    .unwrap();
}

fn create_key_xml_element(
    key: &str,
    key_hint_label_index: Option<usize>,
    longpress: Option<String>,
) -> NewElement {
    let mut attrs = IndexMap::new();

    attrs.insert("latin:keySpec".to_owned(), key.to_owned());

    if let Some(key_hint_label_index) = key_hint_label_index {
        attrs.insert(
            "latin:keyHintLabel".to_owned(),
            key_hint_label_index.to_string(),
        );
        attrs.insert(
            "latin:additionalMoreKeys".to_owned(),
            key_hint_label_index.to_string(),
        );
    }

    if let Some(longpress) = longpress.as_ref() {
        attrs.insert("latin:moreKeys".to_owned(), longpress.clone());
    }

    NewElement {
        name: "key".to_owned(),
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
        name: "Element".to_owned(),
        attrs: [(element_name.to_owned(), keyboard.to_owned())].into(),
    }
}
