use std::cell::RefCell;
use std::{path::Path, sync::Arc};

use async_trait::async_trait;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use xmlem::Document;

use crate::build::macos::keymap::MACOS_KEYS;
use crate::bundle::layout::Transform;
use crate::util::{split_keys, TRANSFORM_ESCAPE};
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

#[derive(Debug)]
pub struct DeadKeyTransform {
    id: DeadKeyId,
    terminator: String,
}

type DeadKeyId = String;
type StateId = String;

struct TransformIdManager {
    dead_key_counter: usize,
    state_counter: usize,
}

impl TransformIdManager {
    fn new() -> Self {
        TransformIdManager {
            dead_key_counter: 0,
            state_counter: 0,
        }
    }

    fn next_dead_key(&mut self) -> DeadKeyId {
        let old_counter = self.dead_key_counter;

        self.dead_key_counter = self.dead_key_counter + 1;

        format!("dead_key{:03}", old_counter)
    }

    fn next_state(&mut self) -> StateId {
        let old_counter = self.state_counter;

        self.state_counter = self.state_counter + 1;

        format!("state{:03}", old_counter)
    }
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
        println!(
            "what's my CFBundleIdentifier: {}",
            plist.cf_bundle_identifier
        );
        plist.cf_bundle_name = "MyAmazingKbdgenBundle".to_string();

        plist::to_file_xml(cloned_contents_path.join(PLIST_FILENAME), &plist).unwrap();

        // One .keylayout file in Resources folder per language with MacOS primary platform
        for (language_tag, layout) in &bundle.layouts {
            if let Some(mac_os_target) = &layout.mac_os {
                let mut id_manager = TransformIdManager::new();

                let layers = &mac_os_target.primary.layers;

                let dead_key_count = 0;
                let state_count = 0;

                let mut transform_map: IndexMap<String, Option<String>> = IndexMap::new();
                let mut dead_keys: IndexMap<String, DeadKeyTransform> = IndexMap::new();

                let mut cursor = 0;
                for (_iso_key, index) in MACOS_KEYS.iter() {
                    //let layout_doc = Document::from_str(LAYOUT_TEMPLATE).unwrap();

                    for (layer, key_map) in layers {
                        let key_map: Vec<String> = split_keys(&key_map);

                        tracing::debug!(
                            "iso len: {}; keymap len: {}",
                            MACOS_KEYS.len(),
                            key_map.len()
                        );
                        if MACOS_KEYS.len() > key_map.len() {
                            panic!(
                                r#"Provided layer does not have enough keys, expected {} keys but got {}, in {}:{}:{}:{:?}: \n{:?}"#,
                                MACOS_KEYS.len(),
                                key_map.len(),
                                language_tag.to_string(),
                                "MacOS",
                                "Primary",
                                layer,
                                key_map
                            );
                        }

                        // perhaps add the key index here since this may end up being the map we generate tags from
                        transform_map.insert(key_map[cursor].clone(), None);

                        if let Some(transforms) = &layout.transforms {
                            for (dead_key, value) in transforms {
                                match value {
                                    Transform::End(_character) => {
                                        tracing::error!(
                                            "Transform ended too soon for dead key {}",
                                            dead_key
                                        );
                                    }
                                    Transform::More(map) => {
                                        for (next_char, transform) in map {
                                            match transform {
                                                Transform::End(end_char) => {
                                                    if next_char == TRANSFORM_ESCAPE {
                                                        if !dead_keys.contains_key(dead_key) {
                                                            let id = id_manager.next_dead_key();

                                                            dead_keys.insert(
                                                                dead_key.clone(),
                                                                DeadKeyTransform {
                                                                    id,
                                                                    terminator: end_char.clone(),
                                                                },
                                                            );
                                                        }
                                                    }

                                                    //write_transform(next_char, end_char, f)?;
                                                }
                                                Transform::More(_transform) => {
                                                    //todo!("Recursion required ahead");
                                                }
                                            };
                                        }
                                    }
                                }
                            }
                        }

                        //&key_map[cursor]
                    }
                }

                // in xml, fo reach dead key, add a terminator
                // <terminators>
                //   <when state="{}" output="{}"> key, value
                // </terminators>
                // OR, find the ' ' value and slap it here
                for (key, value) in dead_keys {
                    println!("{}: {:?}", key, value);
                }
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
