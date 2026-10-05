//! Modifier components and modifier sets.

use core::cmp::Ordering;
use serde::{Deserialize, Serialize};

/// A modifier component.
///
/// The declaration order is the component order: LDML's canonical order
/// followed by the Extension components.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Component {
    Alt,
    AltL,
    AltR,
    Caps,
    Ctrl,
    CtrlL,
    CtrlR,
    Shift,
    /// Extension: Command, Windows or Super.
    Cmd,
    /// Extension: bound by `windows.extra_modifiers[0]`.
    Extra1,
    /// Extension: bound by `windows.extra_modifiers[1]`.
    Extra2,
    /// Extension: bound by `windows.extra_modifiers[2]`.
    Extra3,
}

impl Component {
    /// Every component, in component order.
    pub const ALL: [Component; 12] = [
        Component::Alt,
        Component::AltL,
        Component::AltR,
        Component::Caps,
        Component::Ctrl,
        Component::CtrlL,
        Component::CtrlR,
        Component::Shift,
        Component::Cmd,
        Component::Extra1,
        Component::Extra2,
        Component::Extra3,
    ];

    /// The component's name as LDML and v4 YAML write it.
    pub const fn name(self) -> &'static str {
        match self {
            Component::Alt => "alt",
            Component::AltL => "altL",
            Component::AltR => "altR",
            Component::Caps => "caps",
            Component::Ctrl => "ctrl",
            Component::CtrlL => "ctrlL",
            Component::CtrlR => "ctrlR",
            Component::Shift => "shift",
            Component::Cmd => "cmd",
            Component::Extra1 => "extra1",
            Component::Extra2 => "extra2",
            Component::Extra3 => "extra3",
        }
    }

    /// The component written as `name`, if any. `none` and `other` are not
    /// components.
    pub fn from_name(name: &str) -> Option<Component> {
        Component::ALL.into_iter().find(|c| c.name() == name)
    }

    const fn bit(self) -> u16 {
        1 << (self as u16)
    }

    /// For `extra1`–`extra3`, the 1-based *n*; otherwise none.
    pub const fn extra_number(self) -> Option<u8> {
        match self {
            Component::Extra1 => Some(1),
            Component::Extra2 => Some(2),
            Component::Extra3 => Some(3),
            _ => None,
        }
    }
}

const ALL_BITS: u16 = (1 << Component::ALL.len()) - 1;

/// A set of modifier components. The empty set is LDML's `none`.
///
/// Stored as a bit set in component order, so equal sets have one encoding.
/// Decoding accepts any `u16`; bits beyond the twelve components are
/// rejected by validation (`Modifiers::is_well_formed`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Modifiers(u16);

impl Modifiers {
    /// `none`.
    pub const NONE: Modifiers = Modifiers(0);

    /// The set of `components`.
    pub fn of(components: &[Component]) -> Modifiers {
        Modifiers(components.iter().fold(0, |acc, c| acc | c.bit()))
    }

    pub fn contains(self, c: Component) -> bool {
        self.0 & c.bit() != 0
    }

    pub fn with(self, c: Component) -> Modifiers {
        Modifiers(self.0 | c.bit())
    }

    pub fn without(self, c: Component) -> Modifiers {
        Modifiers(self.0 & !c.bit())
    }

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// The number of components.
    pub fn len(self) -> u32 {
        self.0.count_ones()
    }

    /// The components, in component order.
    pub fn components(self) -> impl Iterator<Item = Component> {
        Component::ALL
            .into_iter()
            .filter(move |c| self.contains(*c))
    }

    /// Whether every stored bit names a component.
    pub fn is_well_formed(self) -> bool {
        self.0 & !ALL_BITS == 0
    }

    fn has_alt(self) -> bool {
        self.contains(Component::Alt)
            || self.contains(Component::AltL)
            || self.contains(Component::AltR)
    }

    fn has_ctrl(self) -> bool {
        self.contains(Component::Ctrl)
            || self.contains(Component::CtrlL)
            || self.contains(Component::CtrlR)
    }

    // [spec:kbdgen:def:ldml.model.modifiers]
    /// The first rejected combination in this set, if any:
    ///
    /// - `alt` with `altL` or `altR`
    /// - `ctrl` with `ctrlL` or `ctrlR`
    /// - a left side with a right side, such as `altL ctrlR` or `altL altR`
    pub fn rejected_combination(self) -> Option<(Component, Component)> {
        use Component::*;
        let pairs = [
            (Alt, AltL),
            (Alt, AltR),
            (Ctrl, CtrlL),
            (Ctrl, CtrlR),
            (AltL, AltR),
            (AltL, CtrlR),
            (CtrlL, AltR),
            (CtrlL, CtrlR),
        ];
        pairs
            .into_iter()
            .find(|(a, b)| self.contains(*a) && self.contains(*b))
    }

    // [spec:kbdgen:def:ldml.model.native]
    /// Whether a layer with this set is native-only: the set contains
    /// `cmd`, or a ctrl component with no alt component.
    pub fn is_native(self) -> bool {
        self.contains(Component::Cmd) || (self.has_ctrl() && !self.has_alt())
    }

    /// The (left, right) key states that one side's components accept,
    /// as a bit set over the four states `00`, `01` (right), `10` (left),
    /// `11`, following `ldml.engine.modifiers`.
    fn side_states(self, any: Component, left: Component, right: Component) -> u8 {
        if self.contains(any) {
            0b1110
        } else if self.contains(left) {
            0b0100
        } else if self.contains(right) {
            0b0010
        } else {
            0b0001
        }
    }

    /// Whether some modifier state matches both sets under the exact
    /// matching of `ldml.engine.modifiers`. Both sets must have no rejected
    /// combination.
    pub fn overlaps(self, other: Modifiers) -> bool {
        use Component::*;
        let exact = Modifiers::of(&[Shift, Caps, Cmd, Extra1, Extra2, Extra3]).0;
        self.0 & exact == other.0 & exact
            && self.side_states(Alt, AltL, AltR) & other.side_states(Alt, AltL, AltR) != 0
            && self.side_states(Ctrl, CtrlL, CtrlR) & other.side_states(Ctrl, CtrlL, CtrlR) != 0
    }
}

impl Ord for Modifiers {
    /// Size first, then the component sequences compared in component
    /// order.
    fn cmp(&self, other: &Self) -> Ordering {
        self.len()
            .cmp(&other.len())
            .then_with(|| self.components().cmp(other.components()))
    }
}

impl PartialOrd for Modifiers {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// A modifier set of a hardware layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ModifierSet {
    /// A set of components; `Modifiers::NONE` is `none`.
    Set(Modifiers),
    /// LDML `other`: selected when no other layer matches.
    Other,
}

impl ModifierSet {
    /// Whether this set makes its layer native-only (`ldml.model.native`).
    pub fn is_native(self) -> bool {
        match self {
            ModifierSet::Set(m) => m.is_native(),
            ModifierSet::Other => false,
        }
    }
}

impl Ord for ModifierSet {
    /// The stored order of a layer's sets: component sets by
    /// [`Modifiers`]' order, then `Other`.
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (ModifierSet::Set(a), ModifierSet::Set(b)) => a.cmp(b),
            (ModifierSet::Set(_), ModifierSet::Other) => Ordering::Less,
            (ModifierSet::Other, ModifierSet::Set(_)) => Ordering::Greater,
            (ModifierSet::Other, ModifierSet::Other) => Ordering::Equal,
        }
    }
}

impl PartialOrd for ModifierSet {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(test)]
mod tests {
    use super::Component::*;
    use super::*;
    use alloc::vec;
    use alloc::vec::Vec;

    // [spec:kbdgen:def:ldml.model.modifiers/test]
    #[test]
    fn sets_sort_by_size_then_component_order() {
        let mut sets = vec![
            Modifiers::of(&[Shift, Caps]),
            Modifiers::of(&[Shift]),
            Modifiers::NONE,
            Modifiers::of(&[Extra1]),
            Modifiers::of(&[Caps]),
            Modifiers::of(&[Alt, Shift]),
            Modifiers::of(&[Cmd]),
        ];
        sets.sort();
        let expected = vec![
            Modifiers::NONE,
            Modifiers::of(&[Caps]),
            Modifiers::of(&[Shift]),
            Modifiers::of(&[Cmd]),
            Modifiers::of(&[Extra1]),
            Modifiers::of(&[Alt, Shift]),
            Modifiers::of(&[Caps, Shift]),
        ];
        assert_eq!(sets, expected);
        let mut with_other = [ModifierSet::Other, ModifierSet::Set(Modifiers::NONE)];
        with_other.sort();
        assert_eq!(with_other[1], ModifierSet::Other);
    }

    // [spec:kbdgen:def:ldml.model.modifiers/test]
    #[test]
    fn rejects_mixed_sides_and_generic_sides() {
        for bad in [
            [Alt, AltL],
            [Alt, AltR],
            [Ctrl, CtrlR],
            [AltL, CtrlR],
            [AltL, AltR],
            [CtrlL, AltR],
        ] {
            assert!(
                Modifiers::of(&bad).rejected_combination().is_some(),
                "{bad:?}"
            );
        }
        for good in [[AltL, CtrlL], [AltR, CtrlR], [Alt, Ctrl], [Alt, CtrlL]] {
            assert_eq!(
                Modifiers::of(&good).rejected_combination(),
                None,
                "{good:?}"
            );
        }
    }

    // [spec:kbdgen:def:ldml.model.native/test]
    #[test]
    fn ctrl_without_alt_and_cmd_are_native() {
        assert!(Modifiers::of(&[Cmd]).is_native());
        assert!(Modifiers::of(&[Cmd, Alt, Shift]).is_native());
        assert!(Modifiers::of(&[Ctrl]).is_native());
        assert!(Modifiers::of(&[CtrlL, Shift]).is_native());
        assert!(!Modifiers::of(&[Ctrl, Alt]).is_native());
        assert!(!Modifiers::of(&[AltR]).is_native());
        assert!(!ModifierSet::Other.is_native());
    }

    #[test]
    fn overlap_follows_exact_side_matching() {
        assert!(Modifiers::of(&[Alt]).overlaps(Modifiers::of(&[AltR])));
        assert!(!Modifiers::of(&[AltL]).overlaps(Modifiers::of(&[AltR])));
        assert!(!Modifiers::NONE.overlaps(Modifiers::of(&[Alt])));
        assert!(!Modifiers::of(&[Shift]).overlaps(Modifiers::NONE));
        assert!(Modifiers::of(&[Ctrl, Alt]).overlaps(Modifiers::of(&[CtrlL, AltR])));
        assert!(!Modifiers::of(&[Ctrl, AltL]).overlaps(Modifiers::of(&[Ctrl, AltR])));
    }

    #[test]
    fn component_names_round_trip() {
        for c in Component::ALL {
            assert_eq!(Component::from_name(c.name()), Some(c));
        }
        assert_eq!(Component::from_name("none"), None);
        let all: Vec<_> = Modifiers::of(&Component::ALL).components().collect();
        assert_eq!(all, Component::ALL);
        assert!(!Modifiers(1 << 12).is_well_formed());
    }
}
