//! The keyboard invariants (`ldml.model.invariants`) and their check.

use alloc::collections::BTreeSet;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use crate::extensions::WINDOWS_KEY_NAMES;
use crate::keyboard::{DisplayTarget, KeyIndex, Keyboard, MAX_CONTEXT_LEN, Normalization};
use crate::layers::Hardware;
use crate::modifiers::{ModifierSet, Modifiers};
use crate::text::{MarkerIndex, Text, is_nmtoken};
use crate::transforms::{Atom, ClassRange, ReorderClass, ReplacementItem, TransformGroup};

/// Answers whether text is in Unicode Normalization Form D.
///
/// `kbd-model` carries no Unicode data, so the caller supplies the check:
/// `kbd-ldml` and `kbd-engine` back it with `icu_normalizer`.
pub trait NfdCheck {
    fn is_nfd(&self, text: &str) -> bool;

    /// The first scalar value in `lo..=hi` that is not NFD on its own.
    /// The default asks [`NfdCheck::is_nfd`] about each value in turn.
    fn first_non_nfd(&self, lo: char, hi: char) -> Option<char> {
        let mut buf = [0u8; 4];
        (lo..=hi).find(|c| !self.is_nfd(c.encode_utf8(&mut buf)))
    }
}

/// An invariant a keyboard can violate; [`Invariant::description`] says
/// what each one forbids.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Invariant {
    ConformsTo,
    Version,
    MarkerName,
    DuplicateMarker,
    KeyId,
    DuplicateKeyId,
    FlickId,
    DuplicateFlickId,
    FlickDirections,
    IndexRange,
    LongPressDefault,
    MultiTapSelf,
    GapKey,
    ModifierBits,
    ModifierCombination,
    ModifierOrder,
    LayerModifiers,
    OverlappingLayers,
    MultipleOther,
    ExtraModifierUnbound,
    ExtraModifierKeys,
    KeyName,
    FormScanCodes,
    RowsExceedForm,
    TouchWidth,
    TouchBase,
    DuplicateTouchLayerId,
    LayerId,
    EmptyRules,
    EmptyMatch,
    Quantifier,
    PatternTree,
    Captures,
    NestedCapture,
    CaptureNumbering,
    ReplacementGroup,
    MapSet,
    ClassRange,
    ReorderShape,
    ReorderPriority,
    NotNfd,
    NfdUnchecked,
    ContextTooLong,
    ContextLen,
}

impl Invariant {
    pub const fn description(self) -> &'static str {
        use Invariant::*;
        match self {
            ConformsTo => "conformsTo is outside 45-49",
            Version => "version is not a semantic version",
            MarkerName => "marker name is not an NMTOKEN",
            DuplicateMarker => "marker name occurs twice",
            KeyId => "key id is not an NMTOKEN",
            DuplicateKeyId => "key id is not unique",
            FlickId => "flick id is not an NMTOKEN",
            DuplicateFlickId => "flick id is not unique",
            FlickDirections => "flick segment has no directions",
            IndexRange => "index out of range",
            LongPressDefault => "long_press_default is not in long_press",
            MultiTapSelf => "key lists itself in multi_tap",
            GapKey => "gap key has an output, layer_id, gesture or display",
            ModifierBits => "modifier set names an unknown component",
            ModifierCombination => "modifier set holds a rejected combination",
            ModifierOrder => "modifier sets are not distinct and sorted",
            LayerModifiers => "hardware layer has no modifier set",
            OverlappingLayers => "non-native hardware layers overlap",
            MultipleOther => "more than one hardware layer has other",
            ExtraModifierUnbound => "extra modifier component has no binding",
            ExtraModifierKeys => "extra modifier keys exceed three or repeat",
            KeyName => "unknown Windows key name",
            FormScanCodes => "scan code occurs twice in the form",
            RowsExceedForm => "hardware rows do not fit the form",
            TouchWidth => "touch set widths are not ascending, distinct and 1-999",
            TouchBase => "touch set base is not its base layer",
            DuplicateTouchLayerId => "touch layer id is not unique in its set",
            LayerId => "layer_id names no touch layer",
            EmptyRules => "rules group is empty",
            EmptyMatch => "pattern can match the empty text",
            Quantifier => "quantifier out of bounds",
            PatternTree => "pattern nodes are malformed",
            Captures => "pattern has more than nine captures",
            NestedCapture => "capture nested in a capture",
            CaptureNumbering => "captures are not numbered in opening order",
            ReplacementGroup => "replacement names a missing group",
            MapSet => "mapped set does not match its capture",
            ClassRange => "class range is reversed",
            ReorderShape => "reorder rule lists are not padded to from",
            ReorderPriority => "reorder rules are not in priority order",
            NotNfd => "text is not NFD",
            NfdUnchecked => "normalization is enabled but NFD cannot be checked",
            ContextTooLong => "context_len exceeds 64",
            ContextLen => "context_len is not one more than the longest match",
        }
    }
}

/// Which transform list a group belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TransformList {
    Simple,
    Backspace,
}

/// Where in a keyboard an invariant is violated. Indices are positions in
/// the named table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Site {
    Keyboard,
    Marker(usize),
    Key(usize),
    Flick(usize),
    Display(usize),
    Form,
    HardwareLayer(usize),
    TouchSet(usize),
    TouchLayer {
        set: usize,
        layer: usize,
    },
    Set(usize),
    Class(usize),
    Group {
        list: TransformList,
        group: usize,
    },
    Rule {
        list: TransformList,
        group: usize,
        rule: usize,
    },
    Decimal,
    Flush(MarkerIndex),
    DeadKeyName(MarkerIndex),
    Windows,
}

impl fmt::Display for Site {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let list = |l: &TransformList| match l {
            TransformList::Simple => "simple",
            TransformList::Backspace => "backspace",
        };
        match self {
            Site::Keyboard => f.write_str("keyboard"),
            Site::Marker(i) => write!(f, "markers[{i}]"),
            Site::Key(i) => write!(f, "keys[{i}]"),
            Site::Flick(i) => write!(f, "flicks[{i}]"),
            Site::Display(i) => write!(f, "displays[{i}]"),
            Site::Form => f.write_str("hardware.form"),
            Site::HardwareLayer(i) => write!(f, "hardware.layers[{i}]"),
            Site::TouchSet(i) => write!(f, "touch[{i}]"),
            Site::TouchLayer { set, layer } => write!(f, "touch[{set}].layers[{layer}]"),
            Site::Set(i) => write!(f, "sets[{i}]"),
            Site::Class(i) => write!(f, "classes[{i}]"),
            Site::Group { list: l, group } => write!(f, "{}[{group}]", list(l)),
            Site::Rule {
                list: l,
                group,
                rule,
            } => write!(f, "{}[{group}][{rule}]", list(l)),
            Site::Decimal => f.write_str("decimal"),
            Site::Flush(m) => write!(f, "flush[{m}]"),
            Site::DeadKeyName(m) => write!(f, "dead_key_names[{m}]"),
            Site::Windows => f.write_str("windows"),
        }
    }
}

/// A violated invariant and where it is violated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InvariantError {
    pub invariant: Invariant,
    pub site: Site,
}

impl fmt::Display for InvariantError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} at {}", self.invariant.description(), self.site)
    }
}

impl core::error::Error for InvariantError {}

type Check = Result<(), InvariantError>;

fn fail<T>(invariant: Invariant, site: Site) -> Result<T, InvariantError> {
    Err(InvariantError { invariant, site })
}

fn ensure(ok: bool, invariant: Invariant, site: Site) -> Check {
    if ok { Ok(()) } else { fail(invariant, site) }
}

fn in_range(index: impl Into<usize>, len: usize, site: Site) -> Check {
    ensure(index.into() < len, Invariant::IndexRange, site)
}

/// Whether `s` is a semantic version 2.0.0.
fn is_semver(s: &str) -> bool {
    let numeric = |p: &str| {
        !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()) && (p == "0" || !p.starts_with('0'))
    };
    let ident =
        |p: &str| !p.is_empty() && p.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-');
    let (rest, build) = match s.split_once('+') {
        Some((r, b)) => (r, Some(b)),
        None => (s, None),
    };
    let (core, pre) = match rest.split_once('-') {
        Some((c, p)) => (c, Some(p)),
        None => (rest, None),
    };
    let core_ok = {
        let parts: Vec<&str> = core.split('.').collect();
        parts.len() == 3 && parts.iter().all(|p| numeric(p))
    };
    let pre_ok = pre.is_none_or(|p| {
        p.split('.')
            .all(|id| ident(id) && (!id.bytes().all(|b| b.is_ascii_digit()) || numeric(id)))
    });
    let build_ok = build.is_none_or(|b| b.split('.').all(ident));
    core_ok && pre_ok && build_ok
}

/// The position of the first item equal to an earlier one.
fn first_repeat<T: Ord>(items: impl IntoIterator<Item = T>) -> Option<usize> {
    let mut seen = BTreeSet::new();
    items.into_iter().position(|item| !seen.insert(item))
}

fn check_text_markers(text: &Text, markers: usize, site: Site) -> Check {
    text.markers().try_for_each(|m| in_range(m, markers, site))
}

fn check_modifiers(m: Modifiers, windows_extras: usize, site: Site) -> Check {
    ensure(m.is_well_formed(), Invariant::ModifierBits, site)?;
    ensure(
        m.rejected_combination().is_none(),
        Invariant::ModifierCombination,
        site,
    )?;
    let unbound = m
        .components()
        .filter_map(|c| c.extra_number())
        .any(|n| usize::from(n) > windows_extras);
    ensure(!unbound, Invariant::ExtraModifierUnbound, site)
}

fn check_ranges(ranges: &[ClassRange], site: Site) -> Check {
    ranges
        .iter()
        .try_for_each(|r| ensure(r.lo <= r.hi, Invariant::ClassRange, site))
}

impl Keyboard {
    // [spec:kbdgen:req:ldml.model.context-len+1]
    /// One more than the largest number of text elements that any pattern
    /// of `simple` or `backspace` can match. A reorder rule's pattern is
    /// its `before` followed by its `from`, so a reorder group sees the
    /// cluster those name. Every pattern is bounded, so this is finite; it
    /// saturates rather than overflows.
    pub fn computed_context_len(&self) -> Result<usize, InvariantError> {
        let mut longest: usize = 0;
        for (list, groups) in [
            (TransformList::Simple, &self.simple),
            (TransformList::Backspace, &self.backspace),
        ] {
            for (g, group) in groups.iter().enumerate() {
                let rules = match group {
                    TransformGroup::Rules(rules) => rules,
                    TransformGroup::Reorder(rules) => {
                        for rule in rules {
                            longest =
                                longest.max(rule.before.len().saturating_add(rule.from.len()));
                        }
                        continue;
                    }
                };
                for (r, rule) in rules.iter().enumerate() {
                    let info = rule
                        .from
                        .analyze(&self.sets, self.classes.len(), self.markers.len())
                        .map_err(|e| InvariantError {
                            invariant: e.invariant,
                            site: Site::Rule {
                                list,
                                group: g,
                                rule: r,
                            },
                        })?;
                    longest = longest.max(info.max_len);
                }
            }
        }
        Ok(longest.saturating_add(1))
    }

    // [spec:kbdgen:req:ldml.model.invariants+1]
    /// Checks every invariant of `ldml.model.invariants`, together with
    /// the structural rules of the model's definitions, and returns the
    /// first violation. It never panics.
    ///
    /// `nfd` checks the texts of a keyboard with `Enabled` normalization
    /// (`ldml.model.nfd`). Without one, such a keyboard is rejected with
    /// [`Invariant::NfdUnchecked`]; a `Disabled` keyboard needs none.
    pub fn validate(&self, nfd: Option<&dyn NfdCheck>) -> Check {
        self.check_header()?;
        self.check_markers()?;
        self.check_keys()?;
        self.check_flicks()?;
        self.check_displays()?;
        self.check_windows()?;
        if let Some(hardware) = &self.hardware {
            self.check_hardware(hardware)?;
        }
        self.check_touch()?;
        self.check_layer_ids()?;
        self.check_sets_and_classes()?;
        self.check_transforms(TransformList::Simple)?;
        self.check_transforms(TransformList::Backspace)?;
        self.check_extension_texts()?;
        if self.normalization == Normalization::Enabled {
            match nfd {
                Some(nfd) => self.check_nfd(nfd)?,
                None => fail(Invariant::NfdUnchecked, Site::Keyboard)?,
            }
        }
        let computed = self.computed_context_len()?;
        ensure(
            computed <= usize::from(MAX_CONTEXT_LEN),
            Invariant::ContextTooLong,
            Site::Keyboard,
        )?;
        ensure(
            usize::from(self.context_len) == computed,
            Invariant::ContextLen,
            Site::Keyboard,
        )
    }

    fn check_header(&self) -> Check {
        ensure(
            (45..=49).contains(&self.conforms_to),
            Invariant::ConformsTo,
            Site::Keyboard,
        )?;
        ensure(
            self.version.as_deref().is_none_or(is_semver),
            Invariant::Version,
            Site::Keyboard,
        )
    }

    fn check_markers(&self) -> Check {
        for (i, name) in self.markers.iter().enumerate() {
            ensure(is_nmtoken(name), Invariant::MarkerName, Site::Marker(i))?;
        }
        match first_repeat(&self.markers) {
            Some(i) => fail(Invariant::DuplicateMarker, Site::Marker(i)),
            None => Ok(()),
        }
    }

    fn check_keys(&self) -> Check {
        let keys = self.keys.len();
        for (i, key) in self.keys.iter().enumerate() {
            let site = Site::Key(i);
            ensure(is_nmtoken(&key.id), Invariant::KeyId, site)?;
            check_text_markers(&key.output, self.markers.len(), site)?;
            for &k in key.long_press.iter().chain(&key.multi_tap) {
                in_range(k, keys, site)?;
            }
            if let Some(default) = key.long_press_default {
                in_range(default, keys, site)?;
                ensure(
                    key.long_press.contains(&default),
                    Invariant::LongPressDefault,
                    site,
                )?;
            }
            let own = KeyIndex::try_from(i).ok();
            ensure(
                own.is_none_or(|own| !key.multi_tap.contains(&own)),
                Invariant::MultiTapSelf,
                site,
            )?;
            if let Some(flick) = key.flick {
                in_range(flick, self.flicks.len(), site)?;
            }
            if key.gap {
                let bare = key.output.is_empty()
                    && key.layer_id.is_none()
                    && key.long_press.is_empty()
                    && key.long_press_default.is_none()
                    && key.multi_tap.is_empty()
                    && key.flick.is_none();
                ensure(bare, Invariant::GapKey, site)?;
            }
        }
        match first_repeat(self.keys.iter().map(|k| &k.id)) {
            Some(i) => fail(Invariant::DuplicateKeyId, Site::Key(i)),
            None => Ok(()),
        }
    }

    fn check_flicks(&self) -> Check {
        for (i, flick) in self.flicks.iter().enumerate() {
            let site = Site::Flick(i);
            ensure(is_nmtoken(&flick.id), Invariant::FlickId, site)?;
            for segment in &flick.segments {
                ensure(
                    !segment.directions.is_empty(),
                    Invariant::FlickDirections,
                    site,
                )?;
                in_range(segment.key, self.keys.len(), site)?;
            }
        }
        match first_repeat(self.flicks.iter().map(|f| &f.id)) {
            Some(i) => fail(Invariant::DuplicateFlickId, Site::Flick(i)),
            None => Ok(()),
        }
    }

    fn check_displays(&self) -> Check {
        for (i, display) in self.displays.entries.iter().enumerate() {
            let site = Site::Display(i);
            match &display.target {
                DisplayTarget::Output(text) => {
                    check_text_markers(text, self.markers.len(), site)?;
                }
                DisplayTarget::Key(k) => {
                    let Some(key) = self.key(*k) else {
                        return fail(Invariant::IndexRange, site);
                    };
                    ensure(!key.gap, Invariant::GapKey, site)?;
                }
            }
        }
        Ok(())
    }

    fn check_windows(&self) -> Check {
        let extras = &self.windows.extra_modifiers;
        let distinct = first_repeat(extras).is_none();
        ensure(
            extras.len() <= 3 && distinct,
            Invariant::ExtraModifierKeys,
            Site::Windows,
        )?;
        for name in self.windows.key_names.keys() {
            ensure(
                WINDOWS_KEY_NAMES.contains(&name.as_str()),
                Invariant::KeyName,
                Site::Windows,
            )?;
        }
        Ok(())
    }

    fn check_hardware(&self, hardware: &Hardware) -> Check {
        let form = &hardware.form;
        let repeated = first_repeat(form.rows.iter().flatten()).is_some();
        ensure(!repeated, Invariant::FormScanCodes, Site::Form)?;

        let extras = self.windows.extra_modifiers.len();
        let mut other_seen = false;
        for (i, layer) in hardware.layers.iter().enumerate() {
            let site = Site::HardwareLayer(i);
            ensure(!layer.modifiers.is_empty(), Invariant::LayerModifiers, site)?;
            for set in &layer.modifiers {
                match set {
                    ModifierSet::Set(m) => check_modifiers(*m, extras, site)?,
                    ModifierSet::Other => {
                        ensure(!other_seen, Invariant::MultipleOther, site)?;
                        other_seen = true;
                    }
                }
            }
            let sorted = layer
                .modifiers
                .windows(2)
                .all(|w| matches!(w, [a, b] if a < b));
            ensure(sorted, Invariant::ModifierOrder, site)?;

            let fits = layer.rows.len() <= form.rows.len()
                && layer
                    .rows
                    .iter()
                    .zip(&form.rows)
                    .all(|(row, form_row)| row.len() <= form_row.len());
            ensure(fits, Invariant::RowsExceedForm, site)?;
            for &k in layer.rows.iter().flatten() {
                in_range(k, self.keys.len(), site)?;
            }
        }

        let typing: Vec<(usize, Modifiers)> = hardware
            .layers
            .iter()
            .enumerate()
            .filter(|(_, l)| !l.is_native())
            .flat_map(|(i, l)| {
                l.modifiers.iter().filter_map(move |s| match s {
                    ModifierSet::Set(m) => Some((i, *m)),
                    ModifierSet::Other => None,
                })
            })
            .collect();
        for (n, (layer, set)) in typing.iter().enumerate() {
            let clash = typing
                .iter()
                .take(n)
                .any(|(other, o)| other != layer && o.overlaps(*set));
            ensure(
                !clash,
                Invariant::OverlappingLayers,
                Site::HardwareLayer(*layer),
            )?;
        }
        Ok(())
    }

    fn check_touch(&self) -> Check {
        let mut previous: Option<Option<u16>> = None;
        for (s, set) in self.touch.iter().enumerate() {
            let site = Site::TouchSet(s);
            let width_ok = match set.min_device_width {
                None => previous.is_none(),
                Some(w) => {
                    (1..=999).contains(&w) && previous.is_none_or(|p| p.is_none_or(|p| p < w))
                }
            };
            ensure(width_ok, Invariant::TouchWidth, site)?;
            previous = Some(set.min_device_width);

            if let Some(l) = first_repeat(set.layers.iter().map(|l| &l.id)) {
                return fail(
                    Invariant::DuplicateTouchLayerId,
                    Site::TouchLayer { set: s, layer: l },
                );
            }
            for (l, layer) in set.layers.iter().enumerate() {
                let site = Site::TouchLayer { set: s, layer: l };
                for &k in layer.rows.iter().flatten() {
                    in_range(k, self.keys.len(), site)?;
                }
            }
            let base = set.layers.get(usize::from(set.base));
            ensure(
                base.is_some_and(|b| b.id == "base"),
                Invariant::TouchBase,
                site,
            )?;
        }
        Ok(())
    }

    /// A `layer_id` names a layer of some touch set. A keyboard without
    /// touch sets presents its hardware set as touch (`ldml.model.touch`),
    /// so its `layer_id`s name hardware layer ids instead.
    fn check_layer_ids(&self) -> Check {
        for (i, key) in self.keys.iter().enumerate() {
            let Some(id) = &key.layer_id else { continue };
            let found = if self.touch.is_empty() {
                self.hardware
                    .as_ref()
                    .is_some_and(|h| h.layers.iter().any(|l| l.id.as_ref() == Some(id)))
            } else {
                self.touch
                    .iter()
                    .any(|set| set.layers.iter().any(|l| &l.id == id))
            };
            ensure(found, Invariant::LayerId, Site::Key(i))?;
        }
        Ok(())
    }

    fn check_sets_and_classes(&self) -> Check {
        let markers = self.markers.len();
        for (i, set) in self.sets.iter().enumerate() {
            for text in set {
                check_text_markers(text, markers, Site::Set(i))?;
            }
        }
        for (i, class) in self.classes.iter().enumerate() {
            let site = Site::Class(i);
            check_ranges(&class.ranges, site)?;
            for &m in &class.markers {
                in_range(m, markers, site)?;
            }
        }
        Ok(())
    }

    fn transform_list(&self, list: TransformList) -> &[TransformGroup] {
        match list {
            TransformList::Simple => &self.simple,
            TransformList::Backspace => &self.backspace,
        }
    }

    fn check_transforms(&self, list: TransformList) -> Check {
        for (g, group) in self.transform_list(list).iter().enumerate() {
            match group {
                TransformGroup::Rules(rules) => {
                    ensure(
                        !rules.is_empty(),
                        Invariant::EmptyRules,
                        Site::Group { list, group: g },
                    )?;
                    for (r, rule) in rules.iter().enumerate() {
                        let site = Site::Rule {
                            list,
                            group: g,
                            rule: r,
                        };
                        self.check_rule(rule, site)?;
                    }
                }
                TransformGroup::Reorder(rules) => {
                    for (r, rule) in rules.iter().enumerate() {
                        let site = Site::Rule {
                            list,
                            group: g,
                            rule: r,
                        };
                        let n = rule.from.len();
                        let padded = n > 0
                            && rule.order.len() == n
                            && rule.tertiary.len() == n
                            && rule.tertiary_base.len() == n
                            && rule.pre_base.len() == n;
                        ensure(padded, Invariant::ReorderShape, site)?;
                        for class in rule.before.iter().chain(&rule.from) {
                            if let ReorderClass::Ranges(ranges) = class {
                                check_ranges(ranges, site)?;
                            }
                        }
                    }
                    let sorted = rules
                        .windows(2)
                        .all(|w| matches!(w, [a, b] if a.priority() >= b.priority()));
                    ensure(
                        sorted,
                        Invariant::ReorderPriority,
                        Site::Group { list, group: g },
                    )?;
                }
            }
        }
        Ok(())
    }

    fn check_rule(&self, rule: &crate::transforms::Rule, site: Site) -> Check {
        let info = rule
            .from
            .analyze(&self.sets, self.classes.len(), self.markers.len())
            .map_err(|e| InvariantError {
                invariant: e.invariant,
                site,
            })?;
        ensure(info.min_len != Some(0), Invariant::EmptyMatch, site)?;
        for item in &rule.to {
            match item {
                ReplacementItem::Text(text) => {
                    check_text_markers(text, self.markers.len(), site)?;
                }
                ReplacementItem::Group(n) => {
                    ensure(
                        usize::from(*n) <= info.captures.len(),
                        Invariant::ReplacementGroup,
                        site,
                    )?;
                }
                ReplacementItem::MapSet { group, from, to } => {
                    let node = usize::from(*group)
                        .checked_sub(1)
                        .and_then(|n| info.captures.get(n));
                    let Some(&node) = node else {
                        return fail(Invariant::ReplacementGroup, site);
                    };
                    let captured = rule
                        .from
                        .nodes
                        .get(usize::from(node))
                        .map(|n| n.alternatives.as_slice());
                    let single_set = match captured {
                        Some([sequence]) => match sequence.as_slice() {
                            [item] if item.min == 1 && item.max == 1 => match item.atom {
                                Atom::Set(s) => Some(s),
                                _ => None,
                            },
                            _ => None,
                        },
                        _ => None,
                    };
                    let from_set = self.sets.get(usize::from(*from));
                    let to_set = self.sets.get(usize::from(*to));
                    let ok = single_set == Some(*from)
                        && matches!((from_set, to_set), (Some(a), Some(b)) if a.len() == b.len());
                    ensure(ok, Invariant::MapSet, site)?;
                }
            }
        }
        Ok(())
    }

    fn check_extension_texts(&self) -> Check {
        let markers = self.markers.len();
        if let Some(decimal) = &self.decimal {
            check_text_markers(decimal, markers, Site::Decimal)?;
        }
        for &m in self.flush.keys() {
            in_range(m, markers, Site::Flush(m))?;
        }
        for &m in self.dead_key_names.keys() {
            in_range(m, markers, Site::DeadKeyName(m))?;
        }
        Ok(())
    }

    // [spec:kbdgen:req:ldml.model.nfd+1]
    /// With `Enabled` normalization: key outputs, pattern `Char` runs and
    /// `Set` items, replacement texts, display targets, `decimal` and
    /// `flush` texts are NFD, and every class range and reorder class holds
    /// only NFD scalar values.
    ///
    /// Markers glue to the following scalar value, so the marker algorithm
    /// of §Normalization and Markers leaves a text unchanged exactly when
    /// its plain text is NFD; that is what is checked. A `Char` run is a
    /// maximal run of unquantified `Char` items in one sequence; a
    /// quantified `Char` is checked alone.
    fn check_nfd(&self, nfd: &dyn NfdCheck) -> Check {
        let text_ok =
            |t: &Text, site: Site| ensure(nfd.is_nfd(&t.plain()), Invariant::NotNfd, site);
        let ranges_ok = |ranges: &[ClassRange], site: Site| {
            ranges.iter().try_for_each(|r| {
                ensure(
                    nfd.first_non_nfd(r.lo, r.hi).is_none(),
                    Invariant::NotNfd,
                    site,
                )
            })
        };

        for (i, key) in self.keys.iter().enumerate() {
            text_ok(&key.output, Site::Key(i))?;
        }
        for (i, display) in self.displays.entries.iter().enumerate() {
            if let DisplayTarget::Output(t) = &display.target {
                text_ok(t, Site::Display(i))?;
            }
        }
        for (i, set) in self.sets.iter().enumerate() {
            for t in set {
                text_ok(t, Site::Set(i))?;
            }
        }
        for (i, class) in self.classes.iter().enumerate() {
            ranges_ok(&class.ranges, Site::Class(i))?;
        }
        for list in [TransformList::Simple, TransformList::Backspace] {
            for (g, group) in self.transform_list(list).iter().enumerate() {
                match group {
                    TransformGroup::Rules(rules) => {
                        for (r, rule) in rules.iter().enumerate() {
                            let site = Site::Rule {
                                list,
                                group: g,
                                rule: r,
                            };
                            for node in &rule.from.nodes {
                                for sequence in &node.alternatives {
                                    let mut run = String::new();
                                    for item in sequence {
                                        match item.atom {
                                            Atom::Char(c) if item.min == 1 && item.max == 1 => {
                                                run.push(c);
                                                continue;
                                            }
                                            Atom::Char(c) => {
                                                let mut buf = [0u8; 4];
                                                ensure(
                                                    nfd.is_nfd(c.encode_utf8(&mut buf)),
                                                    Invariant::NotNfd,
                                                    site,
                                                )?;
                                            }
                                            _ => {}
                                        }
                                        ensure(nfd.is_nfd(&run), Invariant::NotNfd, site)?;
                                        run.clear();
                                    }
                                    ensure(nfd.is_nfd(&run), Invariant::NotNfd, site)?;
                                }
                            }
                            for item in &rule.to {
                                if let ReplacementItem::Text(t) = item {
                                    text_ok(t, site)?;
                                }
                            }
                        }
                    }
                    TransformGroup::Reorder(rules) => {
                        for (r, rule) in rules.iter().enumerate() {
                            let site = Site::Rule {
                                list,
                                group: g,
                                rule: r,
                            };
                            for class in rule.before.iter().chain(&rule.from) {
                                match class {
                                    ReorderClass::Char(c) => {
                                        ranges_ok(&[ClassRange::single(*c)], site)?
                                    }
                                    ReorderClass::Ranges(ranges) => ranges_ok(ranges, site)?,
                                }
                            }
                        }
                    }
                }
            }
        }
        if let Some(decimal) = &self.decimal {
            text_ok(decimal, Site::Decimal)?;
        }
        for (&m, text) in &self.flush {
            ensure(nfd.is_nfd(text), Invariant::NotNfd, Site::Flush(m))?;
        }
        Ok(())
    }
}
