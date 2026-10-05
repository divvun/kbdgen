//! Modifier handling for hardware keys: extra-modifier bindings, the
//! shortcut rule and layer selection.

use kbd_model::{Component, ExtraModifierKey, Hardware, Host, ModifierSet, Modifiers, Windows};

use crate::api::ModifierState;

/// The scan code of the `B00` key, the ISO key left of `Z`.
pub(crate) const B00_SCAN_CODE: u8 = 0x56;

// [spec:kbdgen:sem:ldml.engine.extra]
/// Applies `windows.extra_modifiers` before matching: a `rightCtrl`
/// binding to `extra`*n* moves `ctrl_r` into `extra[n-1]`, and a
/// `capsLock` binding clears `caps`. For `capsLock` and `B00` bindings the
/// host itself sets `extra[n-1]` while the key is held, so the engine and
/// the layout DLL select the same columns.
pub(crate) fn apply_extra_bindings(windows: &Windows, mut m: ModifierState) -> ModifierState {
    for (i, key) in windows.extra_modifiers.iter().enumerate() {
        match key {
            ExtraModifierKey::RightCtrl => {
                if m.ctrl_r {
                    m.ctrl_r = false;
                    if let Some(slot) = m.extra.get_mut(i) {
                        *slot = true;
                    }
                }
            }
            ExtraModifierKey::CapsLock => m.caps = false,
            ExtraModifierKey::B00 => {}
        }
    }
    m
}

// [spec:kbdgen:req:ldml.engine.shortcuts+1]
/// Whether the event is a shortcut, which passes without matching any
/// layer: `cmd`, ctrl without alt, or Left Alt on a Windows host, where
/// Left Alt drives menus. Ctrl with AltGr is not a shortcut, so it reaches
/// a layer naming both ctrl and alt; and since every native-only layer
/// needs cmd or ctrl without alt, those layers are never used for typing.
pub(crate) fn is_shortcut(m: ModifierState, host: Option<Host>) -> bool {
    m.cmd || (m.any_ctrl() && !m.any_alt()) || (m.alt_l && host == Some(Host::Windows))
}

/// The state of one side pair (any, left, right) as a modifier set needs it.
fn side_matches(
    set: Modifiers,
    any: Component,
    left: Component,
    right: Component,
    l: bool,
    r: bool,
) -> bool {
    if set.contains(any) {
        l || r
    } else if set.contains(left) {
        l && !r
    } else if set.contains(right) {
        r && !l
    } else {
        !l && !r
    }
}

// [spec:kbdgen:sem:ldml.engine.modifiers]
/// Whether the component set `set` matches `m` exactly: shift, caps, cmd
/// and each extra are in the set exactly when held, and each of alt and
/// ctrl matches by side. `altL` does not tolerate a held right Alt.
pub(crate) fn set_matches(set: Modifiers, m: ModifierState) -> bool {
    use Component::*;
    set.contains(Shift) == m.any_shift()
        && set.contains(Caps) == m.caps
        && set.contains(Cmd) == m.cmd
        && [Extra1, Extra2, Extra3]
            .iter()
            .zip(m.extra)
            .all(|(c, held)| set.contains(*c) == held)
        && side_matches(set, Alt, AltL, AltR, m.alt_l, m.alt_r)
        && side_matches(set, Ctrl, CtrlL, CtrlR, m.ctrl_l, m.ctrl_r)
}

/// The non-native layer with a set matching `m`, if any. Overlapping
/// non-native sets are rejected by validation, so at most one matches.
fn matching_layer(hardware: &Hardware, m: ModifierState) -> Option<usize> {
    hardware.layers.iter().position(|layer| {
        !layer.is_native()
            && layer.modifiers.iter().any(|set| match set {
                ModifierSet::Set(s) => set_matches(*s, m),
                ModifierSet::Other => false,
            })
    })
}

// [spec:kbdgen:sem:ldml.engine.altgr]
/// The selected hardware layer: the non-native layer whose set matches
/// exactly; else, for Right Alt held as AltGr without ctrl, the layer
/// matching with `ctrl_l` added, so AltGr reaches both `altR` and
/// `ctrl alt` layers; else the `Other` layer, if any.
pub(crate) fn select_layer(hardware: &Hardware, m: ModifierState) -> Option<usize> {
    matching_layer(hardware, m)
        .or_else(|| {
            (m.altgr && m.alt_r && !m.any_ctrl())
                .then(|| matching_layer(hardware, ModifierState { ctrl_l: true, ..m }))
                .flatten()
        })
        .or_else(|| {
            hardware.layers.iter().position(|layer| {
                !layer.is_native() && layer.modifiers.contains(&ModifierSet::Other)
            })
        })
}
