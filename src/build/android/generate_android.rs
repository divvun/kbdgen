use std::path::Path;

use async_trait::async_trait;

use crate::{build::BuildStep, bundle::KbdgenBundle};

pub struct GenerateAndroid;

#[async_trait(?Send)]
impl BuildStep for GenerateAndroid {
    async fn build(&self, bundle: &KbdgenBundle, output_path: &Path) {
        // Files added for kbd-sme (confirm)

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
