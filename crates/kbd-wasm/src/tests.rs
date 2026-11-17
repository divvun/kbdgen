//! The exports, called natively as a host calls them on wasm: through the
//! input and output buffers, by address. Each test thread has its own
//! instance.

use std::collections::BTreeMap;

use kbd_model::Component::{AltR, Shift};
use kbd_model::{
    ExtraModifierKey, Form, Hardware, HardwareLayer, Info, Key as ModelKey, Keyboard, ModifierSet,
    Modifiers, Normalization, Pattern, ReplacementItem, Rule, Text, TextElem, TransformGroup,
    TreeAtom, TreeItem,
};

use super::abi::*;
use super::*;

const Q: u32 = 0x10;
const A: u32 = 0x1E;

/// Q is a dead acute and A types `a`, `A` with Shift and `b` with
/// AltGr; the acute composes `á` with `a`.
fn keyboard() -> Keyboard {
    let mut k = Keyboard::new(
        "und",
        46,
        Info {
            name: "Test".to_owned(),
            ..Info::default()
        },
    );
    k.normalization = Normalization::Disabled;
    k.markers = vec!["acute".to_owned()];
    k.keys = vec![
        ModelKey::new("acute", Text(vec![TextElem::Marker(0)])),
        ModelKey::new("a", Text::from("a")),
        ModelKey::new("A", Text::from("A")),
        ModelKey::new("b", Text::from("b")),
    ];
    let layer = |sets: &[&[kbd_model::Component]], rows: Vec<Vec<u16>>| HardwareLayer {
        id: None,
        modifiers: sets
            .iter()
            .map(|s| ModifierSet::Set(Modifiers::of(s)))
            .collect(),
        rows,
    };
    k.hardware = Some(Hardware {
        form: Form {
            id: "test".to_owned(),
            rows: vec![vec![0x10, 0x1E]],
        },
        min_device_width: None,
        layers: vec![
            layer(&[&[]], vec![vec![0, 1]]),
            layer(&[&[Shift]], vec![vec![0, 2]]),
            layer(&[&[AltR]], vec![vec![0, 3]]),
        ],
    });
    let from = vec![
        TreeItem::one(TreeAtom::Marker(0)),
        TreeItem::one(TreeAtom::Char('a')),
    ];
    k.simple = vec![TransformGroup::Rules(vec![Rule {
        from: Pattern::from_tree(false, vec![from]).unwrap(),
        to: vec![ReplacementItem::Text(Text::from("á"))],
    }])];
    k.flush = BTreeMap::from([(0, "´".to_owned())]);
    k.context_len = u8::try_from(k.computed_context_len().unwrap()).unwrap();
    k
}

/// Copies `bytes` into the input buffer, as a host writes into memory.
fn write_input(bytes: &[u8]) -> usize {
    let address = kbd_input(bytes.len());
    with_instance((), |i| {
        assert_eq!(i.input.as_ptr().expose_provenance(), address);
        i.input.copy_from_slice(bytes);
    });
    bytes.len()
}

fn output() -> (String, String) {
    let address = kbd_output();
    let len = kbd_output_len();
    let bytes = with_instance(Vec::new(), |i| {
        assert_eq!(i.output.as_ptr().expose_provenance(), address);
        i.output.clone()
    });
    assert_eq!(bytes.len(), len);
    let (insert, preedit) = bytes.split_at(kbd_insert_len());
    (
        String::from_utf8(insert.to_vec()).unwrap(),
        String::from_utf8(preedit.to_vec()).unwrap(),
    )
}

fn load(keyboard: &Keyboard) -> u32 {
    let len = write_input(&keyboard.to_bytes().unwrap());
    let handle = kbd_load(len);
    assert_ne!(handle, 0, "{:?}", output());
    handle
}

/// Sends `key` in `context`; the action as (delete, insert, preedit).
fn key(handle: u32, key: u32, modifiers: u32, context: &str) -> Option<(usize, String, String)> {
    let len = write_input(context.as_bytes());
    match kbd_key(handle, key, modifiers, len, AUTHORITATIVE) {
        PASS => None,
        EDIT => {
            let (insert, preedit) = output();
            Some((kbd_delete(), insert, preedit))
        }
        error => panic!("kbd_key failed with {error}"),
    }
}

fn edit(delete: usize, insert: &str, preedit: &str) -> Option<(usize, String, String)> {
    Some((delete, insert.to_owned(), preedit.to_owned()))
}

// [spec:kbdgen:req:ldml.chromeos.wasm/test]
#[test]
fn a_session_keeps_the_engine_state_between_keys() {
    let handle = load(&keyboard());
    assert_eq!(kbd_pending(handle), 0);
    assert_eq!(key(handle, Q, 0, "x"), edit(0, "", "´"));
    assert_eq!(kbd_pending(handle), 1);
    assert_eq!(key(handle, A, 0, "x"), edit(0, "á", ""));
    assert_eq!(kbd_pending(handle), 0);

    assert_eq!(key(handle, Q, 0, "xá"), edit(0, "", "´"));
    assert_eq!(key(handle, KEY_COMMIT, 0, "xá"), edit(0, "´", ""));
    kbd_reset(handle);
    assert_eq!(kbd_pending(handle), 0);

    assert_eq!(key(handle, A, SHIFT_L, ""), edit(0, "A", ""));
    assert_eq!(key(handle, A, ALT_R | ALTGR, ""), edit(0, "b", ""));
    assert_eq!(key(handle, KEY_BACKSPACE, 0, "b"), None);
    assert_eq!(key(handle, KEY_DECIMAL, 0, ""), None);
}

// [spec:kbdgen:req:ldml.chromeos.wasm/test]
// [spec:kbdgen:req:ldml.engine.shortcuts+2/test]
#[test]
fn shortcuts_pass_and_keep_the_state() {
    let handle = load(&keyboard());
    assert_eq!(key(handle, Q, 0, ""), edit(0, "", "´"));
    assert_eq!(key(handle, A, CTRL_L, ""), None);
    assert_eq!(key(handle, A, ALT_L, ""), None);
    assert_eq!(key(handle, A, CMD, ""), None);
    assert_eq!(kbd_pending(handle), 1);
}

// [spec:kbdgen:req:ldml.chromeos.wasm/test]
#[test]
fn errors_are_codes_and_messages() {
    let len = write_input(b"not a model");
    assert_eq!(kbd_load(len), 0);
    let (message, preedit) = output();
    assert!(!message.is_empty());
    assert!(preedit.is_empty());
    assert_eq!(kbd_load(len + 1), 0);
    assert!(
        output().0.contains("input buffer holds 11"),
        "{:?}",
        output()
    );

    let handle = load(&keyboard());
    let len = write_input(b"");
    assert_eq!(kbd_key(handle + 1, Q, 0, len, 0), NO_SUCH_MODEL);
    assert_eq!(kbd_key(handle, 0x1000, 0, len, 0), INVALID_EVENT);
    assert_eq!(kbd_key(handle, Q, EXTRA3 << 1, len, 0), INVALID_EVENT);
    let len = write_input(&[0xFF]);
    assert_eq!(kbd_key(handle, Q, 0, len, 0), INVALID_CONTEXT);
    assert_eq!(kbd_key(handle, Q, 0, len + 1, 0), INVALID_CONTEXT);
    assert_eq!(kbd_pending(handle + 1), 0);
    assert_eq!(kbd_context_len(handle + 1), 0);
}

// [spec:kbdgen:req:ldml.chromeos.wasm/test]
#[test]
fn handles_are_reused_after_unloading() {
    let first = load(&keyboard());
    let second = load(&keyboard());
    assert_eq!((first, second), (1, 2));
    kbd_unload(first);
    assert_eq!(kbd_context_len(first), 0);
    assert_eq!(load(&keyboard()), first);
    kbd_unload(99);
    assert_ne!(kbd_context_len(second), 0);
}

// [spec:kbdgen:req:ldml.chromeos.wasm/test]
#[test]
fn keyboard_options_are_exposed() {
    let mut k = keyboard();
    let handle = load(&k);
    assert_eq!(kbd_extra_modifiers(handle), 0);
    assert_eq!(kbd_shift_lock(handle), 0);
    assert_eq!(kbd_context_len(handle), u32::from(keyboard().context_len));

    k.windows.extra_modifiers = vec![
        ExtraModifierKey::RightCtrl,
        ExtraModifierKey::CapsLock,
        ExtraModifierKey::B00,
    ];
    k.windows.shift_lock = true;
    let handle = load(&k);
    assert_eq!(kbd_extra_modifiers(handle), 1 | (2 << 2) | (3 << 4));
    assert_eq!(kbd_shift_lock(handle), 1);
}
