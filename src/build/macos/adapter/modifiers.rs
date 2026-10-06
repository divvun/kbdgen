//! The `<modifier keys="…">` terms of a layer's modifier sets.

use kbd_model::{Component, Modifiers};

/// How a keyboard's modifiers reach the terms of every layer.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Policy {
    /// A `capsLock` extra-modifier binding makes the engine ignore Caps
    /// Lock, so every term takes it as optional.
    pub caps_ignored: bool,
    /// No typing set names ctrl and no layer is `Other`, so a native `ctrl`
    /// layer may take v3's loose term: every state it then matches passes
    /// in the engine.
    pub loose_ctrl: bool,
}

/// v3's term for the layer a set migrates from, which keeps the modifier
/// maps of migrated layouts as they were (`keylayout.keymaps`).
fn v3_term(set: Modifiers, caps_optional: bool, policy: Policy) -> Option<&'static str> {
    use Component::*;
    let of = Modifiers::of;
    let terms: [(Modifiers, bool, &str); 11] = [
        (Modifiers::NONE, false, "command?"),
        (of(&[Shift]), true, "anyShift caps? command?"),
        (of(&[Caps]), false, "caps"),
        (of(&[Caps, Shift]), false, "anyShift caps command?"),
        (of(&[Alt]), false, "anyOption command?"),
        (of(&[Alt, Shift]), true, "anyOption anyShift caps? command?"),
        (of(&[Alt, Caps]), false, "caps anyOption command?"),
        (of(&[Cmd]), false, "command"),
        (of(&[Cmd, Shift]), false, "command anyShift"),
        (of(&[Alt, Cmd]), false, "command anyOption"),
        (of(&[Alt, Cmd, Shift]), false, "command anyOption anyShift"),
    ];
    if set == of(&[Ctrl]) && policy.loose_ctrl {
        return Some("anyShift? caps? anyOption? anyControl");
    }
    terms
        .into_iter()
        .find(|(s, optional, _)| *s == set && *optional == caps_optional)
        .map(|(.., term)| term)
}

fn side(
    set: Modifiers,
    [any, left, right]: [Component; 3],
    names: [&'static str; 3],
) -> Option<&'static str> {
    [any, left, right]
        .into_iter()
        .zip(names)
        .find(|(component, _)| set.contains(*component))
        .map(|(_, name)| name)
}

/// The exact term of `set`: each component as the key the keylayout names,
/// Caps optional when `caps_optional`, and `command?` on typing sets, whose
/// Command states the engine passes as shortcuts.
fn generic_term(set: Modifiers, caps_optional: bool) -> String {
    use Component::*;
    let mut keys = Vec::new();
    if set.contains(Shift) {
        keys.push("anyShift");
    }
    if set.contains(Caps) {
        keys.push("caps");
    } else if caps_optional {
        keys.push("caps?");
    }
    keys.extend(side(
        set,
        [Alt, AltL, AltR],
        ["anyOption", "option", "rightOption"],
    ));
    keys.extend(side(
        set,
        [Ctrl, CtrlL, CtrlR],
        ["anyControl", "control", "rightControl"],
    ));
    if set.contains(Cmd) {
        keys.push("command");
    } else if !set.is_native() {
        keys.push("command?");
    }
    keys.join(" ")
}

/// The terms of a layer's sets, in set order. A set S and S with `caps` on
/// one layer share one term with Caps optional.
// [spec:kbdgen:sem:ldml.macos.modifiers]
pub fn terms(sets: &[Modifiers], policy: Policy) -> Vec<String> {
    let mut out = Vec::new();
    for set in sets {
        let caps = set.contains(Component::Caps);
        if caps && sets.contains(&set.without(Component::Caps)) && !policy.caps_ignored {
            continue;
        }
        let optional = policy.caps_ignored || (!caps && sets.contains(&set.with(Component::Caps)));
        let term = v3_term(*set, optional, policy)
            .filter(|_| !policy.caps_ignored)
            .map_or_else(|| generic_term(*set, optional), str::to_owned);
        out.push(term);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use Component::*;

    fn of(components: &[Component]) -> Modifiers {
        Modifiers::of(components)
    }

    // [spec:kbdgen:sem:ldml.macos.modifiers/test]
    #[test]
    fn migrated_v3_layers_get_their_v3_terms() {
        let policy = Policy {
            loose_ctrl: true,
            ..Policy::default()
        };
        let cases: [(&[Modifiers], &str); 8] = [
            (&[Modifiers::NONE], "command?"),
            (
                &[of(&[Shift]), of(&[Caps, Shift])],
                "anyShift caps? command?",
            ),
            (&[of(&[Caps])], "caps"),
            (&[of(&[Alt]), of(&[Alt, Caps])], "caps? anyOption command?"),
            (
                &[of(&[Alt, Shift]), of(&[Alt, Caps, Shift])],
                "anyOption anyShift caps? command?",
            ),
            (&[of(&[Alt, Caps])], "caps anyOption command?"),
            (&[of(&[Ctrl])], "anyShift? caps? anyOption? anyControl"),
            (&[of(&[Alt, Cmd])], "command anyOption"),
        ];
        for (sets, expected) in cases {
            assert_eq!(terms(sets, policy), vec![expected.to_owned()], "{sets:?}");
        }
    }

    // [spec:kbdgen:sem:ldml.macos.modifiers/test]
    #[test]
    fn other_sets_get_exact_terms() {
        let policy = Policy::default();
        assert_eq!(
            terms(&[of(&[AltR, Shift])], policy),
            ["anyShift rightOption command?"]
        );
        assert_eq!(
            terms(&[of(&[CtrlL, Alt])], policy),
            ["anyOption control command?"]
        );
        assert_eq!(terms(&[of(&[Ctrl])], policy), ["anyControl"]);
        assert_eq!(terms(&[of(&[Shift])], policy), ["anyShift command?"]);
        let ignored = Policy {
            caps_ignored: true,
            ..policy
        };
        assert_eq!(terms(&[Modifiers::NONE], ignored), ["caps? command?"]);
        assert_eq!(terms(&[of(&[Cmd])], ignored), ["caps? command"]);
    }
}
