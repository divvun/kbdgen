//! The CLDR 48 keyboardTest3 vectors and the keyboards they test, as
//! vendored in `kbd-ldml` (`ldml.test.cldr`), embedded so that
//! `kbdgen ldml test --cldr` runs anywhere.

/// The CLDR release the files come from.
pub const RELEASE: u8 = 48;

/// `keyboards/test/*.xml`, by file name.
pub const TESTS: [(&str, &[u8]); 5] = [
    (
        "bn-test.xml",
        include_bytes!("../../../crates/kbd-ldml/testdata/cldr/48/keyboards/test/bn-test.xml"),
    ),
    (
        "fr-t-k0-test-test.xml",
        include_bytes!(
            "../../../crates/kbd-ldml/testdata/cldr/48/keyboards/test/fr-t-k0-test-test.xml"
        ),
    ),
    (
        "ja-Latn-test.xml",
        include_bytes!("../../../crates/kbd-ldml/testdata/cldr/48/keyboards/test/ja-Latn-test.xml"),
    ),
    (
        "pcm-test.xml",
        include_bytes!("../../../crates/kbd-ldml/testdata/cldr/48/keyboards/test/pcm-test.xml"),
    ),
    (
        "pt-t-k0-abnt2-test.xml",
        include_bytes!(
            "../../../crates/kbd-ldml/testdata/cldr/48/keyboards/test/pt-t-k0-abnt2-test.xml"
        ),
    ),
];

/// `keyboards/3.0/*.xml`, by file name.
pub const KEYBOARDS: [(&str, &[u8]); 9] = [
    (
        "bn.xml",
        include_bytes!("../../../crates/kbd-ldml/testdata/cldr/48/keyboards/3.0/bn.xml"),
    ),
    (
        "fr-t-k0-test.xml",
        include_bytes!("../../../crates/kbd-ldml/testdata/cldr/48/keyboards/3.0/fr-t-k0-test.xml"),
    ),
    (
        "fr.xml",
        include_bytes!("../../../crates/kbd-ldml/testdata/cldr/48/keyboards/3.0/fr.xml"),
    ),
    (
        "ja-Hira-t-k0-flicks.xml",
        include_bytes!(
            "../../../crates/kbd-ldml/testdata/cldr/48/keyboards/3.0/ja-Hira-t-k0-flicks.xml"
        ),
    ),
    (
        "ja-Latn.xml",
        include_bytes!("../../../crates/kbd-ldml/testdata/cldr/48/keyboards/3.0/ja-Latn.xml"),
    ),
    (
        "mt-t-k0-47key.xml",
        include_bytes!("../../../crates/kbd-ldml/testdata/cldr/48/keyboards/3.0/mt-t-k0-47key.xml"),
    ),
    (
        "mt.xml",
        include_bytes!("../../../crates/kbd-ldml/testdata/cldr/48/keyboards/3.0/mt.xml"),
    ),
    (
        "pcm.xml",
        include_bytes!("../../../crates/kbd-ldml/testdata/cldr/48/keyboards/3.0/pcm.xml"),
    ),
    (
        "pt-t-k0-abnt2.xml",
        include_bytes!("../../../crates/kbd-ldml/testdata/cldr/48/keyboards/3.0/pt-t-k0-abnt2.xml"),
    ),
];
