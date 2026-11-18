//! Edits, context, held text, on-screen Backspace and password fields
//! (`linux.ibus.semantics`, `linux.ibus.edit`, `linux.ibus.context`,
//! `linux.ibus.context.fallback`, `linux.ibus.osk-backspace`,
//! `linux.ibus.security`).

// Clippy counts only `#[test]` functions of a test binary as tests, not
// the helpers beside them.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use common::field::{Client, GTK, Report};
use common::fixture::{context, model, vro};
use kbd_engine::{Model, Options};
use kbd_ibus::keys::SHIFT_MASK;
use kbd_ibus::keysym;
use kbd_ibus::planner::{PURPOSE_PASSWORD, PURPOSE_PIN, PreeditMode, Session, Signal};

const Q: u32 = 16;
const A: u32 = 30;
const O: u32 = 24;
const Z: u32 = 44;
const W: u32 = 17;
const ACUTE: u32 = 13;
const ASTRAL: u32 = 27;
const TLDE: u32 = 41;
const BACKSPACE: u32 = 14;
const LEFT: u32 = 105;
const TAB: u32 = 0xff09;

fn commit(text: &str) -> Signal {
    Signal::CommitText(text.to_owned())
}

fn preedit(text: &str, mode: PreeditMode) -> Signal {
    Signal::UpdatePreeditText {
        text: text.to_owned(),
        cursor: u32::try_from(text.chars().count()).unwrap(),
        visible: !text.is_empty(),
        mode,
    }
}

fn clear() -> Signal {
    preedit("", PreeditMode::Clear)
}

fn delete(n: u32) -> Signal {
    Signal::DeleteSurroundingText {
        offset: -i32::try_from(n).unwrap(),
        nchars: n,
    }
}

// [spec:kbdgen:req:linux.ibus.edit/test]
#[test]
fn edits_clear_delete_commit_then_show() {
    let mut client = Client::gtk(model("vro", &vro()));
    assert!(client.tap(ACUTE));
    assert_eq!(client.last, [preedit("´", PreeditMode::Commit)]);
    assert!(client.tap(A));
    assert_eq!(client.last, [clear(), commit("á")]);

    let mut client = Client::gtk(model("und", &context()));
    client.tap(Q);
    assert_eq!(client.last, [commit("q")]);
    client.tap(Q);
    assert_eq!(client.last, [delete(1), commit("ʠ")]);
    client.tap(ASTRAL);
    client.tap(Z);
    assert_eq!(client.last, [delete(1), commit("ż")], "one astral scalar");
    assert_eq!(client.field.text(), "ʠż");
}

// [spec:kbdgen:sem:linux.ibus.context/test]
#[test]
fn readings_on_time_or_late_give_context() {
    for report in [Report::OnTime, Report::Late] {
        let mut client = Client::new(model("und", &context()), report, true);
        for code in [Q, Q, W, ASTRAL, Z, A, ACUTE, Q, Q] {
            client.tap(code);
        }
        assert_eq!(client.field.text(), "ʠwżáʠ", "{report:?}");
        assert!(client.field.preedit.is_empty());
    }
}

// [spec:kbdgen:sem:linux.ibus.context/test]
#[test]
fn disagreeing_reading_replaces_context() {
    let mut client = Client::gtk(model("und", &context()));
    client.tap(Q);
    client.replace_text("x");
    client.tap(Q);
    assert_eq!(client.last, [commit("q")]);
    assert_eq!(client.field.text(), "xq");
    client.replace_text("");
    client.tap(Q);
    client.tap(Q);
    assert_eq!(
        client.field.text(),
        "ʠ",
        "an empty reading is an empty context"
    );
}

// [spec:kbdgen:sem:linux.ibus.context/test]
#[test]
fn selection_resets_and_empties_context() {
    let mut client = Client::new(model("und", &context()), Report::Never, true);
    client.session.set_capabilities(GTK);
    client.tap(Q);
    assert_eq!(client.last, [commit("q")]);
    client.session.set_surrounding_text("q", 0, 1);
    client.tap(Q);
    assert_eq!(client.last, [commit("q")], "nothing before a selection");
    client.tap(Q);
    assert_eq!(client.last, [delete(1), commit("ʠ")]);
}

// [spec:kbdgen:sem:linux.ibus.context.fallback/test]
#[test]
fn rewriting_model_holds_text_without_surrounding() {
    let m = model("und", &context());
    let held = m.context_len();
    let mut client = Client::new(m, Report::Never, true);
    client.tap(Q);
    assert_eq!(client.last, [preedit("q", PreeditMode::Commit)]);
    client.tap(Q);
    assert_eq!(client.last, [preedit("ʠ", PreeditMode::Commit)]);
    assert!(client.field.before.is_empty(), "nothing committed yet");
    for code in [W, A, ACUTE, ASTRAL, Z] {
        client.tap(code);
        assert!(
            !client
                .last
                .iter()
                .any(|s| matches!(s, Signal::DeleteSurroundingText { .. })),
            "never deletes: {:?}",
            client.last
        );
        assert!(client.field.preedit.chars().count() <= held);
    }
    assert_eq!(client.field.text(), "ʠwáż");
    assert!(!client.tap(LEFT), "a frame key passes");
    assert_eq!(client.field.before, "ʠwáż", "and commits the held text");
    assert!(client.field.preedit.is_empty());
    client.tap(Q);
    client.focus_out();
    assert_eq!(client.field.text(), "ʠwáżq");
}

// [spec:kbdgen:sem:linux.ibus.context.fallback/test]
#[test]
fn held_text_is_left_to_client_on_reset() {
    let mut client = Client::new(model("und", &context()), Report::Never, true);
    client.tap(Q);
    client.focus_out();
    assert_eq!(
        client.field.before, "q",
        "committed as preedit by the client"
    );
    client.session.focus_in();
    client.tap(Q);
    assert_eq!(client.field.text(), "qq", "the cache was forgotten");
}

// [spec:kbdgen:sem:linux.ibus.context.fallback/test]
#[test]
fn other_models_commit_at_once_without_surrounding() {
    let mut client = Client::new(model("vro", &vro()), Report::Never, true);
    client.tap(A);
    assert_eq!(client.last, [commit("a")]);
    client.tap(ACUTE);
    client.tap(A);
    assert_eq!(client.last, [clear(), commit("á")]);
    assert_eq!(client.field.text(), "aá");
}

// [spec:kbdgen:sem:linux.ibus.context.fallback/test]
#[test]
fn without_preedit_nothing_is_shown() {
    let mut client = Client::new(model("und", &context()), Report::Never, false);
    client.tap(Q);
    client.tap(Q);
    assert_eq!(client.last, [commit("q")], "the context is empty");
    let mut client = Client::new(model("vro", &vro()), Report::Never, false);
    assert!(client.tap(ACUTE));
    assert!(client.last.is_empty());
    assert!(!client.tap(LEFT));
    assert_eq!(client.last, [commit("´")], "Commit before passing");
}

/// Võro with `~` a dead key that shows nothing of its own.
fn bare_tilde() -> String {
    let vro = vro();
    let tilde = "'~': {compose: {o: õ, O: Õ}}";
    assert!(vro.contains(tilde));
    vro.replace(tilde, "'~': {standalone: '', compose: {o: õ, O: Õ}}")
}

// [spec:kbdgen:sem:linux.ibus.osk-backspace/test]
#[test]
fn bare_dead_key_shows_marker_dropped_on_reset() {
    let mut client = Client::gtk(model("vro", &bare_tilde()));
    assert!(
        client.key(0, TLDE, SHIFT_MASK),
        "~ shows nothing of its own"
    );
    assert_eq!(
        client.last,
        [preedit("~", PreeditMode::Clear)],
        "the marker's display"
    );
    assert!(client.tap(BACKSPACE), "Backspace cancels it");
    assert_eq!(client.last, [preedit("", PreeditMode::Commit)]);
    client.key(0, TLDE, SHIFT_MASK);
    client.tap(O);
    assert_eq!(client.last, [clear(), commit("õ")]);
    client.key(0, TLDE, SHIFT_MASK);
    client.focus_out();
    assert_eq!(client.field.text(), "õ", "the reset drops it");
    let onscreen = client.session.process_key_event(keysym::BACKSPACE, 0, 0);
    assert!(!onscreen.handled);

    let mut keyboard = model("vro", &bare_tilde()).keyboard().clone();
    keyboard.displays.entries.clear();
    let undisplayed = Model::from_keyboard(keyboard, Options::default()).unwrap();
    let mut client = Client::gtk(undisplayed);
    client.key(0, TLDE, SHIFT_MASK);
    assert_eq!(client.last, [preedit("\u{25CC}", PreeditMode::Clear)]);
}

// [spec:kbdgen:req:linux.ibus.security/test]
#[test]
fn password_fields_map_keys_without_preedit() {
    for purpose in [PURPOSE_PASSWORD, PURPOSE_PIN] {
        let mut client = Client::gtk(model("vro", &vro()));
        client.session.set_content_type(purpose, 0);
        assert!(client.tap(ACUTE));
        assert!(client.last.is_empty(), "no preedit");
        client.tap(A);
        assert_eq!(client.last, [commit("á")]);
        client.tap(ACUTE);
        assert!(!client.tap(LEFT));
        assert_eq!(client.last, [commit("´")]);
        client.tap(ACUTE);
        client.focus_out();
        assert_eq!(client.field.text(), "á´", "the pending dead key is dropped");

        let mut client = Client::gtk(model("und", &context()));
        client.session.set_content_type(purpose, 0);
        client.tap(Q);
        client.tap(Q);
        assert_eq!(client.field.text(), "qq", "no context is kept");
    }
}

// [spec:kbdgen:sem:linux.ibus.semantics/test]
#[test]
fn resets_forget_state_context_and_cache() {
    let mut client = Client::new(model("vro", &vro()), Report::Never, true);
    client.tap(ACUTE);
    assert!(client.session.reset().is_empty());
    client.field.settle();
    assert_eq!(
        client.field.text(),
        "´",
        "the client commits the shown preedit"
    );
    client.tap(A);
    assert_eq!(client.last, [commit("a")]);
    for event in [Session::focus_in, Session::focus_out, Session::disable] {
        client.tap(ACUTE);
        assert!(event(&mut client.session).is_empty());
        client.field.settle();
        client.tap(A);
        assert_eq!(client.last, [commit("a")]);
    }
    client.tap(ACUTE);
    assert_eq!(client.session.enable(), [Signal::RequireSurroundingText]);
}

// [spec:kbdgen:sem:linux.ibus.semantics/test]
#[test]
fn passing_key_commits_preedit_then_types() {
    let mut client = Client::gtk(model("vro", &vro()));
    client.tap(ACUTE);
    assert!(!client.key(TAB, 15, 0), "Tab is in no form");
    assert_eq!(client.last, [clear(), commit("´")]);
    client.tap(ACUTE);
    assert!(client.tap(BACKSPACE), "Backspace cancels a dead key");
    assert!(!client.tap(BACKSPACE), "and else passes");
    assert_eq!(client.field.text(), "");
}
