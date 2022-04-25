use std::{path::Path, sync::Arc};
use std::cell::RefCell;

use serde::{Serialize, Deserialize};
use async_trait::async_trait;
use xmlem::Document;
use xmlem::element::Element;
use xmlem::node::Node;
use xmlem::qname::QName;

use crate::{build::BuildStep, bundle::KbdgenBundle};

pub const KEY_LAYOUT_EXT: &str = "keylayout";

const TOP_FOLDER: &str = "Contents";
const RESOURCES_FOLDER: &str = "Resources";
const PLIST_FILENAME: &str = "Info.plist";

const PLIST_TEMPLATE: &str = include_str!("../../../resources/template-macos-plist.xml");
const LAYOUT_TEMPLATE: &str = include_str!("../../../resources/template-macos-layout.xml");

#[derive(Serialize, Deserialize)]
pub struct InfoPlist {
    #[serde(rename = "CFBundleIdentifier")]
    pub cf_bundle_identifier: String,
    #[serde(rename = "CFBundleName")]
    pub cf_bundle_name: String,
    #[serde(rename = "CFBundleVersion")]
    pub cf_bundle_version: String,
    #[serde(rename = "CFBundleShortVersionString")]
    pub cf_bundle_short_version_string: String,
}

pub struct GenerateMacOs {}

#[async_trait(?Send)]
impl BuildStep for GenerateMacOs {
    async fn build(&self, bundle: Arc<KbdgenBundle>, output_path: &Path) {
        
        let contents_path = output_path.join(TOP_FOLDER);
        let cloned_contents_path = contents_path.clone();
        let resources_path = contents_path.join(RESOURCES_FOLDER);

        std::fs::create_dir_all(contents_path).unwrap();
        std::fs::create_dir_all(resources_path.clone()).unwrap();

        let mut plist: InfoPlist = plist::from_bytes(PLIST_TEMPLATE.as_bytes()).unwrap();
        println!("what's my CFBundleIdentifier: {}", plist.cf_bundle_identifier);
        plist.cf_bundle_name = "MyAmazingKbdgenBundle".to_string();

        plist::to_file_xml(cloned_contents_path.join(PLIST_FILENAME), &plist).unwrap();

        // One .keylayout file in Resources folder per language with MacOS primary platform
        for (language_tag, layout) in &bundle.layouts {
            if let Some(mac_os_target) = &layout.mac_os {
                let layers = &mac_os_target.primary.layers;

                
            }
        }
    }
}

fn compute_keyboard_id(language_name: &str) -> String {
    "-8045".to_string()
}

/*
    let document = Document::from_str(LAYOUT_TEMPLATE).unwrap();

    let doc_children = document.children().unwrap();
    let children = RefCell::borrow(&*doc_children);

    let root = document.root();

    let borrowed_root = RefCell::borrow(&*root);
    let modifier_map_elem = borrowed_root.find_child_tag_with_name("modifierMap").unwrap();

    let borrowed_modifier_map = RefCell::borrow(&*modifier_map_elem);
    let key_map_select = borrowed_modifier_map.find_child_tag_with_name("keyMapSelect").unwrap();
    let borrowed_key_map_select = RefCell::borrow(&*key_map_select);

    let modifier = Element::new_child(&key_map_select, "modifier").unwrap();
    {
        let el = modifier.borrow();
        el.add_attr(QName::without_namespace("keys"), "command?");
    }

    let key_layout_path =
        resources_path.join(format!("{}.{}", language_tag.to_string(), KEY_LAYOUT_EXT));
    std::fs::write(key_layout_path, document.to_string()).unwrap();
*/


// return str(-min(max(binascii.crc_hqx(name.encode("utf-8"), 0) // 2, 1), 32768,))
