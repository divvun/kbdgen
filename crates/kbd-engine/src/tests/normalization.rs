//! Normalization, output forms and the edit segment.

use super::*;

/// Keys for combining marks, a dead key glued before a mark, and rules
/// written in NFD.
fn nfd_keyboard(normalization: Normalization) -> Keyboard {
    let mut k = keyboard(normalization);
    k.markers = vec!["x".to_string()];
    k.keys = vec![
        ModelKey::new("macron-below", text("\u{320}")),
        ModelKey::new("acute", text("\u{301}")),
        ModelKey::new(
            "grave-marked",
            Text(vec![TextElem::Char('\u{300}'), TextElem::Marker(0)]),
        ),
    ];
    k.simple = vec![TransformGroup::Rules(vec![
        rule(chars("e\u{320}\u{300}"), vec![to_text("X")]),
        rule(
            vec![mk(0), ch('\u{320}'), ch('\u{300}')],
            vec![to_text("Y")],
        ),
    ])];
    k
}

fn press_id(m: &Model, state: &State, context: &str, id: &str) -> (Action, State) {
    m.key(
        state,
        &Context::new(context),
        &KeyEvent::new(Key::Id {
            id: id.to_string(),
            gesture: Gesture::Tap,
        }),
    )
}

// [spec:kbdgen:sem:ldml.engine.normalization/test]
#[cfg(feature = "normalization")]
#[test]
fn context_is_normalized_before_matching() {
    let m = model(nfd_keyboard(Normalization::Enabled));
    let state = State::default();
    for context in ["\u{E8}", "e\u{300}"] {
        let (action, _) = press_id(&m, &state, context, "macron-below");
        assert_eq!(
            action,
            edit(context.chars().count(), "X", ""),
            "{context:?}"
        );
    }
}

// [spec:kbdgen:sem:ldml.engine.normalization/test]
#[test]
fn disabled_normalization_keeps_scalars() {
    let m = model(nfd_keyboard(Normalization::Disabled));
    let state = State::default();
    let (action, _) = press_id(&m, &state, "\u{E8}", "macron-below");
    assert_eq!(action, typed("\u{320}"));
    let (action, _) = press_id(&m, &state, "e\u{300}", "macron-below");
    assert_eq!(action, typed("\u{320}"));
    let (action, _) = press_id(&m, &state, "\u{E8}", "acute");
    assert_eq!(action, typed("\u{301}"));
}

// [spec:kbdgen:sem:ldml.engine.normalization/test]
#[cfg(feature = "normalization")]
#[test]
fn markers_survive_normalization_steps() {
    let m = model(nfd_keyboard(Normalization::Enabled));
    let (action, state) = press_id(&m, &State::default(), "e", "grave-marked");
    assert_eq!(action, edit(1, "\u{E8}", ""));
    // The host's context is now NFC; the tail still matches it.
    let (action, _) = press_id(&m, &state, "\u{E8}", "macron-below");
    assert_eq!(action, edit(1, "eY", ""));
}

// [spec:kbdgen:sem:ldml.engine.output.segment/test]
// [spec:kbdgen:def:ldml.engine.output.form/test]
#[cfg(feature = "normalization")]
#[test]
fn output_form_applies_to_caret_segment() {
    let state = State::default();
    let nfc = model(nfd_keyboard(Normalization::Enabled));
    let (action, _) = press_id(&nfc, &state, "ab\u{E8}", "acute");
    assert_eq!(action, edit(1, "\u{E8}\u{301}", ""));
    // A mark after a base rewrites the base, the start of its segment.
    let (action, _) = press_id(&nfc, &state, "ab", "acute");
    assert_eq!(action, edit(1, "b\u{301}", ""));
    let (action, _) = press_id(&nfc, &state, "a", "macron-below");
    assert_eq!(action, edit(1, "a\u{320}", ""));
    let options = Options {
        output_form: OutputForm::Nfd,
        ..Options::default()
    };
    let nfd = model_with(nfd_keyboard(Normalization::Enabled), options);
    let (action, _) = press_id(&nfd, &state, "ab\u{E8}", "acute");
    assert_eq!(action, edit(1, "e\u{300}\u{301}", ""));
    // An NFC base with a mark that composes is rewritten as one scalar.
    let (action, _) = press_id(&nfc, &state, "e", "acute");
    assert_eq!(action, edit(1, "\u{E9}", ""));
}

// [spec:kbdgen:sem:ldml.engine.output.segment/test]
#[test]
fn disabled_edit_keeps_common_prefix() {
    let mut k = keyboard(Normalization::Disabled);
    k.simple = vec![TransformGroup::Rules(vec![rule(
        chars("abc"),
        vec![to_text("aXc")],
    )])];
    let m = model(k);
    let (action, _) = m.key(
        &State::default(),
        &Context::new("zab"),
        &KeyEvent::new(Key::Emit("c".to_string())),
    );
    assert_eq!(action, edit(1, "Xc", ""));
    let (action, _) = m.key(
        &State::default(),
        &Context::new("zab"),
        &KeyEvent::new(Key::Emit("c".to_string())),
    );
    // Deterministic across calls.
    assert_eq!(action, edit(1, "Xc", ""));
}

// [spec:kbdgen:req:ldml.crate.engine/test]
#[cfg(feature = "normalization")]
#[test]
fn enabled_model_loads_with_feature() {
    let k = nfd_keyboard(Normalization::Enabled);
    let mut bad = k.clone();
    bad.keys.push(ModelKey::new("e-acute", text("\u{E9}")));
    bad.context_len = u8::try_from(bad.computed_context_len().unwrap()).unwrap();
    assert!(matches!(
        Model::from_keyboard(bad, Options::default()),
        Err(Error::Invalid(e)) if e.invariant == kbd_model::Invariant::NotNfd
    ));
    let m = model(k);
    let bytes = m.keyboard().to_bytes().unwrap();
    assert_eq!(Model::from_bytes(&bytes).unwrap(), m);
}

// [spec:kbdgen:req:ldml.crate.engine/test]
#[cfg(not(feature = "normalization"))]
#[test]
fn enabled_model_refused_without_feature() {
    let mut k = nfd_keyboard(Normalization::Enabled);
    k.context_len = u8::try_from(k.computed_context_len().unwrap()).unwrap();
    let bytes = k.to_bytes().unwrap();
    assert_eq!(
        Model::from_keyboard(k, Options::default()),
        Err(Error::NormalizationUnsupported)
    );
    assert_eq!(
        Model::from_bytes(&bytes),
        Err(Error::NormalizationUnsupported)
    );
    // Disabled keyboards load.
    model(nfd_keyboard(Normalization::Disabled));
}
