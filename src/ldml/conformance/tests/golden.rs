//! The golden vectors (`ldml.test.golden`): every file passes, and every
//! item the rule lists has a named test.

use super::super::bundle::{bundle_tests, run_bundle};
use super::*;

/// Each item of `ldml.test.golden`, with the vector file and test that
/// cover it.
const COVERAGE: &[(&str, &str, &str)] = &[
    (
        "macOS rule: unmatched dead key",
        "vro-macos",
        "unmatched dead key emits standalone then the key",
    ),
    (
        "macOS rule: caps shift",
        "vro-macos",
        "caps and caps shift give uppercase",
    ),
    (
        "macOS rule: no normalization",
        "vro-macos",
        "output is not normalized",
    ),
    (
        "macOS rule: backspace cancels the dead key",
        "vro-macos",
        "backspace cancels only the pending dead key",
    ),
    (
        "macOS rule: pending dead key shown and flushed",
        "vro-macos",
        "commit flushes the pending dead key",
    ),
    (
        "AltGr through altR",
        "vro-windows",
        "altgr reaches the altR layer",
    ),
    (
        "AltGr through ctrl alt",
        "modifiers",
        "altgr falls back to the ctrl alt layer",
    ),
    (
        "shortcut pass: Ctrl and Windows Left Alt",
        "vro-windows",
        "left alt and ctrl pass on windows",
    ),
    (
        "shortcut pass: Cmd",
        "vro-macos",
        "command shortcuts pass and native layers stay unused",
    ),
    (
        "native-only layers never selected",
        "modifiers",
        "native ctrl layer is never selected",
    ),
    (
        "extra modifier: rightCtrl",
        "modifiers",
        "right ctrl binds extra1",
    ),
    (
        "extra modifier: capsLock",
        "modifiers",
        "caps lock binds extra2 and loses caps",
    ),
    (
        "extra modifier: B00",
        "modifiers",
        "B00 binds extra3 and is consumed",
    ),
    ("LRM/RLM", "modifiers", "shift backspace inserts LRM or RLM"),
    (
        "decimal",
        "vro-macos",
        "decimal key types the layout decimal",
    ),
    (
        "Other with no layer",
        "vro-windows",
        "windows has no macOS alt layer",
    ),
    (
        "Other layer",
        "modifiers",
        "unmatched modifiers select the other layer",
    ),
    ("gesture: tap", "gestures", "tap types the key"),
    (
        "gesture: long press",
        "gestures",
        "long press picks by index or default",
    ),
    (
        "gesture: multi-tap",
        "gestures",
        "multi tap cycles through the key",
    ),
    (
        "gesture: flick",
        "gestures",
        "flicks follow their direction sequence",
    ),
    (
        "layer switch",
        "gestures",
        "a layer key switches without text",
    ),
    (
        "layer switch after output",
        "gestures",
        "output comes before the layer switch",
    ),
    (
        "regex: classes",
        "regex",
        "classes match members and non-members",
    ),
    ("regex: captures", "regex", "captures are replaced in order"),
    ("regex: $0", "regex", "dollar zero is the whole match"),
    ("regex: mapped sets", "regex", "mapped sets map by index"),
    (
        "regex: ^ with at_start",
        "regex",
        "caret matches at the start of text",
    ),
    (
        "regex: ^ without at_start",
        "regex",
        "caret fails when the host cannot tell",
    ),
    ("regex: \\m{.}", "regex", "any marker matches each marker"),
    (
        "regex: set alternation order",
        "regex",
        "set items are tried in order",
    ),
    (
        "regex: earliest match start",
        "regex",
        "the earliest match start wins",
    ),
    ("reorder: Tai Tham", "reorder", "vowel and tone typed first"),
    (
        "normalization: example 1",
        "markers",
        "example 1b glues the marker to the next mark",
    ),
    (
        "normalization: example 2",
        "markers",
        "example 2 keeps a trailing marker last",
    ),
    (
        "normalization: example 3",
        "markers",
        "example 3 normalizes each segment alone",
    ),
    (
        "output form: NFD",
        "markers-nfd",
        "inserted text takes the NFD output form",
    ),
    (
        "normalization: è plus U+0320",
        "markers",
        "e grave context then U+0320 key",
    ),
    (
        "backspace: ksha",
        "backspace-cancel",
        "one backspace deletes a ksha",
    ),
    (
        "backspace: cancelOrPass",
        "backspace-cancel",
        "a pending marker is cancelled alone",
    ),
    (
        "backspace: codePoint",
        "backspace-codepoint",
        "without a rule backspace deletes a scalar",
    ),
    (
        "context mismatch drops markers",
        "markers",
        "a context change drops the markers",
    ),
    (
        "vro dead keys",
        "vro-macos",
        "dead key composes with the next key",
    ),
    ("vro caps", "vro-windows", "dead keys and caps on windows"),
    ("vro long press", "vro-ios", "long press picks a candidate"),
    (
        "migrated vro dead keys",
        "vro-v3",
        "migrated windows dead keys compose",
    ),
    (
        "migrated vro caps",
        "vro-v3",
        "migrated windows caps layers keep v3 caps",
    ),
    (
        "migrated sme dead keys",
        "sme-v3",
        "migrated macos dead keys compose",
    ),
    (
        "migrated sme caps",
        "sme-v3",
        "migrated macos caps and caps shift",
    ),
    ("vro flicks", "vro-ios", "flicks type their target"),
    (
        "vro role keys",
        "vro-ios",
        "role keys and gaps are the host's own",
    ),
];

// [spec:kbdgen:req:ldml.test.golden/test]
// [spec:kbdgen:def:ldml.test.harness/test]
// [spec:kbdgen:def:ldml.test.vectors/test]
#[test]
fn golden_vectors_all_pass() {
    let report = run_bundle(&golden(), false).unwrap_or_else(|e| panic!("{e}"));
    let failures: Vec<String> = report.failures.iter().map(|f| f.to_string()).collect();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    assert_eq!(report.files, golden_files("tests", "yaml").len());
    assert!(report.checks >= 100, "{}", report.summary());
}

// [spec:kbdgen:req:ldml.test.golden/test]
#[test]
fn every_golden_item_has_a_named_test() {
    for (item, file, test) in COVERAGE {
        let path = golden().join(format!("tests/{file}.yaml"));
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{item}: {e}"));
        let parsed = vector_file(&text).unwrap_or_else(|e| panic!("{item}: {e}"));
        assert!(
            parsed.tests.iter().any(|t| t.name == *test),
            "{item}: no test {test:?} in {file}"
        );
    }
    let files = bundle_tests(&golden()).unwrap();
    for path in &files {
        let stem = path.file_stem().unwrap().to_string_lossy();
        assert!(
            COVERAGE.iter().any(|(_, f, _)| *f == stem),
            "{stem} covers no listed item"
        );
    }
}
