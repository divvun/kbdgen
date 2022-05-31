use std::str::FromStr;
use std::{fs::File, path::Path};

use async_trait::async_trait;
use futures::stream::Select;
use indexmap::IndexMap;
use language_tags::LanguageTag;
use qname::qname;
use serde::Serialize;
use url::Url;
use xmlem::{Document, NewElement, Node, Selector};

use crate::bundle::project::{self, LocaleProjectDescription};
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
    // The generator is currently not designed to be ran more than once
    // it will generate extra subtypes for some of the files
    async fn build(&self, bundle: &KbdgenBundle, output_path: &Path) {
        let mut android_targets = false;

        for (language_tag, layout) in &bundle.layouts {
            if let Some(android_target) = &layout.android {
                android_targets = true;
                break;
            }
        }

        if !android_targets {
            tracing::warn!("No Android targets found in the supplied kbdgen bundle!");
            return;
        }

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

        let subtype_selector = Selector::new("subtype").expect("subtype selector");

        // Method

        let method_path = main_xml_path.join(Path::new("method.xml"));
        let file = File::open(method_path.clone()).expect(&format!(
            "method.xml to exist in {:?} and open without issues",
            &main_xml_path
        ));

        let mut method_doc = Document::from_file(file).expect("can't read strings file");

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

                let lowecase_scored_display_name =
                    default_display_name.to_lowercase().replace(" ", "_");

                let layers = &android_target.primary.layers;

                // Rows

                let mut rows_document =
                    Document::from_str(ROWS_TEMPLATE).expect("invalid rows template");

                let include_selector = Selector::new("include").expect("this selector is fine");

                let rows_include = rows_document
                    .root()
                    .query_selector(&mut rows_document, &include_selector)
                    .expect("there should be an include");

                // Rowkeys

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
                                new_elem = create_key_xml_element(&key, longpress);
                            }

                            default_row_keys
                                .append_new_element(&mut new_rowkeys_document, new_elem);
                        }
                    }
                }

                let mut row_append = rows_include;

                for (line_index, mut rowkey_doc) in rowkeys_docs_map {
                    std::fs::write(
                        main_xml_path.join(format!(
                            "rowkeys_{}_keyboard{}.xml",
                            default_display_name.to_lowercase(),
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
                                name: qname!("key"),
                                attrs: [
                                    (qname!("latin:keyStyle"), "deleteKeyStyle".to_string()),
                                    (qname!("latin:keyWidth"), "fillRight".to_string()),
                                ]
                                .into(),
                            },
                        );

                        let file_name = format!(
                            "rowkeys_{}_keyboard{}.xml",
                            lowecase_scored_display_name,
                            line_index + 1
                        );

                        // commented out because xmlem bug
                        /*
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
                                        format!("@xml/{}", &file_name),
                                    ),
                                    (qname!("latin:keyWidth"), "8.18%p".to_owned()),
                                ]
                                .into(),
                            },
                        );*/

                        std::fs::write(
                            short_width_xml_path.join(file_name),
                            rowkey_doc.to_string_pretty(),
                        )
                        .unwrap();
                    }
                }

                create_and_write_kbd(&main_xml_path, &lowecase_scored_display_name);
                create_and_write_layout_set(&main_xml_path, &lowecase_scored_display_name);

                let current_language_tag_subtype = format!("subtype_{}", language_tag);

                create_and_write_values_strings(
                    &main_values_path,
                    &default_display_name,
                    &current_language_tag_subtype,
                );

                for (language_tag, display_name) in &layout.display_names {
                    let folder = resources_path.join(
                        Path::new(&format!("values-{}", language_tag.to_string())).to_owned(),
                    );
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

                    std::fs::write(strings_path, strings_doc.to_string_pretty()).unwrap();
                }

                // Can use template here and insertion after for order preservation for neatness

                // Method
                let mut subtype = method_doc.root().append_new_element(
                    &mut method_doc,
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
                    &mut method_doc,
                    "android:label",
                    &format!("@string/subtype_{}", language_tag.to_string()),
                );
                subtype.set_attribute(
                    &mut method_doc,
                    "android:imeSubtypeLocale",
                    &language_tag.to_string(),
                );
                subtype.set_attribute(
                    &mut method_doc,
                    "android:imeSubtypeExtraValue",
                    &format!(
                        "KeyboardLayoutSet={},AsciiCapable,EmojiCapable",
                        lowecase_scored_display_name
                    ),
                );

                // Spellchecker
                let mut subtype = spellchecker_doc.root().append_new_element(
                    &mut spellchecker_doc,
                    NewElement {
                        name: qname!("subtype"),
                        attrs: [(
                            qname!("android:label"),
                            "@string/subtype_generic".to_string(),
                        )]
                        .into(),
                    },
                );

                subtype.set_attribute(
                    &mut method_doc,
                    "android:imeSubtypeLocale",
                    &language_tag.to_string(),
                );
            }
        }

        for (locale, LocaleProjectDescription { name, .. }) in &bundle.project.locales {
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

            std::fs::write(strings_appname_path, strings_doc.to_string_pretty()).unwrap();
        }

        std::fs::write(method_path, method_doc.to_string_pretty()).unwrap();
        std::fs::write(spellchecker_path, spellchecker_doc.to_string_pretty()).unwrap();

        /*
          (use "git add <file>..." to include in what will be committed)
            app/src/main/jniLibs/arm64-v8a/
            app/src/main/jniLibs/armeabi-v7a/
        */

        // added app/src/main/jniLibs/arm64-v8a/
        // 2 .so files... oi...

        // added app/src/main/jniLibs/armeabi-v7a/
        // 2 .so files... oi...
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
            name: qname!("include"),
            attrs: [(
                qname!("latin:keyboardLayout"),
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
            "keyboard_layout_set_{}_keyboard.xml",
            rowkeys_display_name,
        )),
        layout_set_document.to_string_pretty(),
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

    std::fs::write(strings_appname_path, strings_appname_doc.to_string_pretty()).unwrap();

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

    std::fs::write(strings_path, strings_doc.to_string_pretty()).unwrap();
}

fn create_numbered_key_xml_element(
    key: &str,
    key_hint_label_index: Option<usize>,
    longpress: Option<&Vec<String>>,
) -> NewElement {
    let mut attrs = IndexMap::new();

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

    NewElement {
        name: qname!("key"),
        attrs,
    }
}

fn create_key_xml_element(key: &str, longpress: Option<&Vec<String>>) -> NewElement {
    let mut attrs = IndexMap::new();

    if let Some(longpress) = longpress {
        let joined_longpress = longpress.join(LONGPRESS_JOIN_CHARACTER);

        let longpress_hint = longpress
            .first()
            .expect("longpress to actually have at least one key");

        attrs.insert(qname!("latin:keyHintLabel"), longpress_hint.to_owned());

        attrs.insert(qname!("latin:moreKeys"), joined_longpress.clone());
    }

    attrs.insert(qname!("latin:keySpec"), key.to_owned());

    NewElement {
        name: qname!("key"),
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
        attrs: [(element_name.parse().unwrap(), keyboard.to_owned())].into(),
    }
}
