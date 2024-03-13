use std::path::Path;

use anyhow::Result;
use async_trait::async_trait;
use codecs::utf16::Utf16Ext;
use language_tags::LanguageTag;
use tracing::{debug, trace};

use crate::{
    build::BuildStep,
    bundle::{
        layout::{Layout, WindowsTarget},
        KbdgenBundle, DEFAULT_DECIMAL,
    },
    util::split_keys,
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
    layer_set::{populate_layer_set, WindowsLayerSet, WindowsLayerSetKey, SG_CAP},
};

pub struct GenerateKlc {}

#[async_trait(?Send)]
impl BuildStep for GenerateKlc {
    async fn build(&self, bundle: &KbdgenBundle, output_path: &Path) -> Result<()> {
        trace!("Generating klc files");
        // One .klc file per language with Windows primary platform
        for (language_tag, layout) in &bundle.layouts {
            trace!("Checking if we need a klc file for {}", language_tag);
            if let Some(windows_target) = &layout.windows {
                debug!("Generating klc for {}", language_tag);
                let metadata =
                    generate_metadata(bundle, language_tag, layout, windows_target);

                let layers = &windows_target.primary.layers;

                let mut klc_layout_rows = Vec::new();
                let mut klc_ligature_rows = Vec::new();
                let mut dead_key_characters = Vec::new();

                let mut cursor = 0;
                for (_iso_key, klc_key) in MSKLC_KEYS.iter() {
                    let mut layer_set = WindowsLayerSet::default();

                    // Layer set to determine caps_mode, special escapes, dead keys and null keys
                    for (layer, key_map) in layers {
                        let key_map: Vec<String> = split_keys(&key_map);

                        tracing::debug!(
                            "iso len: {}; keymap len: {}",
                            MSKLC_KEYS.len(),
                            key_map.len()
                        );
                        if MSKLC_KEYS.len() > key_map.len() {
                            panic!(
                                r#"Provided layer does not have enough keys, expected {} keys but got {}, in {}:{}:{}:{:?}: \n{:?}"#,
                                MSKLC_KEYS.len(),
                                key_map.len(),
                                language_tag.to_string(),
                                "Windows",
                                "Primary",
                                layer,
                                key_map
                            );
                        }

                        populate_layer_set(
                            &mut layer_set,
                            layer,
                            key_map,
                            cursor,
                            windows_target.dead_keys.as_ref(),
                        );
                    }

                    let caps_mode = layer_set.caps_mode();

                    klc_layout_rows.push(KlcLayoutRow {
                        scancode: klc_key.scancode.clone(),
                        virtual_key: klc_key.virtual_key.clone(),
                        caps_mode: caps_mode.clone(), // Generate extra key for SGCap here?
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

                    if caps_mode == SG_CAP {
                        let caps_key = convert_to_klc_key(
                            layer_set.caps,
                            &klc_key.virtual_key,
                            KlcLayer::Default,
                            &mut klc_ligature_rows,
                            &mut dead_key_characters,
                        );

                        let caps_shift_key = convert_to_klc_key(
                            layer_set.caps_and_shift,
                            &klc_key.virtual_key,
                            KlcLayer::Shift,
                            &mut klc_ligature_rows,
                            &mut dead_key_characters,
                        );

                        klc_layout_rows.push(KlcLayoutRow {
                            scancode: "-1".to_owned(),
                            virtual_key: "-1".to_owned(),
                            caps_mode: "0".to_owned(),
                            default_key: caps_key,
                            shift_key: caps_shift_key,
                            ctrl_key: KlcKey::Skip,
                            alt_key: KlcKey::Skip,
                            alt_and_shift_key: KlcKey::Skip,
                        });
                    }

                    cursor += 1;
                }

                klc_layout_rows.push(space_layout_row());
                klc_layout_rows.push(decimal_layout_row(&layout.decimal));

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

        Ok(())
    }
}

fn generate_metadata(
    bundle: &KbdgenBundle,
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

    let autonym = layout.autonym().to_string();

    // Language Code Identifier
    // https://docs.microsoft.com/en-us/openspecs/windows_protocols/ms-lcid/70feba9f-294e-491e-b6eb-56532684c37f
    let lcid_record = iso639::lcid::get(
        language_tag.primary_language(),
        language_tag.script(),
        language_tag.region(),
    );

    let locale_id = match lcid_record {
        Some(record) => record.lcid,
        None => 0x2000,
    };

    let locale_name = target
        .config
        .as_ref()
        .and_then(|config| config.locale.as_ref())
        .map(|locale| locale.to_string())
        .unwrap_or_else(|| match lcid_record {
            Some(_) => language_tag.to_string(),
            None => format!(
                "{}-{}-{}",
                language_tag.primary_language(),
                language_tag.script().unwrap_or("Latn"),
                language_tag.region().unwrap_or("001")
            ),
        });

    KlcFileMetadata {
        keyboard_name,
        description,
        copyright,
        company,
        autonym,
        locale_id,
        locale_name,
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

fn space_layout_row() -> KlcLayoutRow {
    KlcLayoutRow {
        scancode: "39".to_owned(),
        virtual_key: "SPACE".to_owned(),
        caps_mode: "0".to_owned(),
        default_key: KlcKey::Character(' '),
        shift_key: KlcKey::Character(' '),
        ctrl_key: KlcKey::Character(' '),
        alt_key: KlcKey::None,
        alt_and_shift_key: KlcKey::None,
    }
}

fn decimal_layout_row(layout_decimal: &Option<String>) -> KlcLayoutRow {
    let mut decimal = DEFAULT_DECIMAL.to_owned();
    if let Some(layout_decimal) = layout_decimal {
        decimal = layout_decimal.clone();
    }

    let decimal = decimal
        .chars()
        .next()
        .expect("Layout decimal must be a single character.");

    KlcLayoutRow {
        scancode: "53".to_owned(),
        virtual_key: "DECIMAL".to_owned(),
        caps_mode: "0".to_owned(),
        default_key: KlcKey::Character(decimal),
        shift_key: KlcKey::Character(decimal),
        ctrl_key: KlcKey::None,
        alt_key: KlcKey::None,
        alt_and_shift_key: KlcKey::None,
    }
}
