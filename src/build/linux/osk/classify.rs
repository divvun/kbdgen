//! What the on-screen keyboards do not show or type
//! (`ldml.linux.classify.touch`), reported once per layout, OSK and
//! feature. plasma-keyboard and Stevia type their layouts themselves, and
//! GNOME Shell types through the IBus engine only for the engine's source,
//! so a presented set loses what only the engine or a hardware key gives.

use std::collections::BTreeSet;

use kbd_model::{Component, KeyIndex, Keyboard, MarkerIndex, ModifierSet, TransformGroup};
use language_tags::LanguageTag;

use super::presented::{Face, Item, Level, LevelKind, Presented, face, positions};
use crate::build::diag::Diagnostics;
use crate::build::macos::adapter::Category;
use kbd_model::is_generated_backspace;

/// An on-screen keyboard, for the messages that concern only it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Osk<'a> {
    /// GNOME Shell, with the markers whose dead keys its layout sends as
    /// dead keysyms (`ldml.linux.touch.gnome`); the others type their
    /// standalone.
    GnomeShell(&'a BTreeSet<MarkerIndex>),
    PlasmaKeyboard,
    Stevia,
}

impl Osk<'_> {
    pub fn name(&self) -> &'static str {
        match self {
            Osk::GnomeShell(_) => "GNOME Shell",
            Osk::PlasmaKeyboard => "plasma-keyboard",
            Osk::Stevia => "Stevia",
        }
    }
}

/// A dead key the presented set shows, once per marker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShownDeadKey {
    pub marker: MarkerIndex,
    pub display: String,
    /// Whether it has no standalone, so OSKs without dead keys leave it out.
    pub no_standalone: bool,
}

/// Everything of `ldml.linux.classify.touch` for one layout.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Classification {
    /// Typing layers other than the default and shift levels, by their
    /// modifier sets.
    pub hidden_layers: Category,
    /// Shown keys with flicks, multi-tap or a default long-press candidate.
    pub gestures: Category,
    /// Shown keys mixing text and markers, by key id.
    pub mixed: Category,
    /// Long-press candidates of shown keys that are not plain text, by key
    /// id.
    pub marked_candidates: Category,
    pub dead_keys: Vec<ShownDeadKey>,
    /// Shift-level texts that plasma-keyboard's uppercasing changes, as
    /// `before → after`.
    pub uppercased: Category,
    /// Shown texts of more than one scalar value.
    pub multi_scalar: Category,
    /// Whether K has `backspace` rules other than the generated one.
    pub backspace_rules: bool,
}

/// A layer's modifier sets as `ldml.yaml.modifier-names` writes them.
fn sets_name(sets: &[ModifierSet]) -> String {
    let name = |set: &ModifierSet| match set {
        ModifierSet::Other => "other".to_owned(),
        ModifierSet::Set(set) if set.is_empty() => "none".to_owned(),
        ModifierSet::Set(set) => set
            .components()
            .map(Component::name)
            .collect::<Vec<_>>()
            .join(" "),
    };
    sets.iter().map(name).collect::<Vec<_>>().join(", ")
}

/// What plasma-keyboard commits for `text` while shifted: its locale's
/// uppercase when it is one UTF-16 unit, else `text` itself
/// (`textcomposers/directtextcomposer.cpp`). `QLocale::toUpper` maps `i`
/// to `İ` for Turkish and Azerbaijani, and otherwise as Unicode does.
pub fn plasma_shifted(text: &str, tag: &LanguageTag) -> String {
    let mut chars = text.chars();
    match (chars.next(), chars.next()) {
        (Some('i'), None) if matches!(tag.primary_language(), "tr" | "az") => "İ".to_owned(),
        (Some(c), None) if c.len_utf16() == 1 => c.to_uppercase().collect(),
        _ => text.to_owned(),
    }
}

/// The texts a level commits: outputs, standalones and candidates.
fn texts(level: &Level) -> impl Iterator<Item = &str> {
    level.rows.iter().flatten().flat_map(|item| {
        let (first, candidates) = match item {
            Item::Text(key) => (Some(key.output.as_str()), key.candidates.as_slice()),
            Item::Dead(key) => (
                Some(key.standalone.as_str()).filter(|s| !s.is_empty()),
                key.candidates.as_slice(),
            ),
            _ => (None, [].as_slice()),
        };
        first
            .into_iter()
            .chain(candidates.iter().map(String::as_str))
    })
}

/// The keys at the shown positions of the default and shift levels, each
/// once.
fn shown_keys(keyboard: &Keyboard, presented: &Presented) -> Vec<KeyIndex> {
    let Some(hardware) = &keyboard.hardware else {
        return Vec::new();
    };
    let mut keys = Vec::new();
    for layer in presented
        .hardware_levels()
        .filter_map(|level| level.layer)
        .filter_map(|layer| hardware.layers.get(layer))
    {
        for (r, c) in positions(hardware).into_iter().flatten() {
            if let Some(key) = layer.key_at(r, c)
                && !keys.contains(&key)
            {
                keys.push(key);
            }
        }
    }
    keys
}

// [spec:kbdgen:req:ldml.linux.classify.touch]
pub fn classify(keyboard: &Keyboard, presented: &Presented, tag: &LanguageTag) -> Classification {
    let mut result = Classification::default();
    let shown_layers: Vec<usize> = presented
        .hardware_levels()
        .filter_map(|level| level.layer)
        .collect();
    if let Some(hardware) = &keyboard.hardware {
        for (i, layer) in hardware.layers.iter().enumerate() {
            if !layer.is_native() && !shown_layers.contains(&i) {
                result.hidden_layers.add(|| sets_name(&layer.modifiers));
            }
        }
    }
    for index in shown_keys(keyboard, presented) {
        let Some(key) = keyboard.key(index) else {
            continue;
        };
        if face(key) == Face::Mixed {
            result.mixed.add(|| key.id.clone());
            continue;
        }
        if face(key) == Face::Blank {
            continue;
        }
        if key.flick.is_some() || !key.multi_tap.is_empty() || key.long_press_default.is_some() {
            result.gestures.add(|| key.id.clone());
        }
        for candidate in key.long_press.iter().filter_map(|&i| keyboard.key(i)) {
            if matches!(face(candidate), Face::Dead(_) | Face::Mixed) {
                result.marked_candidates.add(|| candidate.id.clone());
            }
        }
    }
    let mut multi = BTreeSet::new();
    let mut shifted = BTreeSet::new();
    for level in presented.hardware_levels() {
        for item in level.rows.iter().flatten() {
            if let Item::Dead(dead) = item
                && !result.dead_keys.iter().any(|d| d.marker == dead.marker)
            {
                result.dead_keys.push(ShownDeadKey {
                    marker: dead.marker,
                    display: dead.display.clone(),
                    no_standalone: dead.standalone.is_empty(),
                });
            }
        }
        for text in texts(level) {
            if text.chars().nth(1).is_some() && multi.insert(text) {
                result.multi_scalar.add(|| text.to_owned());
            }
            if level.kind == LevelKind::Shift && shifted.insert(text) {
                let upper = plasma_shifted(text, tag);
                if upper != text {
                    result.uppercased.add(|| format!("{text} → {upper}"));
                }
            }
        }
    }
    result.backspace_rules = keyboard.backspace.iter().any(|group| match group {
        TransformGroup::Rules(rules) => rules.iter().any(|rule| !is_generated_backspace(rule)),
        TransformGroup::Reorder(_) => true,
    });
    result
}

fn warn(osk: &Osk, what: &str, category: &Category, diag: &mut Diagnostics) {
    if category.count > 0 {
        diag.warn(format!(
            "{} {what} (e.g. {})",
            osk.name(),
            category.examples.join("; ")
        ));
    }
}

/// The warnings of `ldml.linux.classify.touch` that concern `osk`, each
/// with its count and up to ten examples.
// [spec:kbdgen:req:ldml.linux.classify.touch]
pub fn report(classification: &Classification, osk: Osk, diag: &mut Diagnostics) {
    let c = classification;
    warn(
        &osk,
        &format!(
            "does not show {} layer(s) other than the default and shift levels, which only hardware keys type",
            c.hidden_layers.count
        ),
        &c.hidden_layers,
        diag,
    );
    warn(
        &osk,
        &format!(
            "has no flicks, multi-tap or default long-press candidate for {} key(s)",
            c.gestures.count
        ),
        &c.gestures,
        diag,
    );
    let mut dead = Category::default();
    for key in &c.dead_keys {
        let sent = match osk {
            Osk::GnomeShell(keysyms) => keysyms.contains(&key.marker),
            Osk::PlasmaKeyboard | Osk::Stevia => false,
        };
        if !sent {
            dead.add(|| match key.no_standalone {
                true => format!("{} (left out)", key.display),
                false => key.display.clone(),
            });
        }
    }
    let which = match osk {
        Osk::GnomeShell(_) => " without a dead keysym at Level1 to 3 of the XKB layout",
        Osk::PlasmaKeyboard | Osk::Stevia => "",
    };
    warn(
        &osk,
        &format!(
            "types {} dead key(s){which} as their standalone output, which composes nothing, or leaves them out where they have none",
            dead.count
        ),
        &dead,
        diag,
    );
    warn(
        &osk,
        &format!(
            "leaves out {} key(s) mixing text and markers",
            c.mixed.count
        ),
        &c.mixed,
        diag,
    );
    warn(
        &osk,
        &format!(
            "leaves out {} long-press candidate(s) holding markers",
            c.marked_candidates.count
        ),
        &c.marked_candidates,
        diag,
    );
    if osk == Osk::PlasmaKeyboard {
        warn(
            &osk,
            &format!(
                "uppercases {} shift-level text(s) for the locale, so they type otherwise than the shift layer",
                c.uppercased.count
            ),
            &c.uppercased,
            diag,
        );
    }
}

/// The information messages of `ldml.linux.classify.touch`, once per
/// layout, each where the layout has what it concerns.
// [spec:kbdgen:req:ldml.linux.classify.touch]
pub fn inform(classification: &Classification, diag: &mut Diagnostics) {
    let c = classification;
    if c.multi_scalar.count > 0 {
        diag.inform(format!(
            "GNOME Shell sends {} on-screen text(s) of several scalar values to the IBus engine one scalar value at a time (e.g. {})",
            c.multi_scalar.count,
            c.multi_scalar.examples.join("; ")
        ));
    }
    if !c.dead_keys.is_empty() {
        diag.inform(
            "on GNOME Shell, a dead key pressed on screen composes only with the layout's IBus source; with its XKB source, only with a hardware key",
        );
    }
    if c.backspace_rules {
        diag.inform(
            "GNOME Shell's on-screen Backspace skips the layout's backspace transforms while nothing is shown as preedit",
        );
    }
    diag.inform(
        "plasma-keyboard and Stevia type their on-screen layouts without the IBus engine, so no transform applies there",
    );
}
