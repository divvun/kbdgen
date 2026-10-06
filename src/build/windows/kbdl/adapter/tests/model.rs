//! The engine model the DLL embeds, and v3 and v4 layouts built side by
//! side.

use std::path::Path;

use kbd_model::Host;
use language_tags::LanguageTag;

use crate::build::BuildStep;
use crate::build::windows::kbdl::{
    BuildKbdl, GenerateKbdl,
    build::VARIANTS,
    generate_bundle, image, layout_names,
    resources::{MODEL_RESOURCE_ID, tests::entries},
};

use super::*;

const RT_RCDATA: u16 = 10;

fn model_entry(res: &[u8]) -> Option<(u16, u16, Vec<u8>)> {
    entries(res)
        .into_iter()
        .find(|(kind, ..)| *kind == RT_RCDATA)
        .map(|(_, name, language, _, data)| (name, language, data))
}

const SE_V3: &str = "displayNames: {se: Davvisámegiella}\nwindows:\n  primary:\n    layers:\n      default: a b c d e f g h i j k l m n o p q r s t u v w x y z 1 2 3 4 5 6 7 8 9 0 A B C D E F G H I J K L\n";

// [spec:kbdgen:req:ldml.kbdl.model-resource+1/test]
// [spec:kbdgen:req:tsf.data.resource/test]
// [spec:kbdgen:def:tsf.engine.model+1/test]
#[test]
fn the_dll_embeds_the_windows_keyboard() {
    let fixture = fixture(&[("vro", VRO4)]);
    let layout = fixture.model_layout("vro").unwrap();
    let adapted = adapt(&fixture.bundle, &layout).unwrap();
    let keyboard = layout.layout.keyboard_for(Host::Windows).unwrap();
    assert_eq!(adapted.model, keyboard.to_bytes().unwrap());
    let model = kbd_engine::Model::from_bytes(&adapted.model).unwrap();
    assert_eq!(model.keyboard(), keyboard);

    let generated = generate_bundle(&fixture.bundle).unwrap();
    let (name, language, data) = model_entry(&generated[0].res).unwrap();
    assert_eq!((name, language), (MODEL_RESOURCE_ID, 0));
    assert_eq!(data, adapted.model);
    let kinds: Vec<u16> = entries(&generated[0].res).iter().map(|e| e.0).collect();
    assert_eq!(
        kinds,
        [0, 16, RT_RCDATA, 6, 6, 6],
        "before the string tables"
    );
}

// [spec:kbdgen:req:ldml.kbdl.model-resource+1/test]
// [spec:kbdgen:def:ldml.kbdl.adapter/test]
// [spec:kbdgen:def:tsf.engine.model+1/test]
#[test]
fn v3_and_v4_layouts_build_side_by_side() {
    let fixture = fixture(&[("vro", VRO4), ("se", SE_V3)]);
    assert!(fixture.bundle.reject_v4_layouts("windows").is_ok());
    assert!(fixture.bundle.reject_v4_layouts("macos").is_err());
    let generated = generate_bundle(&fixture.bundle).unwrap();
    let names: Vec<&str> = generated.iter().map(|l| l.name.as_str()).collect();
    assert_eq!(names, ["kbdse", "kbdvro"], "bundle layout order");
    assert_eq!(layout_names(&fixture.bundle).unwrap(), names);
    let migration = crate::ldml::migrate::migrate_bundle_layout(&fixture.bundle.path, "se")
        .unwrap()
        .unwrap();
    let migrated = crate::ldml::migrate::windows_model(&migration)
        .unwrap()
        .unwrap();
    let (_, _, data) = model_entry(&generated[0].res).unwrap();
    assert_eq!(data, migrated, "a v3 layout embeds its migrated model");
    assert!(model_entry(&generated[1].res).is_some());
}

// [spec:kbdgen:req:ldml.kbdl.model-resource+1/test]
// [spec:kbdgen:req:tsf.data.resource/test]
#[test]
fn blocked_v3_migration_builds_without_model() {
    let extra = SE_V3.replace("K L\n", "K L M\n");
    let fixture = fixture(&[("se", &extra)]);
    let tag: LanguageTag = "se".parse().unwrap();
    let bundle = &fixture.bundle;
    let layout = bundle.layouts.get(&tag).unwrap();
    let target = layout.windows.as_ref().unwrap();
    let (input, mut diag) =
        super::super::super::bundle::layout_input(bundle, &tag, layout, target).unwrap();
    let model = super::super::super::v3_model(bundle, &tag, &mut diag);
    assert!(model.is_none());
    assert!(
        diag.warnings()
            .iter()
            .any(|w| w.contains("blocked by defects M05")),
        "{:?}",
        diag.warnings()
    );
    let generated = generate_bundle(bundle).unwrap();
    assert!(model_entry(&generated[0].res).is_none());
    assert_eq!(generated[0].name, input.metadata.name);
}

/// Builds `bundle` for every variant into `out` and checks each DLL's
/// image and its embedded model.
async fn build_and_check(bundle: &KbdgenBundle, out: &Path, model: &[u8]) {
    GenerateKbdl.build(bundle, out).await.unwrap();
    BuildKbdl.build(bundle, out).await.unwrap();
    for variant in VARIANTS {
        let path = out.join(variant.name).join("kbdvro.dll");
        let bytes = std::fs::read(&path).unwrap();
        image::verify(&bytes, variant.machine).unwrap();
        let parsed = image::Image::parse(&bytes).unwrap();
        let embedded = parsed
            .resource(&bytes, RT_RCDATA, MODEL_RESOURCE_ID)
            .unwrap();
        assert_eq!(embedded.as_deref(), Some(model), "{}", variant.name);
    }
}

/// Needs the three `*-pc-windows-msvc` targets (`kbdl.build.toolchain`).
// [spec:kbdgen:req:ldml.kbdl.model-resource+1/test]
// [spec:kbdgen:req:tsf.data.resource/test]
#[tokio::test]
#[ignore = "builds DLLs with the Windows targets of the Rust toolchain"]
async fn built_vro_dlls_verify_and_embed_the_model() {
    let fixture = fixture(&[("vro", VRO4)]);
    let model = fixture.adapt("vro").unwrap().model;
    let out = tempfile::tempdir().unwrap();
    build_and_check(&fixture.bundle, out.path(), &model).await;
}
