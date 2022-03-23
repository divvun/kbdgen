use std::{path::Path, sync::Arc};

use codecs::utf16::Utf16Ext;
use language_tags::LanguageTag;

use crate::{
    build::BuildStep,
    bundle::{
        layout::{Layout, WindowsTarget},
        KbdgenBundle,
    },
};

use super::{
    klc::{
        dead_key::KlcDeadKeys,
        file::KlcFileMetadata,
        file::{KlcFile, KLC_EXT},
        key::KlcKey,
        keymap::MSKLC_KEYS,
        layout::{KlcLayer, KlcLayout, KlcLayoutRow},
        ligature::{KlcLigature, KlcLigatureRow},
    },
    layer_set::{populate_layer_set, WindowsLayerSet, WindowsLayerSetKey},
};

pub struct GenerateKlc {}

impl BuildStep for GenerateKlc {
    fn build(&self, bundle: Arc<KbdgenBundle>, output_path: &Path) {
        // One .klc file per language with Windows primary platform
        for (language_tag, layout) in &bundle.layouts {
            if let Some(windows_target) = &layout.windows {
                let metadata =
                    generate_metadata(bundle.clone(), language_tag, layout, windows_target);

                let layers = &windows_target.primary.layers;

                let mut klc_layout_rows = Vec::new();
                let mut klc_ligature_rows = Vec::new();
                let mut dead_key_characters = Vec::new();

                let mut cursor = 0;
                for (_iso_key, klc_key) in MSKLC_KEYS.iter() {
                    let mut layer_set = WindowsLayerSet::default();

                    // Layer set to determine caps_mode, special escapes, dead keys and null keys
                    for (layer, key_map) in layers {
                        populate_layer_set(
                            &mut layer_set,
                            layer,
                            &key_map,
                            cursor,
                            windows_target.dead_keys.as_ref(),
                        );
                    }

                    klc_layout_rows.push(KlcLayoutRow {
                        scancode: klc_key.scancode.clone(),
                        virtual_key: klc_key.virtual_key.clone(),
                        caps_mode: layer_set.caps_mode(),
                        default_key: convert_to_klc_key(
                            layer_set.default,
                            &klc_key.virtual_key,
                            KlcLayer::Default,
                            &mut klc_ligature_rows,
                            &mut dead_key_characters,
                        ),
                        shift_key: convert_to_klc_key(
                            layer_set.shift,
                            &klc_key.virtual_key,
                            KlcLayer::Shift,
                            &mut klc_ligature_rows,
                            &mut dead_key_characters,
                        ),
                        ctrl_key: convert_to_klc_key(
                            layer_set.ctrl,
                            &klc_key.virtual_key,
                            KlcLayer::Ctrl,
                            &mut klc_ligature_rows,
                            &mut dead_key_characters,
                        ),
                        alt_key: convert_to_klc_key(
                            layer_set.alt,
                            &klc_key.virtual_key,
                            KlcLayer::Alt,
                            &mut klc_ligature_rows,
                            &mut dead_key_characters,
                        ),
                        alt_and_shift_key: convert_to_klc_key(
                            layer_set.alt_and_shift,
                            &klc_key.virtual_key,
                            KlcLayer::AltAndShift,
                            &mut klc_ligature_rows,
                            &mut dead_key_characters,
                        ),
                    });

                    cursor += 1;
                }

                let klc_file = KlcFile {
                    metadata,
                    layout: KlcLayout {
                        rows: klc_layout_rows,
                    },
                    ligature: KlcLigature {
                        rows: klc_ligature_rows,
                    },
                    dead_keys: KlcDeadKeys {
                        characters: dead_key_characters,
                        transforms: &layout.transforms,
                    },
                };

                let klc_bytes = klc_file.to_string().encode_utf16_le_bom();
                let klc_path =
                    output_path.join(format!("{}.{}", klc_file.metadata.keyboard_name, KLC_EXT));
                std::fs::write(klc_path, klc_bytes).unwrap();
            }
        }
    }
}

fn generate_metadata(
    bundle: Arc<KbdgenBundle>,
    language_tag: &LanguageTag,
    layout: &Layout,
    target: &WindowsTarget,
) -> KlcFileMetadata {
    let keyboard_name = format!(
        "kbd{}",
        target
            .config
            .as_ref()
            .and_then(|t| t.id.as_ref())
            .map(|x| x.to_string())
            .unwrap_or_else(|| language_tag.as_str().chars().take(5).collect::<String>())
    );

    let description = layout.autonym().to_owned();

    let copyright = bundle.project.copyright.clone();
    let company = bundle.project.organisation.clone();

    KlcFileMetadata {
        keyboard_name,
        description,
        copyright,
        company,
    }
}

fn convert_to_klc_key(
    key: Option<WindowsLayerSetKey>,
    virtual_key: &str,
    klc_layer: KlcLayer,
    klc_ligature_rows: &mut Vec<KlcLigatureRow>,
    dead_key_characters: &mut Vec<char>,
) -> KlcKey {
    match key {
        Some(key) => {
            let utf16s: Vec<u16> = key.string.encode_utf16().collect::<Vec<_>>();

            if utf16s.len() == 1 {
                let character = key.string.chars().next().unwrap();

                if key.dead_key {
                    dead_key_characters.push(character);

                    KlcKey::DeadKey(character)
                } else {
                    KlcKey::Character(character)
                }
            } else {
                let ligature_row = KlcLigatureRow {
                    virtual_key: virtual_key.to_owned(),
                    shift_state: (klc_layer as u8).to_string(),
                    utf16s,
                };

                klc_ligature_rows.push(ligature_row);

                KlcKey::Ligature
            }
        }
        None => KlcKey::None,
    }
}
