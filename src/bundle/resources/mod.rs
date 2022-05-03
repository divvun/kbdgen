use std::path::{Path, PathBuf};

use indexmap::IndexMap;
use language_tags::LanguageTag;

#[derive(Debug, Default)]
pub struct Resources {
    pub(crate) macos: Option<MacOS>,
    pub(crate) chromeos: Option<ChromeOS>,
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
pub(crate) struct ChromeOS {
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