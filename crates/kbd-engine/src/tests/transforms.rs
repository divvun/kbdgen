//! Transforms: captures, sets, classes and group chaining.

use super::*;

// [spec:kbdgen:sem:ldml.engine.replace/test]
#[test]
fn map_set_maps_captured_item() {
    let m = model(transform_keyboard());
    let mut s = Session::new(&m);
    assert_eq!(s.emit("C"), typed("C"));
    assert_eq!(s.emit("C"), edit(1, "c", ""));
    assert_eq!(s.emit("A"), edit(0, "a", ""));
    assert_eq!(s.text, "ca");
}

// [spec:kbdgen:sem:ldml.engine.replace/test]
#[test]
fn class_captures_reorder_in_replacement() {
    let m = model(transform_keyboard());
    let mut s = Session::new(&m);
    s.emit("e");
    assert_eq!(s.emit("7"), edit(1, "7e", ""));
    s.emit("x");
    assert_eq!(s.emit("7"), typed("7"));
}

// [spec:kbdgen:sem:ldml.engine.transforms/test]
// [spec:kbdgen:sem:ldml.engine.replace/test]
#[test]
fn groups_chain_without_looping_back() {
    let m = model(transform_keyboard());
    let mut s = Session::new(&m);
    s.emit("x");
    // Group 0 gives "<>" from an empty capture; group 1 rewrites it.
    assert_eq!(s.emit("z"), edit(1, "∅", ""));
    s.emit("x");
    s.emit("y");
    assert_eq!(s.emit("z"), edit(2, "<y>", ""));
    // B maps to b in group 0, which group 1 turns into BB; processing
    // never returns to group 0, which would map a B again.
    assert_eq!(s.emit("B"), typed("BB"));
}

// [spec:kbdgen:sem:ldml.engine.replace/test]
#[test]
fn whole_match_keeps_its_markers() {
    let m = model(transform_keyboard());
    let mut s = Session::new(&m);
    s.id("mark");
    assert_eq!(s.emit("b"), typed("b!"));
    assert_eq!(s.state.tail().elements()[0], TextElem::Marker(0));
    assert!(s.pending().is_empty());
}

// [spec:kbdgen:sem:ldml.engine.match+1/test]
// [spec:kbdgen:req:ldml.engine.tsf+2/test]
#[test]
fn anchored_rule_needs_at_start_context() {
    let m = model(transform_keyboard());
    let mut s = Session::new(&m);
    s.at_start = true;
    assert_eq!(s.emit("q"), typed("Q"));
    assert_eq!(s.emit("q"), typed("q"));
    let mut t = Session::new(&m);
    assert_eq!(t.emit("q"), typed("q"));
}

// [spec:kbdgen:sem:ldml.engine.context+1/test]
// [spec:kbdgen:sem:ldml.engine.match+1/test]
#[test]
fn truncated_context_is_not_at_start() {
    let mut k = keyboard(Normalization::Disabled);
    // The first group shrinks C, so the anchored rule of the second can
    // span all of it: only the truncation decides whether it applies.
    k.simple = vec![
        TransformGroup::Rules(vec![rule(chars("xyz"), vec![to_text("Q")])]),
        TransformGroup::Rules(vec![Rule {
            from: Pattern::from_tree(true, vec![chars("vwQ")]).unwrap(),
            to: vec![to_text("R")],
        }]),
    ];
    let m = model(k);
    assert_eq!(m.context_len(), 4);
    let press = |text: &str| {
        let context = Context {
            text: text.to_string(),
            authoritative: true,
            at_start: true,
        };
        let event = KeyEvent::new(Key::Emit("z".to_string()));
        m.key(&State::default(), &context, &event).0
    };
    assert_eq!(press("vwxy"), edit(4, "R", ""));
    assert_eq!(press("uvwxy"), edit(2, "Q", ""));
}
