use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context, Result};
use async_trait::async_trait;
use language_tags::LanguageTag;
use rayon::prelude::*;

use crate::{
    build::{
        BuildStep,
        ios::{
            ensure_success, ios_layouts, keyboard_name, pbxproj::Pbxproj, pkg_id,
            xcode_structures::*,
        },
    },
    bundle::{KbdgenBundle, layout::IOsTarget, project::Project, target::iOS},
};

const REPOSITORY: &str = "repo";
const HOSTING_APP: &str = "HostingApp";
const HOSTING_INFO_STRINGS: &str = "InfoPlist.strings";

const KEYBOARD: &str = "Keyboard";
const INFO_PLIST: &str = "Info.plist";

const SETTINGS_BUNDLE: &str = "Settings.bundle";
const ROOT_PLIST: &str = "Root.plist";

const DEFAULT_LOCALE: &str = "en";

/// Xcode calls the development language's localization `Base`.
fn xcode_locale(locale: &str) -> &str {
    if locale == DEFAULT_LOCALE {
        "Base"
    } else {
        locale
    }
}

fn read_plist<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    plist::from_file(path).with_context(|| format!("read {}", path.display()))
}

fn write_plist<T: serde::Serialize>(path: &Path, value: &T) -> Result<()> {
    tracing::debug!("Writing {}", path.display());
    plist::to_file_xml(path, value).with_context(|| format!("write {}", path.display()))
}

// [spec:kbdgen:req:xcode.targets.keyboard-plist+2]
fn keyboard_plist(
    template: &KeyboardInfoPlist,
    target: &iOS,
    ios_layout: &IOsTarget,
    keyboard_index: usize,
    primary_language: &LanguageTag,
    display_name: &str,
    contact_email: Option<String>,
) -> Result<KeyboardInfoPlist> {
    let mut plist = template.clone();

    let speller = &ios_layout.config;
    if let (Some(key), Some(path)) = (&speller.speller_package_key, &speller.speller_path) {
        plist.divvun_speller_package_key = Some(key.clone());
        plist.divvun_speller_path = Some(path.clone());
    } else {
        plist.divvun_speller_package_key = None;
        plist.divvun_speller_path = None;
    }

    if let Some(email) = contact_email {
        plist.divvun_contact_email = email;
    }

    plist.cf_bundle_display_name = display_name.to_string();
    plist.cf_bundle_short_version_string = target.version.clone();
    plist.cf_bundle_version = target.build.to_string();
    *plist
        .ls_application_queries_schemes
        .first_mut()
        .context("Keyboard/Info.plist has no LSApplicationQueriesSchemes")? =
        target.package_id.clone();
    plist.sentry_dsn = target.sentry_dsn.clone();
    plist.ns_extension.ns_extension_attributes.primary_language = primary_language.to_string();
    plist.divvun_keyboard_index = keyboard_index;

    Ok(plist)
}

// [spec:kbdgen:req:xcode.targets.hosting-app+2]
fn generate_hosting_plist(path: &Path, target: &iOS) -> Result<()> {
    let mut plist: HostingPlist = read_plist(path)?;

    plist.cf_bundle_display_name = target.bundle_name.clone();
    plist.cf_bundle_short_version_string = target.version.clone();
    plist.cf_bundle_version = target.build.to_string();
    *plist
        .cf_bundle_url_types
        .first_mut()
        .and_then(|url_type| url_type.cf_bundle_url_schemes.first_mut())
        .with_context(|| format!("{} has no CFBundleURLSchemes", path.display()))? =
        target.package_id.clone();
    *plist
        .ls_application_queries_schemes
        .first_mut()
        .with_context(|| format!("{} has no LSApplicationQueriesSchemes", path.display()))? =
        target.package_id.clone();
    plist.sentry_dsn = target.sentry_dsn.clone();

    write_plist(path, &plist)
}

// [spec:kbdgen:req:xcode.targets.hosting-app+2]
fn update_entitlements(path: &Path, app_group: &str) -> Result<()> {
    let mut entitlements: EntitlementsDict = read_plist(path)?;
    entitlements.com_apple_security_application_groups = vec![app_group.to_string()];
    write_plist(path, &entitlements)
}

// [spec:kbdgen:req:xcode.targets.hosting-app+2]
fn generate_icons(bundle: &KbdgenBundle, hosting_app_path: &Path) -> Result<()> {
    let icon = bundle
        .resources
        .ios
        .as_ref()
        .context("the bundle has no resources/ios directory")?
        .icon
        .as_ref()
        .context("resources/ios has no default icon (icon.png)")?;

    let appiconset_path = hosting_app_path.join("Images.xcassets/AppIcon.appiconset");
    let contents_path = appiconset_path.join("Contents.json");
    let contents = fs::read_to_string(&contents_path)
        .with_context(|| format!("read {}", contents_path.display()))?;
    let mut contents: AppIconSetContents = serde_json::from_str(&contents)
        .with_context(|| format!("parse {}", contents_path.display()))?;

    tracing::info!("Generating app icons from {}", icon.display());
    contents
        .images
        .par_iter_mut()
        .try_for_each(|image| -> Result<()> {
            let filename = format!("{}-{}@{}.png", image.idiom, image.size, image.scale);
            let side = icon_side(&image.size, &image.scale).with_context(|| {
                format!("{}: unsupported icon {filename}", contents_path.display())
            })?;
            let geometry = format!("{side}x{side}");
            let icon_path = appiconset_path.join(&filename);
            tracing::debug!("Writing {}", icon_path.display());

            let output = Command::new("convert")
                .arg("-resize")
                .arg(&geometry)
                .args([
                    "-background",
                    "transparent",
                    "-gravity",
                    "center",
                    "-extent",
                ])
                .arg(&geometry)
                .arg(icon)
                .arg(&icon_path)
                .output()
                .context("`convert` could not be started; install ImageMagick")?;
            ensure_success(&format!("convert {}", icon_path.display()), &output)?;

            image.filename = Some(filename);
            Ok(())
        })?;

    let json = serde_json::to_string_pretty(&contents).expect("icon set contents serialize");
    fs::write(&contents_path, json).with_context(|| format!("write {}", contents_path.display()))
}

/// The pixel width of an icon from its asset catalog `size` (`83.5x83.5`) and `scale` (`2x`).
fn icon_side(size: &str, scale: &str) -> Option<f32> {
    let width: f32 = size.split('x').next()?.parse().ok()?;
    let scale: f32 = scale.strip_suffix('x')?.parse().ok()?;
    Some(width * scale)
}

/// The contact email in the layout's `projects/<tag>.yaml`. Only layouts fetched from a
/// keyboard repository have one; those in the bundle's own `layouts` folder do not.
fn layout_contact_email(bundle: &KbdgenBundle, tag: &LanguageTag) -> Option<String> {
    let path = bundle.path.join("projects").join(format!("{tag}.yaml"));
    let content = match fs::read_to_string(&path) {
        Ok(content) => content,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return None,
        Err(err) => {
            tracing::warn!(
                "Ignoring {}, so {tag} keeps the template's contact email: {err}",
                path.display()
            );
            return None;
        }
    };
    match crate::yaml::from_str::<Project>(&content) {
        Ok(project) => Some(project.email),
        Err(err) => {
            tracing::warn!(
                "Ignoring {}, so {tag} keeps the template's contact email: {err}",
                path.display()
            );
            None
        }
    }
}

struct Paths {
    repository: PathBuf,
    hosting_app: PathBuf,
    keyboard: PathBuf,
}

/// Adds a keyboard target per iOS layout, configures the hosting app and localizations, and
/// removes the template's `Keyboard` target.
fn configure_targets(
    bundle: &KbdgenBundle,
    target: &iOS,
    paths: &Paths,
    pbxproj: &mut Pbxproj,
) -> Result<()> {
    let development_team = target.team_id.as_deref().unwrap_or("Unknown");

    // [spec:kbdgen:req:xcode.localization+1]
    for (locale, info) in &bundle.project.locales {
        let locale = xcode_locale(locale);
        let locale_path = paths.hosting_app.join(format!("{locale}.lproj"));
        fs::create_dir_all(&locale_path)
            .with_context(|| format!("create {}", locale_path.display()))?;

        let info_strings = XcodeHostingInfoStrings {
            cf_bundle_name: info.name.clone(),
            cf_bundle_display_name: info.name.clone(),
        };
        let info_strings_path = locale_path.join(HOSTING_INFO_STRINGS);
        tracing::debug!("Writing {}", info_strings_path.display());
        fs::write(&info_strings_path, info_strings.to_string())
            .with_context(|| format!("write {}", info_strings_path.display()))?;

        if locale != "Base" && locale_path.join("About.txt").exists() {
            pbxproj.add_localized_about(locale)?;
        }
    }

    let template_path = paths.keyboard.join(INFO_PLIST);
    let template: KeyboardInfoPlist = read_plist(&template_path)?;
    let default_tag: LanguageTag = DEFAULT_LOCALE
        .parse()
        .expect("default language tag must parse");

    // Must use the same list as GenerateIos: `layout_index` becomes DivvunKeyboardIndex, which the
    // keyboard uses to find its entry in KeyboardDefinitions.json.
    for (layout_index, (language_tag, layout)) in ios_layouts(bundle)?.into_iter().enumerate() {
        let ios_layout = layout
            .i_os
            .as_ref()
            .expect("ios_layouts only returns layouts with an iOS target");
        let display_name = layout
            .display_names
            .get(&default_tag)
            .expect("ios_layouts rejects layouts without an English display name");
        let pkg_id = pkg_id(target, layout);
        let name = keyboard_name(&pkg_id);
        tracing::info!("Adding keyboard target {name} for {language_tag}");

        let layout_dir = Path::new(KEYBOARD).join(name);
        let layout_path = paths.repository.join(&layout_dir);
        fs::create_dir_all(&layout_path)
            .with_context(|| format!("create {}", layout_path.display()))?;

        let plist = keyboard_plist(
            &template,
            target,
            ios_layout,
            layout_index,
            language_tag,
            display_name,
            layout_contact_email(bundle, language_tag),
        )?;
        write_plist(&layout_path.join(INFO_PLIST), &plist)?;

        // [spec:kbdgen:req:xcode.targets+2]
        let plist_ref = pbxproj.create_plist_file_reference(Path::new(INFO_PLIST));
        pbxproj.add_path(&layout_dir)?;
        pbxproj.add_ref_to_group(&plist_ref, &layout_dir)?;
        pbxproj.duplicate_target(KEYBOARD, name, &layout_dir.join(INFO_PLIST))?;
        pbxproj.set_build_setting(name, "PRODUCT_BUNDLE_IDENTIFIER", &pkg_id)?;
        pbxproj.set_build_setting(name, "DEVELOPMENT_TEAM", development_team)?;
        pbxproj.add_appex_to_target_embedded_binaries(HOSTING_APP, name)?;
    }

    generate_hosting_plist(&paths.hosting_app.join(INFO_PLIST), target)?;

    let app_group = format!("group.{}", target.package_id);
    update_entitlements(
        &paths.keyboard.join(format!("{KEYBOARD}.entitlements")),
        &app_group,
    )?;
    update_entitlements(
        &paths
            .hosting_app
            .join(format!("{HOSTING_APP}.entitlements")),
        &app_group,
    )?;

    let root_plist_path = paths.hosting_app.join(SETTINGS_BUNDLE).join(ROOT_PLIST);
    let mut root_plist: SettingsRootDict = read_plist(&root_plist_path)?;
    root_plist.application_group_container_identifier = app_group;
    write_plist(&root_plist_path, &root_plist)?;

    // [spec:kbdgen:req:xcode.targets+2]
    pbxproj.set_build_setting(HOSTING_APP, "PRODUCT_BUNDLE_IDENTIFIER", &target.package_id)?;
    pbxproj.set_build_setting(HOSTING_APP, "DEVELOPMENT_TEAM", development_team)?;
    pbxproj.remove_target(KEYBOARD)?;
    pbxproj.remove_appex_from_target_embedded_binaries(HOSTING_APP, KEYBOARD)?;

    // [spec:kbdgen:req:xcode.localization+1]
    let locales: BTreeSet<String> = bundle
        .project
        .locales
        .keys()
        .map(|locale| xcode_locale(locale).to_string())
        .collect();
    pbxproj.add_locales(HOSTING_APP, &locales)
}

pub struct GenerateXcode;

#[async_trait(?Send)]
impl BuildStep for GenerateXcode {
    async fn build(&self, bundle: &KbdgenBundle, output_path: &Path) -> Result<()> {
        let repository = output_path.join(REPOSITORY);
        let paths = Paths {
            hosting_app: repository.join(HOSTING_APP),
            keyboard: repository.join(KEYBOARD),
            repository,
        };
        let pbxproj_path = paths
            .repository
            .join("GiellaKeyboard.xcodeproj/project.pbxproj");

        tracing::info!(
            "Generating the Xcode project in {}",
            paths.repository.display()
        );
        let mut pbxproj = Pbxproj::from_path(&pbxproj_path)?;

        if let Some(target) = &bundle.targets.ios {
            configure_targets(bundle, target, &paths, &mut pbxproj)?;
        }

        generate_icons(bundle, &paths.hosting_app)?;

        tracing::debug!("Writing {}", pbxproj_path.display());
        fs::write(&pbxproj_path, pbxproj.to_pbxproj_string())
            .with_context(|| format!("write {}", pbxproj_path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bundle::layout::IOsConfig;

    fn template(queries_schemes: &[&str]) -> KeyboardInfoPlist {
        serde_json::from_value(serde_json::json!({
            "DivvunSpellerPath": "template.bhfst",
            "DivvunSpellerPackageKey": "template",
            "DivvunContactEmail": "template@example.com",
            "CFBundleDevelopmentRegion": "en",
            "CFBundleDisplayName": "Template",
            "CFBundleExecutable": "$(EXECUTABLE_NAME)",
            "CFBundleIdentifier": "$(PRODUCT_BUNDLE_IDENTIFIER)",
            "CFBundleInfoDictionaryVersion": "6.0",
            "CFBundleName": "$(PRODUCT_NAME)",
            "CFBundlePackageType": "XPC!",
            "CFBundleShortVersionString": "0.0",
            "CFBundleSignature": "????",
            "CFBundleVersion": "0",
            "DivvunKeyboardIndex": 0,
            "ITSAppUsesNonExemptEncryption": false,
            "LSApplicationQueriesSchemes": queries_schemes,
            "NSExtension": {
                "NSExtensionAttributes": {
                    "IsASCIICapable": false,
                    "PrefersRightToLeft": false,
                    "PrimaryLanguage": "en",
                    "RequestsOpenAccess": true
                },
                "NSExtensionPointIdentifier": "com.apple.keyboard-service",
                "NSExtensionPrincipalClass": "KeyboardViewController"
            },
            "SentryDSN": "https://template@sentry.example"
        }))
        .unwrap()
    }

    fn target() -> iOS {
        crate::yaml::from_str(
            "packageId: com.example.keyboards\nbundleName: Keyboards\nversion: '2.1'\nbuild: 7\n",
        )
        .unwrap()
    }

    fn ios_layout(speller_package_key: Option<&str>) -> IOsTarget {
        IOsTarget {
            config: IOsConfig {
                speller_package_key: speller_package_key.map(str::to_string),
                speller_path: Some("se.bhfst".to_string()),
            },
            primary: None,
            i_pad_9in: None,
            i_pad_12in: None,
            dead_keys: None,
        }
    }

    // [spec:kbdgen:req:xcode.targets.keyboard-plist+2/test]
    #[test]
    fn keyboard_plist_fills_in_the_layout() {
        let se: LanguageTag = "se".parse().unwrap();

        let plist = keyboard_plist(
            &template(&["template"]),
            &target(),
            &ios_layout(Some("se")),
            3,
            &se,
            "Northern Sami",
            Some("se@example.com".to_string()),
        )
        .unwrap();

        assert_eq!(plist.divvun_speller_package_key.as_deref(), Some("se"));
        assert_eq!(plist.divvun_speller_path.as_deref(), Some("se.bhfst"));
        assert_eq!(plist.divvun_contact_email, "se@example.com");
        assert_eq!(plist.cf_bundle_display_name, "Northern Sami");
        assert_eq!(plist.cf_bundle_short_version_string, "2.1");
        assert_eq!(plist.cf_bundle_version, "7");
        assert_eq!(
            plist.ls_application_queries_schemes,
            ["com.example.keyboards"]
        );
        assert_eq!(plist.sentry_dsn, None);
        assert_eq!(
            plist.ns_extension.ns_extension_attributes.primary_language,
            "se"
        );
        assert_eq!(plist.divvun_keyboard_index, 3);

        let unchanged = keyboard_plist(
            &template(&["template"]),
            &target(),
            &ios_layout(None),
            0,
            &se,
            "Northern Sami",
            None,
        )
        .unwrap();
        assert_eq!(unchanged.divvun_speller_package_key, None);
        assert_eq!(unchanged.divvun_speller_path, None);
        assert_eq!(unchanged.divvun_contact_email, "template@example.com");
    }

    // [spec:kbdgen:req:xcode.targets.keyboard-plist+2/test]
    #[test]
    fn keyboard_plist_needs_a_queries_scheme_to_replace() {
        let se: LanguageTag = "se".parse().unwrap();

        let error = keyboard_plist(
            &template(&[]),
            &target(),
            &ios_layout(None),
            0,
            &se,
            "Northern Sami",
            None,
        )
        .unwrap_err();

        assert_eq!(
            error.to_string(),
            "Keyboard/Info.plist has no LSApplicationQueriesSchemes"
        );
    }

    // [spec:kbdgen:req:xcode.targets.hosting-app+2/test]
    #[test]
    fn icons_need_ios_resources() {
        let bundle = KbdgenBundle::new_test("test".to_string(), Default::default());
        let output = tempfile::tempdir().unwrap();

        let error = generate_icons(&bundle, output.path()).unwrap_err();

        assert_eq!(
            error.to_string(),
            "the bundle has no resources/ios directory"
        );
    }

    // [spec:kbdgen:req:xcode.targets.hosting-app+2/test]
    #[test]
    fn icon_sides_are_size_times_scale() {
        assert_eq!(icon_side("83.5x83.5", "2x"), Some(167.0));
        assert_eq!(icon_side("1024x1024", "1x"), Some(1024.0));
        assert_eq!(icon_side("20x20", "2"), None);
        assert_eq!(icon_side("big", "2x"), None);
    }
}
