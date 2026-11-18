//! The presented set and its reports on the golden layouts and on small
//! layouts for single behaviours.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use language_tags::LanguageTag;

use super::*;
use crate::build::diag::{Diagnostics, Platform};
use crate::build::linux::source::{self, LinuxLayout};
use crate::bundle::fixture::write_bundle;
use crate::bundle::read_kbdgen_bundle;

const WORKSPACE: &str = env!("CARGO_MANIFEST_DIR");

fn fixture_text(relative: &str) -> String {
    let path = Path::new(WORKSPACE).join(relative);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// The golden Võro layout with a `linux` variant that inherits its
/// Windows one, so K has dead keys, long-press candidates and an AltGr
/// layer.
pub(super) fn vro() -> String {
    fixture_text("crates/kbd-engine/tests/golden/layouts/vro.yaml")
        .replace("\ntouch:\n", "\n  linux: {inherits: windows}\ntouch:\n")
}

/// The Windows feature fixture, its variant renamed `linux`.
pub(super) fn features() -> String {
    fixture_text("src/build/windows/kbdl/testdata/features.yaml")
        .replace("hardware:\n  windows:", "hardware:\n  linux:")
}

pub(super) fn mods() -> String {
    fixture_text("crates/kbd-engine/tests/golden/layouts/und-x-mods.yaml")
}

pub(super) fn se_no() -> String {
    fixture_text("crates/kbd-engine/tests/golden/v3/se-NO.yaml")
}

/// The token counts of the `us` form's character rows.
const US_ROWS: [usize; 4] = [13, 13, 11, 10];

/// A `us` layer of `rows`, each filled up with `\u{0}`, as YAML block
/// lines under a `layers` key.
pub(super) fn us_layer(sets: &str, rows: [&str; 4]) -> String {
    let mut layer = format!("      {sets}: |\n");
    for (row, count) in rows.into_iter().zip(US_ROWS) {
        let mut tokens: Vec<&str> = row.split_whitespace().collect();
        tokens.resize(count, r"\u{0}");
        layer += &format!("        {}\n", tokens.join(" "));
    }
    layer
}

/// A v4 layout tagged `sma` whose `linux` variant has the `us` form and
/// `layers`, after the top-level fields `head`.
pub(super) fn sma(head: &str, layers: &[String]) -> String {
    format!(
        "format: 4\ndisplayNames: {{sma: Åarjelsaemien, en: Southern Sami}}\n{head}hardware:\n  linux:\n    form: us\n    impliedLayers: none\n    layers:\n{}",
        layers.concat()
    )
}

/// A layout for single behaviours: widths, gaps and short rows, labels by
/// output and by key id, candidates, gestures, a dead key without a
/// standalone, a dead-key candidate, mixed and multi-scalar outputs, a
/// backspace rule, and a shift output plasma-keyboard uppercases.
pub(super) fn single() -> String {
    let head = r#"keyNames: {space: gaskoe, return: åtna}
deadKeys:
  ´: {standalone: ´, compose: {a: á}}
  ¨: {standalone: '', compose: {a: ä}}
longPress: {e: 'é ë \d{´}'}
keys:
  wide: {output: w, width: 1.5}
  tapped: {output: t, multiTap: wide}
  held: {output: h, longPress: wide, longPressDefault: wide}
  flicked: {output: f, flick: up}
flicks: {up: [{directions: n, key: wide}]}
backspace:
  - [{from: ng}]
displays:
  - {output: q, display: Q̄}
  - {keyId: tapped, display: T!}
"#;
    sma(
        head,
        &[
            us_layer(
                "none",
                [
                    r"\d{´} \d{¨} ŋg a\m{x}",
                    r"q \k{wide} e \k{tapped} \k{held} \k{flicked}",
                    r"\s{gap:0.5} s \u{0} d",
                    "",
                ],
            ),
            us_layer("shift", ["ß ˝", "Q W E", "", ""]),
            us_layer("caps", ["x", "", "", ""]),
        ],
    )
}

pub(super) fn tag(tag: &str) -> LanguageTag {
    tag.parse().unwrap()
}

/// The Linux layouts of a bundle holding `layouts`, each as (stem, text).
pub(super) fn linux_layouts(layouts: &[(&str, &str)]) -> Vec<LinuxLayout> {
    let dir = tempfile::tempdir().unwrap();
    let path = write_bundle(
        dir.path(),
        "test",
        layouts,
        &[(
            "linux",
            "packageId: no.divvun\nlayoutPrefix: divvun\nversion: 1.0\n",
        )],
        &[],
    );
    let bundle = read_kbdgen_bundle(&path).unwrap();
    source::layouts(&bundle).unwrap()
}

pub(super) fn linux_layout(stem: &str, text: &str) -> LinuxLayout {
    linux_layouts(&[(stem, text)]).pop().unwrap()
}

/// Thousandths of a key as a decimal: `1.5`, `1`.
fn width(thousandths: u32) -> String {
    format!("{}", f64::from(thousandths) / 1000.0)
}

fn with_width(token: String, thousandths: u32) -> String {
    match thousandths {
        1000 => token,
        w => format!("{token}:{}", width(w)),
    }
}

fn with_candidates(token: String, candidates: &[String]) -> String {
    match candidates {
        [] => token,
        c => format!("{token}{{{}}}", c.join(",")),
    }
}

/// A level as one line per row: `a` a text key, `a=L` with label L,
/// `a{b,c}` with candidates, `:1.5` a width, `\d{display|standalone}` a
/// dead key, `_` a gap, and `⇧`, `⌫` and `#` the added keys.
pub(super) fn dump(level: &Level) -> String {
    let token = |item: &Item| match item {
        Item::Text(key) => {
            let mut token = key.output.clone();
            if let Some(label) = &key.label {
                token = format!("{token}={label}");
            }
            with_width(with_candidates(token, &key.candidates), key.width)
        }
        Item::Dead(key) => with_width(
            with_candidates(
                format!("\\d{{{}|{}}}", key.display, key.standalone),
                &key.candidates,
            ),
            key.width,
        ),
        Item::Gap(w) => with_width("_".to_owned(), *w),
        Item::Shift => "⇧".to_owned(),
        Item::Backspace => "⌫".to_owned(),
        Item::SymbolSwitch => "#".to_owned(),
    };
    level
        .rows
        .iter()
        .map(|row| row.iter().map(token).collect::<Vec<_>>().join(" ") + "\n")
        .collect()
}

/// The warnings and information messages, one per line.
pub(super) fn messages(diag: &Diagnostics) -> String {
    let warnings = diag.warnings().iter().map(|w| format!("warning: {w}\n"));
    let infos = diag.infos().iter().map(|i| format!("info: {i}\n"));
    warnings.chain(infos).collect()
}

/// What `osk` reports for `layout`, with the information messages.
fn reported(layout: &LinuxLayout, osk: Osk) -> String {
    let presented = present(layout).unwrap();
    let classification = classify(&layout.keyboard, &presented, &layout.tag);
    let mut diag = Diagnostics::new(Platform::Linux, layout.tag.as_str());
    report(&classification, osk, &mut diag);
    inform(&classification, &mut diag);
    messages(&diag)
}

// [spec:kbdgen:def:ldml.linux.touch/test]
// [spec:kbdgen:sem:ldml.linux.touch.keys/test]
#[test]
fn desktop_levels_present_the_hardware_rows() {
    let presented = present(&linux_layout("vro", &vro())).unwrap();
    assert_eq!(
        dump(&presented.default),
        "\\d{ˇ|ˇ} 1 2 3 4 5 6 7 8 9 0 + \\d{´|´} ⌫\n\
         q w e r t y u i o{ó,ö,õ,ø,õ̭} p ü õ\n\
         a{á,ä,æ,å} s d f g h j k l ö ä '\n\
         ⇧ < z x c v b n m , . -\n"
    );
    assert_eq!(
        dump(presented.shift.as_ref().unwrap()),
        "\\d{~|~} ! \" # € % & / ( ) = ? \u{301}=\u{25CC}\u{301} ⌫\n\
         Q W E R T Y U I O P Ü Õ\n\
         A S D F G H J K L Ö Ä *\n\
         ⇧ > Z X C V B N M ; : _\n"
    );
    assert_eq!(presented.default.kind, LevelKind::Default);
    assert_eq!(presented.space_label.as_deref(), Some("vaih"));
    assert_eq!(presented.return_label.as_deref(), Some("sisse"));
}

// [spec:kbdgen:def:ldml.linux.touch/test]
#[test]
fn symbol_levels_are_fixed_with_added_keys() {
    let presented = present(&linux_layout("vro", &vro())).unwrap();
    let [first, second] = &presented.symbols;
    assert_eq!(first.kind, LevelKind::Symbols);
    assert_eq!(second.kind, LevelKind::MoreSymbols);
    assert_eq!(
        dump(first),
        "1 2 3 4 5 6 7 8 9 0 ⌫\n@ # € % & - + ( )\n# * \" ' : ; ! ?\n"
    );
    assert_eq!(
        dump(second),
        "~ ` | • √ π ÷ × ¶ ∆ ⌫\n£ ¥ $ ¢ ^ ° = { }\n# \\ © ® ™ ℅ [ ]\n"
    );
    assert!(first.layer.is_none() && second.layer.is_none());
    let levels: Vec<LevelKind> = presented.levels().map(|l| l.kind).collect();
    assert_eq!(
        levels,
        [
            LevelKind::Default,
            LevelKind::Shift,
            LevelKind::Symbols,
            LevelKind::MoreSymbols
        ]
    );
    assert_eq!(Item::SymbolSwitch.width(), ADDED_KEY_WIDTH);
    assert_eq!(ADDED_KEY_WIDTH, 1500);
}

// [spec:kbdgen:sem:ldml.linux.touch.keys/test]
#[test]
fn keys_take_outputs_labels_widths_and_gaps() {
    let presented = present(&linux_layout("sma", &single())).unwrap();
    assert_eq!(
        dump(&presented.default),
        "\\d{´|´} \\d{¨|} ŋg _ _ _ _ _ _ _ _ _ ⌫\n\
         q=Q̄ w:1.5 e{é,ë} t=T! h{w} f _ _ _ _ _ _ _\n\
         _:0.5 s _ d _ _ _ _ _ _ _\n\
         ⇧ _ _ _ _ _ _ _ _ _ _\n"
    );
    assert_eq!(
        dump(presented.shift.as_ref().unwrap()),
        "ß ˝ _ _ _ _ _ _ _ _ _ _ _ ⌫\n\
         Q W E _ _ _ _ _ _ _ _ _ _\n\
         _ _ _ _ _ _ _ _ _ _ _\n\
         ⇧ _ _ _ _ _ _ _ _ _ _\n"
    );
    assert_eq!(presented.space_label.as_deref(), Some("gaskoe"));
    assert_eq!(presented.return_label.as_deref(), Some("åtna"));
    assert_eq!(presented.default.width(), 13_500);
}

// [spec:kbdgen:def:ldml.linux.touch/test]
#[test]
fn other_layer_serves_as_the_default_level() {
    let layout = linux_layout(
        "sma",
        &sma(
            "",
            &[
                us_layer("other", ["a b", "", "", ""]),
                us_layer("caps", ["A B", "", "", ""]),
            ],
        ),
    );
    let presented = present(&layout).unwrap();
    let hardware = layout.keyboard.hardware.as_ref().unwrap();
    assert_eq!(
        dump(&presented.default).lines().next(),
        Some("a b _ _ _ _ _ _ _ _ _ _ _ ⌫")
    );
    assert_eq!(
        presented.shift.as_ref().and_then(|l| l.layer),
        presented.default.layer,
        "Shift selects the other layer too"
    );
    assert_eq!(
        hardware.layers[presented.default.layer.unwrap()].modifiers,
        [kbd_model::ModifierSet::Other]
    );
}

// [spec:kbdgen:def:ldml.linux.touch/test]
#[test]
fn without_a_shift_layer_no_shift_key() {
    let layout = linux_layout("sma", &sma("", &[us_layer("none", ["a b", "", "", ""])]));
    let presented = present(&layout).unwrap();
    assert!(presented.shift.is_none());
    let rows = dump(&presented.default);
    assert!(!rows.contains('⇧'), "{rows}");
    assert_eq!(rows.lines().last(), Some("_ _ _ _ _ _ _ _ _ _"));
}

// [spec:kbdgen:def:ldml.linux.touch/test]
#[test]
fn no_default_layer_fails_naming_the_layout() {
    let mut layout = linux_layout("sma", &sma("", &[us_layer("caps", ["A B", "", "", ""])]));
    assert_eq!(
        present(&layout).unwrap_err().to_string(),
        "sma: no hardware layer is selected with no modifier, so the on-screen keyboards have no default level"
    );
    layout.keyboard.hardware = None;
    assert_eq!(
        present(&layout).unwrap_err().to_string(),
        "sma: the linux keyboard has no hardware layers"
    );
}

// [spec:kbdgen:req:ldml.linux.classify.touch/test]
#[test]
fn plasma_reports_vro_hidden_layers_and_dead_keys() {
    assert_eq!(
        reported(&linux_layout("vro", &vro()), Osk::PlasmaKeyboard),
        "warning: plasma-keyboard does not show 2 layer(s) other than the default and shift levels, which only hardware keys type (e.g. caps; altR, altR caps)\n\
         warning: plasma-keyboard types 3 dead key(s) as their standalone output, which composes nothing, or leaves them out where they have none (e.g. ˇ; ´; ~)\n\
         info: GNOME Shell sends 1 on-screen text(s) of several scalar values to the IBus engine one scalar value at a time (e.g. õ̭)\n\
         info: on GNOME Shell, a dead key pressed on screen composes only with the layout's IBus source; with its XKB source, only with a hardware key\n\
         info: plasma-keyboard and Stevia type their on-screen layouts without the IBus engine, so no transform applies there\n"
    );
}

// [spec:kbdgen:req:ldml.linux.classify.touch/test]
#[test]
fn gnome_reports_only_dead_keys_without_keysyms() {
    let layout = linux_layout("vro", &vro());
    let presented = present(&layout).unwrap();
    let dead: Vec<_> = presented
        .default
        .rows
        .iter()
        .flatten()
        .filter_map(|item| match item {
            Item::Dead(key) => Some(key.marker),
            _ => None,
        })
        .collect();
    let keysyms: BTreeSet<_> = dead.into_iter().collect();
    let gnome = reported(&layout, Osk::GnomeShell(&keysyms));
    assert!(
        gnome.contains(
            "warning: GNOME Shell types 1 dead key(s) without a dead keysym in the XKB layout as their standalone output, which composes nothing, or leaves them out where they have none (e.g. ~)\n"
        ),
        "{gnome}"
    );
    let all = reported(&layout, Osk::GnomeShell(&BTreeSet::new()));
    assert!(all.contains("GNOME Shell types 3 dead key(s)"), "{all}");
    let stevia = reported(&layout, Osk::Stevia);
    assert!(
        stevia.contains("warning: Stevia does not show 2 layer(s)"),
        "{stevia}"
    );
    assert!(!stevia.contains("uppercases"), "{stevia}");
}

// [spec:kbdgen:req:ldml.linux.classify.touch/test]
#[test]
fn single_layout_reports_every_touch_feature() {
    assert_eq!(
        reported(&linux_layout("sma", &single()), Osk::PlasmaKeyboard),
        "warning: plasma-keyboard does not show 1 layer(s) other than the default and shift levels, which only hardware keys type (e.g. caps)\n\
         warning: plasma-keyboard has no flicks, multi-tap or default long-press candidate for 3 key(s) (e.g. tapped; held; flicked)\n\
         warning: plasma-keyboard types 2 dead key(s) as their standalone output, which composes nothing, or leaves them out where they have none (e.g. ´; ¨ (left out))\n\
         warning: plasma-keyboard leaves out 1 key(s) mixing text and markers (e.g. o-1)\n\
         warning: plasma-keyboard leaves out 1 long-press candidate(s) holding markers (e.g. dk-dk_00B4)\n\
         warning: plasma-keyboard uppercases 1 shift-level text(s) for the locale, so they type otherwise than the shift layer (e.g. ß → SS)\n\
         info: GNOME Shell sends 1 on-screen text(s) of several scalar values to the IBus engine one scalar value at a time (e.g. ŋg)\n\
         info: on GNOME Shell, a dead key pressed on screen composes only with the layout's IBus source; with its XKB source, only with a hardware key\n\
         info: GNOME Shell's on-screen Backspace skips the layout's backspace transforms while nothing is shown as preedit\n\
         info: plasma-keyboard and Stevia type their on-screen layouts without the IBus engine, so no transform applies there\n"
    );
}

// [spec:kbdgen:req:ldml.linux.classify.touch/test]
#[test]
fn extra_modifier_and_altgr_layers_are_reported() {
    let mods = reported(&linux_layout("und-x-mods", &mods()), Osk::PlasmaKeyboard);
    assert!(
        mods.starts_with(
            "warning: plasma-keyboard does not show 5 layer(s) other than the default and shift levels, which only hardware keys type (e.g. alt ctrl; other; extra1; extra2; extra3)\n"
        ),
        "{mods}"
    );
    let features = reported(&linux_layout("sme", &features()), Osk::PlasmaKeyboard);
    assert!(
        features.starts_with(
            "warning: plasma-keyboard does not show 3 layer(s) other than the default and shift levels, which only hardware keys type (e.g. caps; altR, altR caps; extra1, caps extra1)\n\
             warning: plasma-keyboard types 2 dead key(s) as their standalone output"
        ),
        "{features}"
    );
}

// [spec:kbdgen:def:ldml.linux.touch/test]
// [spec:kbdgen:req:ldml.linux.classify.touch/test]
#[test]
fn v3_and_abnt2_layouts_present_every_position() {
    let layouts = linux_layouts(&[("se-NO", &se_no()), ("sme", &features())]);
    for layout in &layouts {
        let presented = present(layout).unwrap();
        let hardware = layout.keyboard.hardware.as_ref().unwrap();
        let positions: usize = hardware
            .form
            .rows
            .iter()
            .flatten()
            .filter(|&&code| code != 0x39)
            .count();
        for level in presented.hardware_levels() {
            let shown = level
                .rows
                .iter()
                .flatten()
                .filter(|item| !matches!(item, Item::Shift | Item::Backspace));
            assert_eq!(shown.count(), positions, "{}: {:?}", layout.tag, level.kind);
        }
        let reported = reported(layout, Osk::Stevia);
        assert!(
            reported.ends_with(
                "info: plasma-keyboard and Stevia type their on-screen layouts without the IBus engine, so no transform applies there\n"
            ),
            "{reported}"
        );
    }
    let features = present(&layouts[1]).unwrap();
    assert_eq!(
        dump(&features.default).lines().last(),
        Some("⇧ < z x c v b n m , . - /"),
        "the abnt2 form's 73 key ends the last row"
    );
}

// [spec:kbdgen:req:ldml.linux.classify.touch/test]
#[test]
fn plasma_uppercases_single_units_for_the_locale() {
    let cases = [
        ("ß", "sma", "SS"),
        ("i", "tr", "İ"),
        ("i", "az-Latn", "İ"),
        ("i", "se", "I"),
        ("Õ", "vro", "Õ"),
        ("ŋg", "sma", "ŋg"),
        ("\u{1D44E}", "sma", "\u{1D44E}"),
        ("ǆ", "sma", "Ǆ"),
    ];
    for (text, locale, shifted) in cases {
        assert_eq!(
            classify::plasma_shifted(text, &tag(locale)),
            shifted,
            "{text} in {locale}"
        );
    }
}

#[test]
fn display_name_is_the_autonym() {
    let layout = linux_layout("vro", &vro());
    assert_eq!(display_name(&layout).unwrap(), "Võro");
    let mut layout = layout;
    layout.display_names.remove("vro");
    assert_eq!(
        display_name(&layout).unwrap_err().to_string(),
        "vro: displayNames has no vro entry"
    );
}
