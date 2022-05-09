use std::path::Path;
use std::str::FromStr;

use async_trait::async_trait;
use indexmap::IndexMap;
use language_tags::LanguageTag;
use xmlem::{Document, NewElement, Selector};

use crate::{
    build::BuildStep,
    bundle::{layout::android::AndroidKbdLayer, KbdgenBundle},
    util::split_keys,
};

const ROWS_TEMPLATE: &str = include_str!("../../../resources/template-android-rows.xml");
const ROWKEYS_TEMPLATE: &str = include_str!("../../../resources/template-android-rowkeys.xml");

const TOP_FOLDER: &str = "app/src/main";
const RESOURCES_PART: &str = "res";
const MAIN_XML_PART: &str = "xml";
const SHORT_WIDTH_XML_PART: &str = "xml-sw600dp";

const DEFAULT_LOCALE: &str = "en";

const OUTER_ROWKEYS_TAG: &str = "switch";
const DEFAULT_ROWKEYS_TAG: &str = "default";
const SHIFT_ROWKEYS_TAG: &str = "case";

const LONGPRESS_JOIN_CHARACTER: &str = ",";

pub struct GenerateAndroid;

#[async_trait(?Send)]
impl BuildStep for GenerateAndroid {
    async fn build(&self, bundle: &KbdgenBundle, output_path: &Path) {
        let resources_path = output_path
            .join(Path::new(TOP_FOLDER))
            .join(Path::new(RESOURCES_PART));

        let main_xml_path = resources_path.join(Path::new(MAIN_XML_PART));
        let short_width_xml_path = resources_path.join(Path::new(SHORT_WIDTH_XML_PART));

        std::fs::create_dir_all(&main_xml_path).unwrap();
        std::fs::create_dir_all(&short_width_xml_path).unwrap();

        let default_language_tag =
            LanguageTag::parse(DEFAULT_LOCALE).expect("default language tag must parse");

        // One set of rowkeys_{displayName}_keyboard{count}.xml file per language with an Android platform
        // x files for lines (should be 3)
        // (pretending we're following the primary approach for start)
        for (language_tag, layout) in &bundle.layouts {
            if let Some(android_target) = &layout.android {
                let longpress = &layout.longpress;

                let rowkeys_display_name = layout
                    .display_names
                    .get(&default_language_tag)
                    .expect(&format!("no '{}' displayName!", DEFAULT_LOCALE))
                    .to_lowercase();

                let layers = &android_target.primary.layers;

                let rows_document =
                    Document::from_str(ROWS_TEMPLATE).expect("invalid rows template");
                let rows_root = rows_document.root();

                let rowkeys_document =
                    Document::from_str(ROWKEYS_TEMPLATE).expect("invalid rowkeys template");
                let rowkeys_root = rowkeys_document.root();

                let outer_selector =
                    Selector::new(OUTER_ROWKEYS_TAG).expect("outer rowtag must exist");

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

                        let row_keys = new_rowkeys_root
                            .query_selector(&new_rowkeys_document, &outer_selector)
                            .expect(&format!(
                                "The template document should have a {} tag",
                                OUTER_ROWKEYS_TAG
                            ));

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
                            rowkeys_display_name,
                            line_index + 1
                        )),
                        rowkey_doc.to_string_pretty(),
                    )
                    .unwrap();

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

                        let rowkey_doc_root = rowkey_doc.root();

                        let row_keys = rowkey_doc_root
                            .query_selector(&rowkey_doc, &outer_selector)
                            .expect(&format!("Document should have a {} tag", OUTER_ROWKEYS_TAG));

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
                                rowkeys_display_name,
                                line_index + 1
                            )),
                            rowkey_doc.to_string_pretty(),
                        )
                        .unwrap();
                    }
                }

                create_and_write_layout_set(&main_xml_path, &rowkeys_display_name);

                // let after_selector = Selector::new("include::after").expect("should be able to select after the include");

                // for each LAYOUT add a rows_northern_sami_keyboard -> points to these
            }
        }

        // Files added for kbd-sme (confirm)

        let top_folder = "app/src/main";

        let json_folder_join = "assets/layouts"; // join top

        // json file name: {layout}.json

        let jni_libs = "jniLibs"; // join top
        let arm = "arm64-v8a"; // join jni
        let other_arm = "armeabi-v7a"; // join jni

        let jni_file_1 = "libdivvunspell.so"; // join arm
        let jni_file_2 = "libpahkat_client.so"; // join arm

        let res_folder = "res"; // join top

        let top_values = "values"; // join res
                                   // top values folder. (non-critical, ignore initially)
                                   // modify the 'strings-appname.xml' to make sure it has the
                                   // appropriate keyboard display names

        let xml_folder1 = "xml"; // join res
        let xml_folder2 = "xml-sw600dp"; // join res. Do we support other screen ranges?

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

        // added app/src/main/res/xml-sw600dp/rowkeys_northern_sami_keyboard1.xml
        // added app/src/main/res/xml-sw600dp/rowkeys_northern_sami_keyboard2.xml
        // added app/src/main/res/xml-sw600dp/rowkeys_northern_sami_keyboard3.xml
        // wonder why 3 keyboards
        // looks like an actual keyboard, as in, keys, and what seems to be modifiers
        // difference between keyboards unclear

        // added app/src/main/res/xml-sw600dp/rows_northern_sami_keyboard.xml
        // seems to link these keyboards up into one thing
        // maybe the above are literal rows of keys?

        // added app/src/main/res/xml/rowkeys_northern_sami_keyboard1.xml
        // added app/src/main/res/xml/rowkeys_northern_sami_keyboard2.xml
        // added app/src/main/res/xml/rowkeys_northern_sami_keyboard3.xml
        // 3 keebs again, this time without the "-sw600dp" folder name
        // seems at least different sizing (i.e., value of latin:keyWidth differs)
        // probably same keyboards but for different screen (or default screen)

        // added app/src/main/res/xml/rows_northern_sami_keyboard.xml
        // same as above but for the non "-sw600dp" version

        // added app/src/main/res/xml/kbd_northern_sami_keyboard.xml

        // just seems to point to app/src/main/res/xml/rows_northern_sami_keyboard.xml

        // added app/src/main/res/xml/keyboard_layout_set_northern_sami_keyboard.xml

        // modifiers????
    }
}

fn create_and_write_layout_set(main_xml_path: &Path, rowkeys_display_name: &str) {
    let mut layout_set_document = Document::new("KeyboardLayoutSet");
    let layout_root = layout_set_document.root();

    layout_root.set_attribute(
        &mut layout_set_document,
        "xmlns:latin",
        "http://schemas.android.com/apk/res-auto",
    );

    std::fs::write(
        main_xml_path.join(format!(
            "keyboard_layout_sets_{}_keyboard.xml",
            rowkeys_display_name,
        )),
        layout_set_document.to_string_pretty(),
    )
    .unwrap();
}

fn create_main_rowkeys() {}

fn create_key_xml_element(
    key: &str,
    key_hint_label_index: Option<usize>,
    longpress: Option<String>,
) -> NewElement {
    let mut attrs = IndexMap::new();

    attrs.insert("latin:keySpec".to_string(), key.to_owned());

    if let Some(key_hint_label_index) = key_hint_label_index {
        attrs.insert(
            "latin:keyHintLabel".to_string(),
            key_hint_label_index.to_string(),
        );
        attrs.insert(
            "latin:additionalMoreKeys".to_string(),
            key_hint_label_index.to_string(),
        );
    }

    if let Some(longpress) = longpress.as_ref() {
        attrs.insert("latin:moreKeys".to_string(), longpress.clone());
    }

    NewElement {
        name: "key".into(),
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
