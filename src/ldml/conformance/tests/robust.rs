//! Robustness (`ldml.test.robust`): decoding arbitrary bytes, and the
//! engine on arbitrary contexts and event sequences against every fixture
//! model, never panic, and every edit keeps to `ldml.engine.action`.

use std::sync::LazyLock;

use kbd_engine::harness::Harness;
use kbd_engine::{Action, BackspacePolicy, Gesture, Key, KeyEvent, ModifierState, Options};
use kbd_model::{Direction, Host};
use proptest::prelude::*;

use super::*;
use crate::ldml::yaml::{load, lower};

/// Every fixture model: each host document of the golden layouts, the
/// golden XML keyboards and the vendored CLDR keyboards, under both
/// backspace policies.
static MODELS: LazyLock<Vec<Model>> = LazyLock::new(|| {
    let mut keyboards = Vec::new();
    for path in golden_files("layouts", "yaml") {
        let tag = path.file_stem().unwrap().to_string_lossy().into_owned();
        for (host, source) in lower(&load(&path, &tag).unwrap()).unwrap() {
            let mut keyboard = kbd_ldml::resolve(&source).unwrap().keyboard;
            keyboard.host = Some(host);
            keyboards.push(keyboard);
        }
    }
    let cldr = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("crates/kbd-ldml/testdata/cldr/48/keyboards/3.0");
    let mut xml = golden_files("keyboards", "xml");
    xml.extend(std::fs::read_dir(cldr).unwrap().map(|e| e.unwrap().path()));
    for path in xml {
        let source = kbd_ldml::read_keyboard_file(&path).unwrap();
        keyboards.push(kbd_ldml::resolve(&source).unwrap().keyboard);
    }
    let mut models = Vec::new();
    for keyboard in keyboards {
        for backspace in [BackspacePolicy::CancelOrPass, BackspacePolicy::CodePoint] {
            let options = Options {
                backspace,
                host: Some(Host::Windows),
                ..Options::default()
            };
            models.push(Model::from_keyboard(keyboard.clone(), options).unwrap());
        }
    }
    models
});

/// Characters the fixtures' rules care about, plus any scalar value.
fn text() -> impl Strategy<Value = String> {
    let interesting = prop::sample::select(vec![
        'a', 'e', 'o', 'x', 'q', 'z', ' ', '´', 'á', 'è', '\u{300}', '\u{301}', '\u{320}',
        '\u{323}', 'ᨡ', '\u{1A60}', 'ᩅ', '\u{1A6B}', '\u{1A76}', 'क', '\u{94D}', 'श', 'ক',
        '\u{9C7}', '\u{9D7}',
    ]);
    let any = any::<char>();
    prop::collection::vec(prop_oneof![4 => interesting, 1 => any], 0..6)
        .prop_map(|cs| cs.into_iter().collect())
}

fn gesture() -> impl Strategy<Value = Gesture> {
    prop_oneof![
        Just(Gesture::Tap),
        (0usize..6).prop_map(Gesture::LongPress),
        (0usize..6).prop_map(Gesture::MultiTap),
        prop::collection::vec(prop::sample::select(Direction::ALL.to_vec()), 0..3)
            .prop_map(Gesture::Flick),
    ]
}

fn modifiers() -> impl Strategy<Value = ModifierState> {
    (any::<[bool; 9]>(), any::<[bool; 3]>()).prop_map(|(b, extra)| ModifierState {
        shift_l: b[0],
        shift_r: b[1],
        caps: b[2],
        ctrl_l: b[3],
        ctrl_r: b[4],
        alt_l: b[5],
        alt_r: b[6],
        altgr: b[7],
        cmd: b[8],
        extra,
    })
}

/// A key event, or `None` for a context change outside the engine.
fn event() -> impl Strategy<Value = Option<KeyEvent>> {
    let key = prop_oneof![
        4 => any::<u8>().prop_map(Key::Scan),
        1 => Just(Key::Decimal),
        2 => Just(Key::Backspace),
        1 => Just(Key::Commit),
        2 => (0usize..3, 0usize..4, 0usize..5, 0usize..14, gesture()).prop_map(
            |(set, layer, row, col, gesture)| Key::Touch { set, layer, row, col, gesture }
        ),
        2 => ("[a-z0-9-]{1,8}", gesture()).prop_map(|(id, gesture)| Key::Id { id, gesture }),
        2 => text().prop_map(Key::Emit),
    ];
    prop_oneof![
        8 => (key, modifiers(), any::<bool>()).prop_map(|(key, modifiers, repeat)| {
            Some(KeyEvent { key, modifiers, repeat })
        }),
        1 => Just(None),
    ]
}

/// Runs `events` from `context`; fails on an edit outside the bounds of
/// `ldml.engine.action`. A context change puts `context` back as the
/// document, which the state's markers usually no longer match.
fn drive(
    model: &Model,
    context: &str,
    at_start: bool,
    events: &[Option<KeyEvent>],
) -> Result<(), String> {
    let mut h = Harness::new(model);
    h.set_document(context);
    h.set_at_start(at_start);
    for event in events {
        let Some(event) = event else {
            h.set_document(context);
            continue;
        };
        let seen = h.context().text.chars().count();
        if let Action::Edit { delete, .. } = h.send(event)
            && *delete > seen
        {
            return Err(format!(
                "{event:?} deleted {delete} of {seen} scalar values"
            ));
        }
    }
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    // [spec:kbdgen:req:ldml.test.robust]
    // [spec:kbdgen:req:ldml.test.robust/test]
    #[test]
    fn decoding_arbitrary_bytes_never_panics(bytes in prop::collection::vec(any::<u8>(), 0..512)) {
        let _ = Model::from_bytes(&bytes);
        let mut header = kbd_model::MAGIC.to_vec();
        header.extend(&bytes);
        let _ = Model::from_bytes(&header);
    }

    // [spec:kbdgen:req:ldml.test.robust/test]
    #[test]
    fn corrupted_fixture_encodings_never_panic(
        index in any::<prop::sample::Index>(),
        flips in prop::collection::vec((any::<prop::sample::Index>(), any::<u8>()), 1..8),
        cut in any::<prop::sample::Index>(),
    ) {
        let mut bytes = index.get(&MODELS).keyboard().to_bytes().unwrap();
        for (at, value) in &flips {
            let i = at.index(bytes.len());
            bytes[i] ^= value;
        }
        let _ = Model::from_bytes(&bytes);
        bytes.truncate(cut.index(bytes.len() + 1));
        let _ = Model::from_bytes(&bytes);
    }

    // [spec:kbdgen:req:ldml.test.robust]
    // [spec:kbdgen:req:ldml.test.robust/test]
    #[test]
    fn engine_edits_stay_in_bounds(
        index in any::<prop::sample::Index>(),
        context in text(),
        at_start in any::<bool>(),
        events in prop::collection::vec(event(), 1..24),
    ) {
        let model = index.get(&MODELS);
        prop_assert_eq!(drive(model, &context, at_start, &events), Ok(()));
    }
}

// [spec:kbdgen:req:ldml.test.robust/test]
#[test]
fn every_fixture_model_survives_every_scan_code() {
    let hosts = MODELS
        .iter()
        .filter(|m| m.keyboard().host == Some(Host::Ios))
        .count();
    assert!(MODELS.len() >= 40, "{}", MODELS.len());
    assert!(hosts >= 2, "touch-only models are included");
    let events: Vec<Option<KeyEvent>> = (0..=0x7F)
        .map(|c| Some(KeyEvent::with(Key::Scan(c), ModifierState::shift())))
        .collect();
    for model in MODELS.iter() {
        assert_eq!(drive(model, "aक्श", true, &events), Ok(()));
    }
}
