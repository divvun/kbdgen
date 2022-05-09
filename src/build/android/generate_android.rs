use std::path::Path;
use std::str::FromStr;

use async_trait::async_trait;
use xmlem::{Document, Selector};

use crate::{build::BuildStep, bundle::{KbdgenBundle, layout::android::AndroidKbdLayer}};

const ROWKEYS_TEMPLATE: &str = include_str!("../../../resources/template-android-rowkeys.xml");

const TOP_FOLDER: &str = "app/src/main";
const RESOURCES_PART: &str = "res";
const MAIN_XML_PART: &str = "xml";
const SHORT_WIDTH_XML_PART: &str = "xml-sw600dp";

pub struct GenerateAndroid;

#[async_trait(?Send)]
impl BuildStep for GenerateAndroid {
    async fn build(&self, bundle: &KbdgenBundle, output_path: &Path) {

        let main_xml_path = Path::new(TOP_FOLDER).join(Path::new(RESOURCES_PART)).join(Path::new(MAIN_XML_PART));



        // One rowkeys_{displayName}_keyboard{count}.xml file per language with an Android platform 
        // (pretending we're following the primary approach for start)
        for (language_tag, layout) in &bundle.layouts {
            if let Some(android_target) = &layout.android {

                let layers = &android_target.primary.layers;



                let mut rowkeys_document = Document::from_str(ROWKEYS_TEMPLATE).expect("invalid template");
                let rowkeys_root = rowkeys_document.root();

                let selector = Selector::new("merge.switch.case").unwrap();


                if let Some(default_layer) = layers.get(&AndroidKbdLayer::Default) {


                    let default_row_keys = rowkeys_root
                        .query_selector(&rowkeys_document, &selector)
                        .expect("The template document should have a tag at 'merge.switch.case'");


                    


                    /*
                    default_row_keys.append_new_element(
                        &mut rowkeys_document,
                        NewElement {
                            name: "key".into(),
                            attrs: [
                                ("latin:keySpec".into(), "aaaa"),

                            ].into(),
                        },
                    );
                     */



                } else { // maybe just hardcode
                    panic!("No default layer for android!")
                }
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



        // xml is the core folder that also contains the root linking xml file
        // seems we add a file with name:

        // rowkeys_{displayName}_keyboard{count}.xml
        // displayName seems to be the   en: Northern Sami displayname


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
