use std::{
    cmp::Ordering,
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    str::FromStr,
};

use async_trait::async_trait;
use language_tags::LanguageTag;
use rayon::prelude::*;

use crate::{
    build::{ios::pbxproj::Pbxproj, ios::xcode_structures::*, BuildStep},
    bundle::KbdgenBundle,
};

const REPOSITORY: &str = "repo";
const HOSTING_APP: &str = "HostingApp";
const HOSTING_INFO_STRINGS: &str = "InfoPlist.strings";

const KEYBOARD: &str = "Keyboard";
const INFO_PLIST: &str = "Info.plist";

const SETTINGS_BUNDLE: &str = "Settings.bundle";
const ROOT_PLIST: &str = "Root.plist";
const ENTITLEMENTS_EXTENSION: &str = ".entitlements";

const DEFAULT_LOCALE: &str = "en";

pub fn replace_all_occurances(input: String, character: char, replace_with: char) -> String {
    input
        .as_str()
        .chars()
        .map(|curr| {
            if curr.cmp(&character) != Ordering::Equal {
                curr
            } else {
                replace_with
            }
        })
        .into_iter()
        .collect::<String>()
}

pub fn generate_keyboard_plist(
    template_path: PathBuf,
    value: IosKeyboardSettings,
    display_name: String,
    keyboard_index: usize,
    primary_language: String,
    output_path: PathBuf,
) {
    let mut keyboard_plist: KeyboardInfoPlist =
        plist::from_file(template_path.clone()).expect("valid stuff");

    keyboard_plist.cf_bundle_display_name = display_name;
    keyboard_plist.cf_bundle_short_version_string = value.short_version;
    keyboard_plist.cf_bundle_version = value.build_version;
    keyboard_plist.ls_application_queries_schemes[0] = value.package_id;
    keyboard_plist
        .ns_extension
        .ns_extension_attributes
        .primary_language = primary_language;
    keyboard_plist.divvun_keyboard_index = keyboard_index;

    plist::to_file_xml(output_path, &keyboard_plist).unwrap();
}

pub fn generate_hosting_plist(
    in_out_path: PathBuf,
    display_name: String,
    value: IosKeyboardSettings,
) {
    let mut hosting_app_plist: HostingPlist =
        plist::from_file(in_out_path.clone()).expect("valid stuff");

    hosting_app_plist.cf_bundle_display_name = display_name;
    hosting_app_plist.cf_bundle_short_version_string = value.short_version;
    hosting_app_plist.cf_bundle_version = value.build_version;
    hosting_app_plist.cf_bundle_url_types[0].cf_bundle_url_schemes[0] = value.package_id.clone();
    hosting_app_plist.ls_application_queries_schemes[0] = value.package_id;

    plist::to_file_xml(in_out_path, &hosting_app_plist).unwrap();
}

pub fn update_entitlements(entitlements_path: PathBuf, new_entitlements: Vec<String>) {
    let mut keyboard_entitlements: EntitlementsDict =
        plist::from_file(entitlements_path.clone()).expect("valid stuff");
    keyboard_entitlements.com_apple_security_application_groups = new_entitlements;
    plist::to_file_xml(entitlements_path, &keyboard_entitlements).unwrap();
}

#[derive(Clone)]
pub struct IosKeyboardSettings {
    short_version: String,
    build_version: String,
    package_id: String,
}

pub fn path_to_relative(path: &Path, relative_to: &str) -> PathBuf {
    let mut path_string = path.to_str().unwrap().to_string();
    path_string.replace_range(
        0..path_string.find(relative_to).unwrap() + relative_to.len() + 1,
        "",
    );
    return PathBuf::from_str(&path_string).unwrap();
}

pub fn generate_icons(bundle: &KbdgenBundle, path: &Path) {
    let icon = bundle
        .resources
        .ios
        .as_ref()
        .unwrap()
        .icons
        .get(&LanguageTag::from_str("png").unwrap())
        .expect("no icon found for ios");

    let images_path = path.join("Images.xcassets");
    let appiconset_path = images_path.join("AppIcon.appiconset");
    let contents_path = appiconset_path.join("Contents.json");

    let mut contents: AppIconSetContents = serde_json::from_str(
        &fs::read_to_string(&contents_path).expect("could not read appicon file"),
    )
    .expect("could not parse appiconset contents");

    contents.images.par_iter_mut().for_each(|content| {
        tracing::debug!("Generating icon at scale {}", &content.scale);
        let filename = format!(
            "{}-{}@{}.png",
            &content.idiom, &content.size, &content.scale
        );

        let size_axis: Vec<f32> = content
            .size
            .split("x")
            .map(|x| serde_json::from_str::<f32>(x).unwrap())
            .collect::<Vec<f32>>();
        let axis_multiplier =
            serde_json::from_str::<f32>(&content.scale.replace("x", "").to_string()).unwrap();
        let new_axis = size_axis.first().unwrap() * axis_multiplier;
        std::process::Command::new("convert")
            .arg("-resize")
            .arg(&format!("{new_axis}x{new_axis}"))
            .args(&[
                "-background",
                "transparent",
                "-gravity",
                "center",
                "-extent",
            ])
            .arg(&format!("{new_axis}x{new_axis}"))
            .arg(icon)
            .arg(appiconset_path.join(&filename))
            .output()
            .expect("convert failed to run");
        content.filename = Some(filename);
    });
    fs::write(
        contents_path,
        serde_json::to_string_pretty(&contents).unwrap(),
    )
    .unwrap();
}

pub struct GenerateXcode;

#[async_trait(?Send)]
impl BuildStep for GenerateXcode {
    async fn build(&self, bundle: &KbdgenBundle, output_path: &Path) {
        let repository_path = output_path.join(REPOSITORY);
        let hosting_app_path = repository_path.join(HOSTING_APP);
        let keyboard_path = repository_path.join(KEYBOARD);

        let xcodeproj_path = repository_path.join("GiellaKeyboard.xcodeproj");
        let pbxproj_path = xcodeproj_path.join("project.pbxproj");
        let mut pbxproj = Pbxproj::from_path(&pbxproj_path);

        // ADD PROJECT LOCALES TO ALL LOCALES LIST AND REPLACE "en" WITH "Base"
        let mut all_locales: BTreeSet<String> =
            bundle.project.locales.keys().map(|x| x.clone()).collect();
        if all_locales.remove("en") {
            all_locales.insert("Base".to_string());
        }

        let default_language_tag =
            LanguageTag::parse(DEFAULT_LOCALE).expect("default language tag must parse");

        if let Some(target) = &bundle.targets.ios {
            let ios_keyboard_settings = IosKeyboardSettings {
                short_version: target.version.clone(),
                build_version: target.build.clone(),
                package_id: target.package_id.clone(),
            };

            for (layout_index, (language_tag, layout)) in bundle
                .layouts
                .iter()
                .filter(|x| x.1.i_os.is_some())
                .enumerate()
            {
                if let Some(_ios_layout) = &layout.i_os {
                    // GENERATE LOCALES
                    // TODO: Check if About.txt exists for locale before creating file reference

                    for (locale_name, locale_info) in &bundle.project.locales {
                        let locale_name = if locale_name == "en" {
                            "Base"
                        } else {
                            locale_name
                        };
                        let locale_path = hosting_app_path.join(&format!("{}.lproj", locale_name));
                        let info_path = locale_path.join(HOSTING_INFO_STRINGS);

                        std::fs::create_dir_all(&locale_path).unwrap();

                        let info_strings = XcodeHostingInfoStrings {
                            cf_bundle_name: locale_info.name.to_string(),
                            cf_bundle_display_name: locale_info.name.to_string(),
                        };
                        std::fs::write(info_path, info_strings.to_string()).unwrap();

                        if locale_name == "Base" {
                            continue;
                        } else {
                            if Path::new(&format!(
                                "{REPOSITORY}/{HOSTING_APP}/{locale_name}.lproj/About.txt"
                            ))
                            .exists()
                            {
                                let about_id =
                                    pbxproj.create_file_reference("text", locale_name, "About.txt");
                                pbxproj.add_file_ref_to_variant_group(about_id);
                            }
                        };
                    }

                    // KEYBOARD PLIST
                    let default_display_name = layout
                        .display_names
                        .get(&default_language_tag)
                        .expect(&format!("no '{}' displayName!", DEFAULT_LOCALE));

                    let layout_folder_name = default_display_name
                        .to_lowercase()
                        .replace(" ", "-")
                        .replace("(", "")
                        .replace(")", "");

                    let keyboard_plist_template = keyboard_path.join(INFO_PLIST);
                    let current_layout_path = keyboard_path.join(layout_folder_name.clone());

                    std::fs::create_dir_all(&current_layout_path).unwrap();

                    // KEYBOARD PLIST
                    let layout_info_plist_path = current_layout_path.join(INFO_PLIST);
                    generate_keyboard_plist(
                        keyboard_plist_template,
                        ios_keyboard_settings.clone(),
                        default_display_name.clone(),
                        layout_index,
                        language_tag.to_string(),
                        layout_info_plist_path.clone(),
                    );

                    // GENERATE .pbxproj
                    let temp = pbxproj.create_plist_file(&PathBuf::from_str(INFO_PLIST).unwrap());
                    pbxproj.add_path(&path_to_relative(&current_layout_path, REPOSITORY));
                    pbxproj.add_ref_to_group(
                        &temp,
                        &path_to_relative(&current_layout_path, REPOSITORY),
                    );
                    pbxproj.duplicate_target(
                        KEYBOARD,
                        &layout_folder_name,
                        &path_to_relative(&layout_info_plist_path, REPOSITORY),
                    );
                    pbxproj.set_target_build_configuration(
                        &layout_folder_name,
                        "PRODUCT_BUNDLE_IDENTIFIER",
                        &format!("{}.{layout_folder_name}", &target.package_id),
                    );

                    pbxproj.set_target_build_configuration(
                        &layout_folder_name,
                        "DEVELOPMENT_TEAM",
                        &target.code_sign_id.as_deref().unwrap_or("Unknown"),
                    );
                    pbxproj.add_appex_to_target_embedded_binaries(HOSTING_APP, &layout_folder_name);
                }
            }

            // HOSTING APP PLIST
            let hosting_app_plist_path = hosting_app_path.join(INFO_PLIST);
            generate_hosting_plist(
                hosting_app_plist_path,
                target.bundle_name.clone(),
                ios_keyboard_settings,
            );

            // NEW ENTITLEMENTS
            let new_entitlements = format!("{}.{}", "group", target.package_id);

            // UPDATE KEYBOARD ENTITLEMENTS
            let keyboard_entitlements_path =
                keyboard_path.join(format!("{}{}", KEYBOARD, ENTITLEMENTS_EXTENSION));
            update_entitlements(keyboard_entitlements_path, vec![new_entitlements.clone()]);

            // UPDATE HOSTING APP ENTITLEMENTS
            let hosting_app_entitlements_path =
                hosting_app_path.join(format!("{}{}", HOSTING_APP, ENTITLEMENTS_EXTENSION));
            update_entitlements(
                hosting_app_entitlements_path,
                vec![new_entitlements.clone()],
            );

            // UPDATE ENTITLEMENTS IN SETTINGS BUNDLE PLIST
            let root_plist_path = hosting_app_path.join(SETTINGS_BUNDLE).join(ROOT_PLIST);
            let mut root_plist: SettingsRootDict =
                plist::from_file(root_plist_path.clone()).expect("valid stuff");
            root_plist.application_group_container_identifier = new_entitlements.clone();
            plist::to_file_xml(root_plist_path.clone(), &root_plist).unwrap();

            // UPDATE PBXPROJ
            pbxproj.set_target_build_configuration(
                HOSTING_APP,
                "PRODUCT_BUNDLE_IDENTIFIER",
                &target.package_id,
            );
            pbxproj.set_target_build_configuration(
                HOSTING_APP,
                "DEVELOPMENT_TEAM",
                &target.code_sign_id.as_deref().unwrap_or("Unknown"),
            );
            pbxproj.remove_target(KEYBOARD);
            pbxproj.remove_appex_from_target_embedded_binaries(HOSTING_APP, KEYBOARD);
            pbxproj.update(HOSTING_APP, all_locales);
        }

        // GENERATE ICONS
        generate_icons(&bundle, &hosting_app_path);

        std::fs::write(pbxproj_path.clone(), pbxproj.to_pbxproj_string()).unwrap();
    }
}
