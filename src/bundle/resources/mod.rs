use std::path::{Path, PathBuf};

use indexmap::IndexMap;
use language_tags::LanguageTag;

use super::Error;

// [spec:kbdgen:def:bundle.resources+1]
#[derive(Debug, Default)]
pub struct Resources {
    pub(crate) macos: Option<MacOS>,
    pub(crate) chromeos: Option<ChromeOS>,
    pub(crate) android: Option<Android>,
    pub(crate) ios: Option<IOS>,
}

/// The icons in one target's resource folder: `icon.<ext>` is the default
/// icon and `icon.<tag>.<ext>` the icon of the layouts tagged `<tag>`.
#[derive(Debug, Default)]
struct Icons {
    default: Option<PathBuf>,
    by_tag: IndexMap<LanguageTag, PathBuf>,
}

// [spec:kbdgen:def:bundle.resources+1]
fn read_icons(dir: &Path) -> Result<Icons, Error> {
    let io = |e| Error::Io(dir.to_path_buf(), e);
    let mut paths = std::fs::read_dir(dir)
        .map_err(io)?
        .map(|entry| entry.map(|entry| entry.path()).map_err(io))
        .collect::<Result<Vec<_>, _>>()?;
    paths.sort();

    let mut icons = Icons::default();
    for path in paths.into_iter().filter(|path| path.is_file()) {
        let name = path
            .file_name()
            .expect("read_dir entries have a file name")
            .to_string_lossy()
            .into_owned();
        let Some(rest) = name.strip_prefix("icon.") else {
            continue;
        };
        let (previous, which) = match rest.rsplit_once('.') {
            None => (icons.default.replace(path.clone()), "default".to_owned()),
            Some((tag, _extension)) => {
                let tag: LanguageTag = tag.parse().map_err(|_| Error::IconTag {
                    path: path.clone(),
                    tag: tag.to_owned(),
                })?;
                let which = tag.to_string();
                (icons.by_tag.insert(tag, path.clone()), which)
            }
        };
        if let Some(first) = previous {
            return Err(Error::DuplicateIcon {
                which,
                first,
                second: path,
            });
        }
    }
    Ok(icons)
}

#[derive(Debug, Default)]
pub(crate) struct MacOS {
    pub(crate) icons: IndexMap<LanguageTag, PathBuf>,
    pub(crate) default_icon: Option<PathBuf>,
}

impl MacOS {
    pub fn load(path: &Path) -> Result<Self, Error> {
        let icons = read_icons(path)?;
        Ok(Self {
            icons: icons.by_tag,
            default_icon: icons.default,
        })
    }
}

/// iOS has one app icon for all its keyboards: the default icon.
#[derive(Debug, Default)]
pub(crate) struct IOS {
    pub(crate) icon: Option<PathBuf>,
}

impl IOS {
    pub fn load(path: &Path) -> Result<Self, Error> {
        Ok(Self {
            icon: read_icons(path)?.default,
        })
    }
}

#[derive(Debug, Default)]
#[allow(dead_code)]
pub(crate) struct ChromeOS {
    pub(crate) icons: IndexMap<LanguageTag, PathBuf>,
    pub(crate) default_icon: Option<PathBuf>,
}

impl ChromeOS {
    pub fn load(path: &Path) -> Result<Self, Error> {
        let icons = read_icons(path)?;
        Ok(Self {
            icons: icons.by_tag,
            default_icon: icons.default,
        })
    }
}

#[derive(Debug, Default)]
pub(crate) struct Android {
    pub(crate) icon: Option<PathBuf>,
}

impl Android {
    pub fn load(path: &Path) -> Self {
        let icon = path.join("icon.png");
        Self {
            icon: icon.is_file().then_some(icon),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use indexmap::IndexMap;
    use language_tags::LanguageTag;

    use crate::bundle::{Error, KbdgenBundle, fixture, read_kbdgen_bundle};

    fn bundle_with_resources(files: &[&str]) -> (tempfile::TempDir, PathBuf) {
        let root = tempfile::tempdir().unwrap();
        let path = fixture::write_bundle(root.path(), "sme", &[], &[], &[]);
        for file in files {
            fixture::write_file(&path, &format!("resources/{file}"), "");
        }
        (root, path)
    }

    fn load_with_resources(files: &[&str]) -> (tempfile::TempDir, KbdgenBundle) {
        let (root, path) = bundle_with_resources(files);
        let bundle = read_kbdgen_bundle(&path).unwrap();
        (root, bundle)
    }

    fn load_error(files: &[&str]) -> Error {
        let (_root, path) = bundle_with_resources(files);
        read_kbdgen_bundle(&path).unwrap_err()
    }

    fn sorted_icons(icons: &IndexMap<LanguageTag, PathBuf>) -> Vec<(String, PathBuf)> {
        let mut icons: Vec<_> = icons
            .iter()
            .map(|(tag, path)| (tag.to_string(), path.clone()))
            .collect();
        icons.sort();
        icons
    }

    // [spec:kbdgen:def:bundle.resources+1/test]
    #[test]
    fn icons_are_the_default_or_keyed_by_tag() {
        let mut files = Vec::new();
        for target in ["macos", "ios", "chromeos"] {
            for name in ["icon.se.png", "icon.sma-NO.icns", "icon.png", "logo.png"] {
                files.push(format!("{target}/{name}"));
            }
        }
        let files: Vec<&str> = files.iter().map(String::as_str).collect();
        let (_root, bundle) = load_with_resources(&files);
        let dir = |target: &str| bundle.path.join("resources").join(target);
        let tagged = |target: &str| -> Vec<(String, PathBuf)> {
            [("se", "icon.se.png"), ("sma-NO", "icon.sma-NO.icns")]
                .into_iter()
                .map(|(tag, file)| (tag.to_string(), dir(target).join(file)))
                .collect()
        };

        let resources = &bundle.resources;
        let macos = resources.macos.as_ref().unwrap();
        assert_eq!(sorted_icons(&macos.icons), tagged("macos"));
        assert_eq!(macos.default_icon, Some(dir("macos").join("icon.png")));
        let chromeos = resources.chromeos.as_ref().unwrap();
        assert_eq!(sorted_icons(&chromeos.icons), tagged("chromeos"));
        assert_eq!(
            chromeos.default_icon,
            Some(dir("chromeos").join("icon.png"))
        );
        assert_eq!(
            resources.ios.as_ref().unwrap().icon,
            Some(dir("ios").join("icon.png"))
        );
        assert!(bundle.resources.android.is_none());
    }

    // [spec:kbdgen:def:bundle.resources+1/test]
    #[test]
    fn chrome_and_chromeos_are_one_folder() {
        let (_root, bundle) = load_with_resources(&["chrome/icon.png"]);
        assert_eq!(
            bundle.resources.chromeos.unwrap().default_icon,
            Some(bundle.path.join("resources/chrome/icon.png"))
        );

        let error = load_error(&["chrome/icon.png", "chromeos/icon.png"]);
        assert!(matches!(error, Error::DuplicateResources { .. }), "{error}");
    }

    // [spec:kbdgen:def:bundle.resources+1/test]
    #[test]
    fn android_resource_is_only_icon_png() {
        let (_root, bundle) = load_with_resources(&["android/icon.png", "android/icon.se.png"]);

        let android = bundle.resources.android.as_ref().unwrap();
        assert_eq!(
            android.icon.as_deref(),
            Some(bundle.path.join("resources/android/icon.png").as_path())
        );

        let (_root, bundle) = load_with_resources(&["android/icon.se.png"]);

        assert_eq!(bundle.resources.android.unwrap().icon, None);
    }

    // [spec:kbdgen:def:bundle.resources+1/test]
    #[test]
    fn unrecognised_directories_and_plain_files_are_ignored() {
        let (_root, bundle) = load_with_resources(&[
            "icon.png",
            "macOS-extra/icon.se_FI.png",
            "windows/icon.se_FI.png",
        ]);

        assert!(bundle.resources.macos.is_none());
        assert!(bundle.resources.ios.is_none());
        assert!(bundle.resources.chromeos.is_none());
        assert!(bundle.resources.android.is_none());
    }

    // [spec:kbdgen:def:bundle.resources+1/test]
    #[test]
    fn malformed_and_duplicate_icons_are_errors() {
        let error = load_error(&["macos/icon.se_FI.png"]);
        assert!(
            matches!(&error, Error::IconTag { tag, .. } if tag == "se_FI"),
            "{error}"
        );

        let error = load_error(&["ios/icon.se.png", "ios/icon.se.icns"]);
        assert!(
            matches!(&error, Error::DuplicateIcon { which, .. } if which == "se"),
            "{error}"
        );

        let error = load_error(&["chromeos/icon.png", "chromeos/icon.svg"]);
        assert!(
            matches!(&error, Error::DuplicateIcon { which, .. } if which == "default"),
            "{error}"
        );
    }
}
