use std::path::{Path, PathBuf};

use indexmap::IndexMap;
use language_tags::LanguageTag;

// [spec:kbdgen:def:bundle.resources]
#[derive(Debug, Default)]
pub struct Resources {
    pub(crate) macos: Option<MacOS>,
    pub(crate) chromeos: Option<ChromeOS>,
    pub(crate) android: Option<Android>,
    pub(crate) ios: Option<IOS>,
}

#[derive(Debug, Default)]
pub(crate) struct MacOS {
    pub(crate) icons: IndexMap<LanguageTag, PathBuf>,
}

impl MacOS {
    pub fn load(path: &Path) -> Result<Self, std::io::Error> {
        let icons = std::fs::read_dir(path)?
            .filter_map(Result::ok)
            .map(|x| x.path())
            .filter_map(|x| {
                let filename = x
                    .file_name()
                    .unwrap()
                    .to_str()
                    .expect("file name must be stringable");
                if filename.starts_with("icon.") {
                    let lang_tag: LanguageTag =
                        filename.split(".").skip(1).next().unwrap().parse().unwrap();
                    Some((lang_tag, x))
                } else {
                    None
                }
            })
            .collect();

        Ok(Self { icons })
    }
}

#[derive(Debug, Default)]
pub(crate) struct IOS {
    pub(crate) icons: IndexMap<LanguageTag, PathBuf>,
}

impl IOS {
    pub fn load(path: &Path) -> Result<Self, std::io::Error> {
        let icons = std::fs::read_dir(path)?
            .filter_map(Result::ok)
            .map(|x| x.path())
            .filter_map(|x| {
                let filename = x
                    .file_name()
                    .unwrap()
                    .to_str()
                    .expect("file name must be stringable");
                if filename.starts_with("icon.") {
                    let lang_tag: LanguageTag =
                        filename.split(".").skip(1).next().unwrap().parse().unwrap();
                    Some((lang_tag, x))
                } else {
                    None
                }
            })
            .collect();

        Ok(Self { icons })
    }
}

#[derive(Debug, Default)]
pub(crate) struct ChromeOS {
    #[allow(dead_code)]
    pub(crate) icons: IndexMap<LanguageTag, PathBuf>,
}

impl ChromeOS {
    pub fn load(path: &Path) -> Result<Self, std::io::Error> {
        let icons = std::fs::read_dir(path)?
            .filter_map(Result::ok)
            .map(|x| x.path())
            .filter_map(|x| {
                let filename = x
                    .file_name()
                    .unwrap()
                    .to_str()
                    .expect("file name must be stringable");
                if filename.starts_with("icon.") {
                    let lang_tag: LanguageTag =
                        filename.split(".").skip(1).next().unwrap().parse().unwrap();
                    Some((lang_tag, x))
                } else {
                    None
                }
            })
            .collect();

        Ok(Self { icons })
    }
}

#[derive(Debug, Default)]
pub(crate) struct Android {
    pub(crate) icon: Option<PathBuf>,
}

impl Android {
    pub fn load(path: &Path) -> Result<Self, std::io::Error> {
        let icon_path = path.join("icon.png");

        let icon = if icon_path.exists() {
            Some(icon_path)
        } else {
            None
        };

        Ok(Self { icon })
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use indexmap::IndexMap;
    use language_tags::LanguageTag;

    use crate::bundle::{KbdgenBundle, fixture, read_kbdgen_bundle};

    fn load_with_resources(files: &[&str]) -> (tempfile::TempDir, KbdgenBundle) {
        let root = tempfile::tempdir().unwrap();
        let path = fixture::write_bundle(root.path(), "sme", &[], &[], &[]);
        for file in files {
            fixture::write_file(&path, &format!("resources/{file}"), "");
        }
        let bundle = read_kbdgen_bundle(&path).unwrap();
        (root, bundle)
    }

    fn sorted_icons(icons: &IndexMap<LanguageTag, PathBuf>) -> Vec<(String, PathBuf)> {
        let mut icons: Vec<_> = icons
            .iter()
            .map(|(tag, path)| (tag.to_string(), path.clone()))
            .collect();
        icons.sort();
        icons
    }

    // [spec:kbdgen:def:bundle.resources/test]
    #[test]
    fn icons_are_keyed_by_second_dot_segment() {
        let mut files = Vec::new();
        for target in ["macos", "ios", "chromeos"] {
            for name in ["icon.se.png", "icon.sma-NO.icns", "icon.png", "logo.png"] {
                files.push(format!("{target}/{name}"));
            }
        }
        let files: Vec<&str> = files.iter().map(String::as_str).collect();
        let (_root, bundle) = load_with_resources(&files);
        let expected = |target: &str| -> Vec<(String, PathBuf)> {
            let dir = bundle.path.join("resources").join(target);
            [
                ("png", "icon.png"),
                ("se", "icon.se.png"),
                ("sma-NO", "icon.sma-NO.icns"),
            ]
            .into_iter()
            .map(|(tag, file)| (tag.to_string(), dir.join(file)))
            .collect()
        };

        let resources = &bundle.resources;
        assert_eq!(
            sorted_icons(&resources.macos.as_ref().unwrap().icons),
            expected("macos")
        );
        assert_eq!(
            sorted_icons(&resources.ios.as_ref().unwrap().icons),
            expected("ios")
        );
        assert_eq!(
            sorted_icons(&resources.chromeos.as_ref().unwrap().icons),
            expected("chromeos")
        );
        assert!(bundle.resources.android.is_none());
    }

    // [spec:kbdgen:def:bundle.resources/test]
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

    // [spec:kbdgen:def:bundle.resources/test]
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

    // [spec:kbdgen:def:bundle.resources/test]
    #[test]
    #[should_panic]
    fn malformed_icon_tag_panics() {
        let _ = load_with_resources(&["macos/icon.se_FI.png"]);
    }
}
