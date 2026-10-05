//! The CLDR import files embedded for `base="cldr"` (`ldml.xml.cldr-data`).
//!
//! Each release's `keyboards/import/*.xml` is kept under its major version,
//! unchanged and under Unicode-3.0 (`data/cldr/LICENSE`). A release that
//! changes the keyboard DTD adds a directory here.

/// (path, contents), with paths as `base="cldr"` writes them.
const FILES: &[(&str, &str)] = &[
    (
        "45/keys-Latn-implied.xml",
        include_str!("../data/cldr/45/keys-Latn-implied.xml"),
    ),
    (
        "45/keys-Zyyy-currency.xml",
        include_str!("../data/cldr/45/keys-Zyyy-currency.xml"),
    ),
    (
        "45/keys-Zyyy-punctuation.xml",
        include_str!("../data/cldr/45/keys-Zyyy-punctuation.xml"),
    ),
    (
        "45/scanCodes-implied.xml",
        include_str!("../data/cldr/45/scanCodes-implied.xml"),
    ),
    (
        "46/keys-Latn-implied.xml",
        include_str!("../data/cldr/46/keys-Latn-implied.xml"),
    ),
    (
        "46/keys-Zyyy-currency.xml",
        include_str!("../data/cldr/46/keys-Zyyy-currency.xml"),
    ),
    (
        "46/keys-Zyyy-punctuation.xml",
        include_str!("../data/cldr/46/keys-Zyyy-punctuation.xml"),
    ),
    (
        "46/scanCodes-implied.xml",
        include_str!("../data/cldr/46/scanCodes-implied.xml"),
    ),
    (
        "47/keys-Latn-implied.xml",
        include_str!("../data/cldr/47/keys-Latn-implied.xml"),
    ),
    (
        "47/keys-Zyyy-currency.xml",
        include_str!("../data/cldr/47/keys-Zyyy-currency.xml"),
    ),
    (
        "47/keys-Zyyy-punctuation.xml",
        include_str!("../data/cldr/47/keys-Zyyy-punctuation.xml"),
    ),
    (
        "47/scanCodes-implied.xml",
        include_str!("../data/cldr/47/scanCodes-implied.xml"),
    ),
    (
        "48/keys-Latn-implied.xml",
        include_str!("../data/cldr/48/keys-Latn-implied.xml"),
    ),
    (
        "48/keys-Zyyy-currency.xml",
        include_str!("../data/cldr/48/keys-Zyyy-currency.xml"),
    ),
    (
        "48/keys-Zyyy-punctuation.xml",
        include_str!("../data/cldr/48/keys-Zyyy-punctuation.xml"),
    ),
    (
        "48/scanCodes-implied.xml",
        include_str!("../data/cldr/48/scanCodes-implied.xml"),
    ),
];

/// The embedded CLDR major versions, ascending.
pub const VERSIONS: [u8; 4] = [45, 46, 47, 48];

// [spec:kbdgen:def:ldml.xml.cldr-data]
/// The embedded file at `path`, such as `45/keys-Zyyy-punctuation.xml`.
pub fn file(path: &str) -> Option<&'static str> {
    FILES
        .iter()
        .find(|(p, _)| *p == path)
        .map(|(_, text)| *text)
}

/// The newest embedded version at most `version`, used for the implied
/// keys and forms of a keyboard conforming to `version`; the oldest when
/// none is.
pub fn implied_version(version: u8) -> u8 {
    VERSIONS
        .iter()
        .rev()
        .find(|v| **v <= version)
        .copied()
        .unwrap_or(VERSIONS[0])
}

#[cfg(test)]
mod tests {
    use super::*;

    // [spec:kbdgen:def:ldml.xml.cldr-data/test]
    #[test]
    fn every_release_embeds_its_imports() {
        for v in VERSIONS {
            for f in [
                "keys-Latn-implied",
                "keys-Zyyy-currency",
                "keys-Zyyy-punctuation",
                "scanCodes-implied",
            ] {
                let text = file(&format!("{v}/{f}.xml")).unwrap();
                assert!(text.contains("<?xml"), "{v}/{f}");
            }
        }
        assert!(file("44/keys-Latn-implied.xml").is_none());
        assert!(file("45/../45/keys-Latn-implied.xml").is_none());
        assert_eq!(implied_version(49), 48);
        assert_eq!(implied_version(46), 46);
        let punctuation = file("45/keys-Zyyy-punctuation.xml").unwrap();
        assert!(punctuation.contains("SPDX-License-Identifier: Unicode-3.0"));
    }
}
