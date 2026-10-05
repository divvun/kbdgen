//! Export: a keyboard model and its extensions to a keyboard3 source
//! document (`ldml.xml.export`), with the superset in kbdgen's namespace
//! (`ldml.xml.special`).

use std::collections::BTreeMap;

use kbd_model::{
    Atom, BottomRow, Class, Component, DisplayTarget, Emoji, Host, Key, Keyboard, ModifierSet,
    Normalization, Pattern, ReorderClass, ReorderRule, ReplacementItem, Text, TextElem,
    TransformGroup, TransformList,
};
use xmlem::{Declaration, Document, Element};

use crate::escape::{Piece, encode_plain, encode_text};
use crate::read::keyboard_namespace;
use crate::resolve::{encode_modifiers, implied_form, implied_keys};
use crate::special::{
    Compose, ComposeValue, DeadKey, Extensions, KBDGEN_NS, KBDGEN_PREFIX, Target,
};
use crate::syntax::{
    Atom as SAtom, ClassMember, ClassSyntax, PatternSyntax, Quantified, ReorderElem, ToItem,
    encode_pattern, encode_reorder, encode_replacement, encode_set_items,
};

/// Builds elements in document order with attributes in DTD order.
struct Builder<'a> {
    doc: Document,
    kb: &'a Keyboard,
}

fn add(doc: &mut Document, parent: Element, name: &str, attrs: &[(&str, String)]) -> Element {
    let el = parent.append_new_element(doc, name);
    for (k, v) in attrs {
        el.set_attribute(doc, *k, v);
    }
    el
}

fn kb_name(local: &str) -> String {
    format!("{KBDGEN_PREFIX}:{local}")
}

/// A width in key widths, with at most three decimals and no trailing
/// zeros.
pub fn encode_width(thousandths: u32) -> String {
    let int = thousandths / 1000;
    let frac = thousandths % 1000;
    if frac == 0 {
        int.to_string()
    } else {
        format!("{int}.{frac:03}").trim_end_matches('0').to_string()
    }
}

/// `conformsTo`: 45, or 47 when `info@attribution` is present, unless the
/// keyboard already conforms to a later version, which is kept so that the
/// exported document resolves to the same model.
fn conforms_to(kb: &Keyboard) -> u8 {
    let minimum = if kb.info.attribution.is_some() {
        47
    } else {
        45
    };
    kb.conforms_to.max(minimum)
}

fn set_id(index: u16) -> String {
    format!("s{}", u32::from(index) + 1)
}

/// Whether a hardware layer can only be written in kbdgen's namespace, as
/// a `kbdgen:layer`: it is native-only or uses `cmd` or `extra`n.
pub fn is_kbdgen_layer(sets: &[ModifierSet]) -> bool {
    sets.iter().any(|s| match s {
        ModifierSet::Set(m) => {
            m.is_native()
                || m.components().any(|c| {
                    matches!(
                        c,
                        Component::Cmd | Component::Extra1 | Component::Extra2 | Component::Extra3
                    )
                })
        }
        ModifierSet::Other => false,
    })
}

impl Builder<'_> {
    fn pieces(&self, text: &Text) -> Vec<Piece> {
        text.elements()
            .iter()
            .map(|e| match e {
                TextElem::Char(c) => Piece::Char(*c),
                TextElem::Marker(m) => Piece::Marker(self.marker(*m)),
            })
            .collect()
    }

    fn marker(&self, m: u16) -> String {
        self.kb.marker_name(m).unwrap_or("").to_string()
    }

    fn key_id(&self, k: u16) -> String {
        self.kb
            .key(k)
            .map_or_else(String::new, |key| key.id.clone())
    }

    fn key_ids(&self, keys: &[u16]) -> String {
        keys.iter()
            .map(|k| self.key_id(*k))
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn class(&self, class: &Class) -> ClassSyntax {
        let mut members: Vec<ClassMember> = class
            .ranges
            .iter()
            .map(|r| ClassMember::Range(r.lo, r.hi))
            .collect();
        members.extend(
            class
                .markers
                .iter()
                .map(|m| ClassMember::Marker(self.marker(*m))),
        );
        ClassSyntax {
            negated: class.negated,
            members,
        }
    }

    fn alternatives(&self, pattern: &Pattern, node: u16) -> Vec<Vec<Quantified>> {
        let Some(alternation) = pattern.nodes.get(usize::from(node)) else {
            return Vec::new();
        };
        alternation
            .alternatives
            .iter()
            .map(|sequence| {
                sequence
                    .iter()
                    .map(|item| {
                        let atom = match item.atom {
                            Atom::Char(c) => SAtom::Char(c),
                            Atom::Any => SAtom::Any,
                            Atom::Class(i) => SAtom::Class(
                                self.kb
                                    .classes
                                    .get(usize::from(i))
                                    .map(|c| self.class(c))
                                    .unwrap_or(ClassSyntax {
                                        negated: false,
                                        members: Vec::new(),
                                    }),
                            ),
                            Atom::Fixed(f) => SAtom::Fixed(f),
                            Atom::Marker(m) => SAtom::Marker(self.marker(m)),
                            Atom::AnyMarker => SAtom::AnyMarker,
                            Atom::Set(s) => SAtom::Var(set_id(s)),
                            Atom::Group(n) => SAtom::Group(self.alternatives(pattern, n)),
                            Atom::Capture { node, .. } => {
                                let mut alternatives = self.alternatives(pattern, node);
                                if alternatives.len() == 1 {
                                    SAtom::Capture(alternatives.pop().unwrap_or_default())
                                } else {
                                    SAtom::Capture(vec![Quantified::one(SAtom::Group(
                                        alternatives,
                                    ))])
                                }
                            }
                        };
                        Quantified {
                            atom,
                            min: item.min,
                            max: item.max,
                        }
                    })
                    .collect()
            })
            .collect()
    }

    fn pattern(&self, pattern: &Pattern) -> String {
        encode_pattern(&PatternSyntax {
            anchored: pattern.anchored,
            alternatives: self.alternatives(pattern, 0),
        })
    }

    fn replacement(&self, items: &[ReplacementItem]) -> String {
        let mut out = Vec::new();
        for item in items {
            match item {
                ReplacementItem::Text(text) => {
                    out.extend(self.pieces(text).into_iter().map(|p| match p {
                        Piece::Char(c) => ToItem::Char(c),
                        Piece::Marker(m) => ToItem::Marker(m),
                    }));
                }
                ReplacementItem::Group(n) => out.push(ToItem::Group(*n)),
                ReplacementItem::MapSet { group, to, .. } => out.push(ToItem::MapSet {
                    group: *group,
                    set: set_id(*to),
                }),
            }
        }
        encode_replacement(&out)
    }

    fn header(&mut self, root: Element) {
        let kb = self.kb;
        if !kb.locales.is_empty() {
            let locales = add(&mut self.doc, root, "locales", &[]);
            for l in &kb.locales {
                add(&mut self.doc, locales, "locale", &[("id", l.clone())]);
            }
        }
        if let Some(v) = &kb.version {
            add(&mut self.doc, root, "version", &[("number", v.clone())]);
        }
        let info = &kb.info;
        let mut attrs = vec![("name", info.name.clone())];
        for (k, v) in [
            ("author", &info.author),
            ("layout", &info.layout),
            ("indicator", &info.indicator),
            ("attribution", &info.attribution),
        ] {
            if let Some(v) = v {
                attrs.push((k, v.clone()));
            }
        }
        add(&mut self.doc, root, "info", &attrs);
        if kb.normalization == Normalization::Disabled {
            add(
                &mut self.doc,
                root,
                "settings",
                &[("normalization", "disabled".to_string())],
            );
        }
    }

    fn displays(&mut self, root: Element) {
        let displays = &self.kb.displays;
        if displays.entries.is_empty() && displays.display_base.is_none() {
            return;
        }
        let el = add(&mut self.doc, root, "displays", &[]);
        for d in &displays.entries {
            let target = match &d.target {
                DisplayTarget::Key(k) => ("keyId", self.key_id(*k)),
                DisplayTarget::Output(text) => ("output", encode_text(&self.pieces(text))),
            };
            add(
                &mut self.doc,
                el,
                "display",
                &[target, ("display", encode_plain(&d.display))],
            );
        }
        if let Some(base) = &displays.display_base {
            add(
                &mut self.doc,
                el,
                "displayOptions",
                &[("baseCharacter", encode_plain(base))],
            );
        }
    }

    fn key_attrs(&self, key: &Key) -> Vec<(&'static str, String)> {
        let mut attrs = vec![("id", key.id.clone())];
        if let Some(f) = key.flick.and_then(|f| self.kb.flicks.get(usize::from(f))) {
            attrs.push(("flickId", f.id.clone()));
        }
        if key.gap {
            attrs.push(("gap", "true".to_string()));
        }
        if !key.output.is_empty() {
            attrs.push(("output", encode_text(&self.pieces(&key.output))));
        }
        if !key.long_press.is_empty() {
            attrs.push(("longPressKeyIds", self.key_ids(&key.long_press)));
        }
        if let Some(d) = key.long_press_default {
            attrs.push(("longPressDefaultKeyId", self.key_id(d)));
        }
        if !key.multi_tap.is_empty() {
            attrs.push(("multiTapKeyIds", self.key_ids(&key.multi_tap)));
        }
        if key.stretch {
            attrs.push(("stretch", "true".to_string()));
        }
        if let Some(l) = &key.layer_id {
            attrs.push(("layerId", l.clone()));
        }
        if key.width != kbd_model::DEFAULT_WIDTH {
            attrs.push(("width", encode_width(key.width)));
        }
        attrs
    }

    /// Writes every key but the implied ones that are unchanged and in
    /// their implied position; an overridden implied key is written, and
    /// resolution puts it back in place.
    fn keys(&mut self, root: Element) {
        let implied = implied_keys(self.kb.conforms_to);
        let written: Vec<&Key> = self
            .kb
            .keys
            .iter()
            .enumerate()
            .filter(|(i, k)| implied.get(*i) != Some(*k))
            .map(|(_, k)| k)
            .collect();
        let roles: Vec<&Key> = self.kb.keys.iter().filter(|k| k.role.is_some()).collect();
        if written.is_empty() && roles.is_empty() {
            return;
        }
        let keys = add(&mut self.doc, root, "keys", &[]);
        for key in written {
            let attrs = self.key_attrs(key);
            add(&mut self.doc, keys, "key", &attrs);
        }
        if !roles.is_empty() {
            let special = add(&mut self.doc, keys, "special", &[]);
            for key in roles {
                let role = key.role.map_or("", |r| r.name()).to_string();
                add(
                    &mut self.doc,
                    special,
                    &kb_name("role"),
                    &[("keyId", key.id.clone()), ("role", role)],
                );
            }
        }
    }

    fn flicks(&mut self, root: Element) {
        if self.kb.flicks.is_empty() {
            return;
        }
        let flicks = add(&mut self.doc, root, "flicks", &[]);
        for f in &self.kb.flicks {
            let flick = add(&mut self.doc, flicks, "flick", &[("id", f.id.clone())]);
            for s in &f.segments {
                let directions: Vec<&str> = s.directions.iter().map(|d| d.name()).collect();
                let attrs = [
                    ("directions", directions.join(" ")),
                    ("keyId", self.key_id(s.key)),
                ];
                add(&mut self.doc, flick, "flickSegment", &attrs);
            }
        }
    }

    fn rows(&mut self, parent: Element, name: &str, rows: &[Vec<u16>]) {
        for row in rows {
            let keys = self.key_ids(row);
            add(&mut self.doc, parent, name, &[("keys", keys)]);
        }
    }

    fn hardware(&mut self, root: Element) {
        let Some(hw) = &self.kb.hardware else {
            return;
        };
        if implied_form(self.kb.conforms_to, &hw.form.id).as_ref() != Some(&hw.form) {
            let forms = add(&mut self.doc, root, "forms", &[]);
            let form = add(&mut self.doc, forms, "form", &[("id", hw.form.id.clone())]);
            for row in &hw.form.rows {
                let codes: Vec<String> = row.iter().map(|c| format!("{c:02X}")).collect();
                add(
                    &mut self.doc,
                    form,
                    "scanCodes",
                    &[("codes", codes.join(" "))],
                );
            }
        }
        let mut attrs = vec![("formId", hw.form.id.clone())];
        if let Some(w) = hw.min_device_width {
            attrs.push(("minDeviceWidth", w.to_string()));
        }
        let layers = add(&mut self.doc, root, "layers", &attrs);
        let (native, plain): (Vec<_>, Vec<_>) = hw
            .layers
            .iter()
            .partition(|l| is_kbdgen_layer(&l.modifiers));
        for layer in plain {
            let mut attrs = Vec::new();
            if let Some(id) = &layer.id {
                attrs.push(("id", id.clone()));
            }
            attrs.push(("modifiers", encode_modifiers(&layer.modifiers)));
            let el = add(&mut self.doc, layers, "layer", &attrs);
            self.rows(el, "row", &layer.rows);
        }
        let extras = &self.kb.windows.extra_modifiers;
        if native.is_empty() && extras.is_empty() {
            return;
        }
        let special = add(&mut self.doc, layers, "special", &[]);
        for key in extras {
            add(
                &mut self.doc,
                special,
                &kb_name("extraModifier"),
                &[("key", key.name().to_string())],
            );
        }
        for layer in native {
            let mut attrs = Vec::new();
            if let Some(id) = &layer.id {
                attrs.push(("id", id.clone()));
            }
            attrs.push(("modifiers", encode_modifiers(&layer.modifiers)));
            let el = add(&mut self.doc, special, &kb_name("layer"), &attrs);
            self.rows(el, &kb_name("row"), &layer.rows);
        }
    }

    fn touch(&mut self, root: Element) {
        for set in &self.kb.touch {
            let mut attrs = vec![("formId", "touch".to_string())];
            if let Some(w) = set.min_device_width {
                attrs.push(("minDeviceWidth", w.to_string()));
            }
            let layers = add(&mut self.doc, root, "layers", &attrs);
            for layer in &set.layers {
                let el = add(&mut self.doc, layers, "layer", &[("id", layer.id.clone())]);
                self.rows(el, "row", &layer.rows);
            }
            if set.name.is_some() || set.bottom_row == BottomRow::Host {
                let special = add(&mut self.doc, layers, "special", &[]);
                let mut attrs = Vec::new();
                if let Some(name) = &set.name {
                    attrs.push(("name", encode_plain(name)));
                }
                let bottom = match set.bottom_row {
                    BottomRow::Host => "host",
                    BottomRow::Authored => "authored",
                };
                attrs.push(("bottomRow", bottom.to_string()));
                add(&mut self.doc, special, &kb_name("touchSet"), &attrs);
            }
        }
    }

    fn variables(&mut self, root: Element) {
        if self.kb.sets.is_empty() {
            return;
        }
        let vars = add(&mut self.doc, root, "variables", &[]);
        for (i, set) in self.kb.sets.iter().enumerate() {
            let items: Vec<Vec<Piece>> = set.iter().map(|t| self.pieces(t)).collect();
            let id = set_id(u16::try_from(i).unwrap_or(u16::MAX));
            add(
                &mut self.doc,
                vars,
                "set",
                &[("id", id), ("value", encode_set_items(&items))],
            );
        }
    }

    fn reorder(&mut self, group: Element, rule: &ReorderRule) {
        let elems = |classes: &[ReorderClass]| {
            let elems: Vec<ReorderElem> = classes
                .iter()
                .map(|c| match c {
                    ReorderClass::Char(c) => ReorderElem::Char(*c),
                    ReorderClass::Ranges(r) => ReorderElem::Ranges(r.clone()),
                })
                .collect();
            encode_reorder(&elems)
        };
        let list = |values: Vec<String>| -> Option<String> {
            match values.as_slice() {
                [] => None,
                [first, rest @ ..] if rest.iter().all(|v| v == first) => Some(first.clone()),
                _ => Some(values.join(" ")),
            }
        };
        let mut attrs = Vec::new();
        if !rule.before.is_empty() {
            attrs.push(("before", elems(&rule.before)));
        }
        attrs.push(("from", elems(&rule.from)));
        if rule.order.iter().any(|v| *v != 0) {
            attrs
                .extend(list(rule.order.iter().map(i8::to_string).collect()).map(|v| ("order", v)));
        }
        if rule.tertiary.iter().any(|v| *v != 0) {
            attrs.extend(
                list(rule.tertiary.iter().map(i8::to_string).collect()).map(|v| ("tertiary", v)),
            );
        }
        if rule.tertiary_base.iter().any(|v| *v) {
            attrs.extend(
                list(rule.tertiary_base.iter().map(bool::to_string).collect())
                    .map(|v| ("tertiaryBase", v)),
            );
        }
        if rule.pre_base.iter().any(|v| *v) {
            attrs.extend(
                list(rule.pre_base.iter().map(bool::to_string).collect()).map(|v| ("preBase", v)),
            );
        }
        add(&mut self.doc, group, "reorder", &attrs);
    }

    fn transforms(&mut self, root: Element, extensions: &Extensions) {
        for (list, groups, name) in [
            (TransformList::Simple, &self.kb.simple, "simple"),
            (TransformList::Backspace, &self.kb.backspace, "backspace"),
        ] {
            if groups.is_empty() {
                continue;
            }
            let transforms = add(
                &mut self.doc,
                root,
                "transforms",
                &[("type", name.to_string())],
            );
            for (index, group) in groups.iter().enumerate() {
                let el = add(&mut self.doc, transforms, "transformGroup", &[]);
                match group {
                    TransformGroup::Rules(rules) => {
                        for rule in rules {
                            let mut attrs = vec![("from", self.pattern(&rule.from))];
                            if !rule.to.is_empty() {
                                attrs.push(("to", self.replacement(&rule.to)));
                            }
                            add(&mut self.doc, el, "transform", &attrs);
                        }
                    }
                    TransformGroup::Reorder(rules) => {
                        for rule in rules {
                            self.reorder(el, rule);
                        }
                    }
                }
                let marks: Vec<&str> = extensions
                    .generated
                    .iter()
                    .filter(|g| g.list == list && g.group == index)
                    .map(|g| g.by.as_str())
                    .collect();
                if !marks.is_empty() {
                    let special = add(&mut self.doc, el, "special", &[]);
                    for by in marks {
                        add(
                            &mut self.doc,
                            special,
                            &kb_name("generated"),
                            &[("by", by.to_string())],
                        );
                    }
                }
            }
        }
    }

    fn compose(&mut self, parent: Element, compose: &Compose) {
        match &compose.value {
            ComposeValue::Output(output) => {
                add(
                    &mut self.doc,
                    parent,
                    &kb_name("compose"),
                    &[
                        ("input", encode_plain(&compose.input)),
                        ("output", encode_plain(output)),
                    ],
                );
            }
            ComposeValue::Node {
                marker,
                standalone,
                compose: children,
            } => {
                let el = add(
                    &mut self.doc,
                    parent,
                    &kb_name("compose"),
                    &[
                        ("input", encode_plain(&compose.input)),
                        ("marker", encode_plain(marker)),
                        ("standalone", encode_plain(standalone)),
                    ],
                );
                for child in children {
                    self.compose(el, child);
                }
            }
        }
    }

    fn dead_key(&mut self, special: Element, dk: &DeadKey) {
        let mut attrs = vec![
            ("identity", encode_plain(&dk.identity)),
            ("marker", encode_plain(&dk.marker)),
            ("display", encode_plain(&dk.display)),
            ("standalone", encode_plain(&dk.standalone)),
        ];
        if let Some(name) = &dk.name {
            attrs.push(("name", encode_plain(name)));
        }
        let el = add(&mut self.doc, special, &kb_name("deadKey"), &attrs);
        for c in &dk.compose {
            self.compose(el, c);
        }
    }

    fn keyboard_special(&mut self, root: Element, extensions: &Extensions) {
        if !has_keyboard_special(self.kb, extensions) {
            return;
        }
        let kb = self.kb;
        let special = add(&mut self.doc, root, "special", &[]);
        let layout = LayoutData {
            tag: extensions.tag.clone(),
            host: kb.host,
            decimal: kb.decimal.as_ref().map(|d| encode_text(&self.pieces(d))),
            space_label: kb.displays.labels.space.clone(),
            return_label: kb.displays.labels.r#return.clone(),
            implied_layers: extensions.implied_layers.clone(),
            display_names: extensions.display_names.clone(),
            targets: Vec::new(),
            emoji: Emoji::default(),
        };
        layout_elements(
            &mut self.doc,
            special,
            &layout,
            KBDGEN_PREFIX,
            LayoutPart::Head,
        );
        for (m, output) in &kb.flush {
            let attrs = [
                ("marker", self.marker(*m)),
                ("output", encode_plain(output)),
            ];
            add(&mut self.doc, special, &kb_name("flush"), &attrs);
        }
        for (m, name) in &kb.dead_key_names {
            let attrs = [("marker", self.marker(*m)), ("name", encode_plain(name))];
            add(&mut self.doc, special, &kb_name("deadKeyName"), &attrs);
        }
        let w = &kb.windows;
        if w.shift_lock || w.lrm_rlm {
            let mut attrs = Vec::new();
            if w.shift_lock {
                attrs.push(("shiftLock", "true".to_string()));
            }
            if w.lrm_rlm {
                attrs.push(("lrmRlm", "true".to_string()));
            }
            add(&mut self.doc, special, &kb_name("windows"), &attrs);
        }
        for (key, name) in &w.key_names {
            add(
                &mut self.doc,
                special,
                &kb_name("windowsKeyName"),
                &[("key", encode_plain(key)), ("name", encode_plain(name))],
            );
        }
        let tail = LayoutData {
            targets: extensions.targets.clone(),
            emoji: kb.emoji.clone(),
            ..LayoutData::default()
        };
        layout_elements(
            &mut self.doc,
            special,
            &tail,
            KBDGEN_PREFIX,
            LayoutPart::Tail,
        );
        for dk in &extensions.dead_keys {
            self.dead_key(special, dk);
        }
        if kb.hardware.is_none() {
            for key in &kb.windows.extra_modifiers {
                add(
                    &mut self.doc,
                    special,
                    &kb_name("extraModifier"),
                    &[("key", key.name().to_string())],
                );
            }
        }
    }
}

fn has_keyboard_special(kb: &Keyboard, ext: &Extensions) -> bool {
    ext.tag.is_some()
        || ext.implied_layers.is_some()
        || !ext.display_names.is_empty()
        || !ext.targets.is_empty()
        || !ext.dead_keys.is_empty()
        || kb.host.is_some()
        || kb.decimal.is_some()
        || kb.displays.labels.space.is_some()
        || kb.displays.labels.r#return.is_some()
        || !kb.flush.is_empty()
        || !kb.dead_key_names.is_empty()
        || kb.windows.shift_lock
        || kb.windows.lrm_rlm
        || !kb.windows.key_names.is_empty()
        || kb.emoji.key.is_some()
        || !kb.emoji.annotations.is_empty()
        || (kb.hardware.is_none() && !kb.windows.extra_modifiers.is_empty())
}

/// Whether the document needs kbdgen's namespace at all.
fn needs_kbdgen(kb: &Keyboard, ext: &Extensions) -> bool {
    has_keyboard_special(kb, ext)
        || !ext.generated.is_empty()
        || kb.keys.iter().any(|k| k.role.is_some())
        || kb
            .touch
            .iter()
            .any(|s| s.name.is_some() || s.bottom_row == BottomRow::Host)
        || kb.hardware.as_ref().is_some_and(|h| {
            !kb.windows.extra_modifiers.is_empty()
                || h.layers.iter().any(|l| is_kbdgen_layer(&l.modifiers))
        })
}

// [spec:kbdgen:req:ldml.xml.export+1]
// [spec:kbdgen:sem:ldml.xml.ldml-view+1]
// [spec:kbdgen:thm:ldml.xml.superset-roundtrip]
/// Builds the source document of `keyboard`: elements in the DTD's order,
/// attributes in the order the DTD declares them, implied keys and forms
/// left out unless overridden, modifier sets canonical, widths with at
/// most three decimals, and every superset field as one kbdgen element.
/// Sets are written as `set` variables named `s1`, `s2`, …; classes are
/// written inline. Resolving the result gives `keyboard` back.
///
/// An LDML-only consumer, which ignores `special`, sees every LDML layer,
/// key, flick, display, variable and transform group, the generated
/// dead-key groups included; role keys as gaps or layer switches; and no
/// native-only or `extra`n layers, decimal key, flush outputs or Windows
/// options.
pub fn export(keyboard: &Keyboard, extensions: &Extensions) -> Document {
    let version = conforms_to(keyboard);
    let mut doc = Document::new("keyboard3");
    doc.set_declaration(Some(Declaration::v1_0()));
    let root = doc.root();
    root.set_attribute(&mut doc, "locale", &keyboard.locale);
    root.set_attribute(&mut doc, "conformsTo", &version.to_string());
    root.set_attribute(&mut doc, "xmlns", &keyboard_namespace(version));
    if needs_kbdgen(keyboard, extensions) {
        root.set_attribute(
            &mut doc,
            format!("xmlns:{KBDGEN_PREFIX}").as_str(),
            KBDGEN_NS,
        );
    }
    let mut b = Builder { doc, kb: keyboard };
    b.header(root);
    b.displays(root);
    b.keys(root);
    b.flicks(root);
    b.hardware(root);
    b.touch(root);
    b.variables(root);
    b.transforms(root, extensions);
    b.keyboard_special(root, extensions);
    b.doc
}

/// The layout-level kbdgen data that a layout file overrides in a
/// referenced LDML document (`ldml.xml.ldml-ref`). `decimal` is written as
/// given, in `output` syntax.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LayoutData {
    pub tag: Option<String>,
    pub host: Option<Host>,
    pub decimal: Option<String>,
    pub space_label: Option<String>,
    pub return_label: Option<String>,
    pub implied_layers: Option<String>,
    pub display_names: BTreeMap<String, String>,
    pub targets: Vec<Target>,
    pub emoji: Emoji,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum LayoutPart {
    /// `kbdgen:keyboard` and `kbdgen:displayName`.
    Head,
    /// `kbdgen:target`, `kbdgen:emojiKey` and `kbdgen:emoji`.
    Tail,
}

fn layout_elements(
    doc: &mut Document,
    special: Element,
    data: &LayoutData,
    prefix: &str,
    part: LayoutPart,
) {
    let name = |local: &str| format!("{prefix}:{local}");
    if part == LayoutPart::Head {
        let mut attrs = Vec::new();
        if let Some(tag) = &data.tag {
            attrs.push(("tag", tag.clone()));
        }
        if let Some(host) = data.host {
            attrs.push(("host", host.name().to_string()));
        }
        if let Some(d) = &data.decimal {
            attrs.push(("decimal", d.clone()));
        }
        if let Some(l) = &data.space_label {
            attrs.push(("spaceLabel", encode_plain(l)));
        }
        if let Some(l) = &data.return_label {
            attrs.push(("returnLabel", encode_plain(l)));
        }
        if let Some(i) = &data.implied_layers {
            attrs.push(("impliedLayers", i.clone()));
        }
        if !attrs.is_empty() {
            add(doc, special, &name("keyboard"), &attrs);
        }
        for (lang, display) in &data.display_names {
            add(
                doc,
                special,
                &name("displayName"),
                &[("lang", lang.clone()), ("name", encode_plain(display))],
            );
        }
        return;
    }
    for t in &data.targets {
        add(
            doc,
            special,
            &name("target"),
            &[
                ("host", t.host.clone()),
                ("name", t.name.clone()),
                ("value", encode_plain(&t.value)),
            ],
        );
    }
    if let Some(key) = data.emoji.key {
        add(
            doc,
            special,
            &name("emojiKey"),
            &[
                ("scanCode", format!("{:02X}", key.scan_code)),
                (
                    "modifiers",
                    encode_modifiers(&[ModifierSet::Set(key.modifiers)]),
                ),
            ],
        );
    }
    for a in &data.emoji.annotations {
        add(
            doc,
            special,
            &name("emoji"),
            &[
                ("emoji", encode_plain(&a.emoji)),
                ("name", encode_plain(&a.name)),
                ("keywords", encode_plain(&a.keywords.join("|"))),
            ],
        );
    }
}

/// The kbdgen elements a layout file overrides in a referenced document.
const LAYOUT_ELEMENTS: [&str; 5] = ["keyboard", "displayName", "target", "emojiKey", "emoji"];

// [spec:kbdgen:req:ldml.xml.ldml-ref+1]
/// Puts a layout file's data into a referenced LDML document, which is
/// otherwise kept exactly as read (`ldml.xml.roundtrip`). Only kbdgen's
/// layout elements of `keyboard3`'s `special` are replaced: `keyboard`,
/// `displayName`, `target`, `emojiKey` and `emoji`. The new ones go into
/// the `special` that held kbdgen elements, else the first `special`, else
/// a `special` appended to `keyboard3`. An `impliedLayers` that the
/// document's `kbdgen:keyboard` had is kept. Other `special` content and
/// comments are untouched.
pub fn replace_extensions(document: &mut Document, data: &LayoutData) {
    let prefix = kbdgen_binding(document);
    let root = document.root();
    let specials: Vec<Element> = root
        .children(document)
        .into_iter()
        .filter(|c| c.name(document) == "special")
        .collect();
    let mut target: Option<Element> = None;
    let mut implied_layers = None;
    for special in &specials {
        for child in special.children(document) {
            let qname = child.qname(document);
            if qname.namespace() != Some(prefix.as_str()) {
                continue;
            }
            target.get_or_insert(*special);
            let local = qname.local_part().to_string();
            if !LAYOUT_ELEMENTS.contains(&local.as_str()) {
                continue;
            }
            if local == "keyboard" {
                implied_layers = child
                    .attribute(document, "impliedLayers")
                    .map(str::to_string);
            }
            special.remove_child(document, child.as_node());
        }
    }
    let special = match target.or_else(|| specials.first().copied()) {
        Some(s) => s,
        None => root.append_new_element(document, "special"),
    };
    let data = LayoutData {
        implied_layers: data.implied_layers.clone().or(implied_layers),
        ..data.clone()
    };
    layout_elements(document, special, &data, &prefix, LayoutPart::Head);
    layout_elements(document, special, &data, &prefix, LayoutPart::Tail);
}

/// The prefix bound to [`KBDGEN_NS`] on the root, binding `kbdgen` (or
/// `kbdgen1`, … when that is taken) if none is.
fn kbdgen_binding(document: &mut Document) -> String {
    let root = document.root();
    let bound = root
        .attributes(document)
        .iter()
        .find(|(k, v)| v.as_str() == KBDGEN_NS && k.prefixed_name().starts_with("xmlns:"))
        .map(|(k, _)| k.prefixed_name().trim_start_matches("xmlns:").to_string());
    if let Some(prefix) = bound {
        return prefix;
    }
    let taken = |doc: &Document, p: &str| {
        root.attributes(doc)
            .iter()
            .any(|(k, _)| k.prefixed_name() == format!("xmlns:{p}"))
    };
    let mut prefix = KBDGEN_PREFIX.to_string();
    let mut n = 1;
    while taken(document, &prefix) {
        prefix = format!("{KBDGEN_PREFIX}{n}");
        n += 1;
    }
    root.set_attribute(document, format!("xmlns:{prefix}").as_str(), KBDGEN_NS);
    prefix
}

/// Sets `kbdgen:keyboard@host`, adding the element, a `special` and the
/// namespace binding as needed and changing nothing else, so hosts that
/// share a model get files that differ only there (`ldml.cli.export`).
pub fn set_host(document: &mut Document, host: Host) {
    let prefix = kbdgen_binding(document);
    let root = document.root();
    let specials: Vec<Element> = root
        .children(document)
        .into_iter()
        .filter(|c| c.name(document) == "special")
        .collect();
    let existing = specials
        .iter()
        .flat_map(|s| s.children(document))
        .find(|c| {
            let q = c.qname(document);
            q.namespace() == Some(prefix.as_str()) && q.local_part() == "keyboard"
        });
    if let Some(keyboard) = existing {
        keyboard.set_attribute(document, "host", host.name());
        return;
    }
    let special = match specials.first() {
        Some(s) => *s,
        None => root.append_new_element(document, "special"),
    };
    add(
        document,
        special,
        &format!("{prefix}:keyboard"),
        &[("host", host.name().to_string())],
    );
}
