//! The source document of one host (`ldml.yaml.lowering` step 5): a
//! keyboard3 document in `ldml.xml.export` form, built with `xmlem`, with
//! kbdgen data per `ldml.xml.special`.

use kbd_ldml::escape::{Piece, encode_plain, encode_text};
use kbd_ldml::{
    KBDGEN_NS, KBDGEN_PREFIX, LayoutData, encode_modifiers, encode_width, is_kbdgen_layer, is_mark,
};
use kbd_model::{BottomRow, DEFAULT_WIDTH, Host, Normalization};
use xmlem::{Declaration, Document, Element};

use super::deadkeys::{DeadOut, Rule, metadata};
use super::keys::{KeyTable, Slot};
use super::layers::{HardwareOut, TouchOut};
use super::schema::{DisplayTarget4, Group, Layout4, VarKind};

/// Everything lowering decided for one document.
pub struct Parts<'a> {
    pub layout: &'a Layout4,
    pub conforms_to: u8,
    pub hardware: Option<HardwareOut>,
    pub touch: Vec<TouchOut>,
    pub table: KeyTable<'a>,
    pub dead: DeadOut,
}

fn add(doc: &mut Document, parent: Element, name: &str, attrs: &[(&str, String)]) -> Element {
    let el = parent.append_new_element(doc, name);
    for (k, v) in attrs {
        el.set_attribute(doc, *k, v);
    }
    el
}

fn kb(local: &str) -> String {
    format!("{KBDGEN_PREFIX}:{local}")
}

/// The autonym, the default `info@name`: the display name of the tag's
/// primary language.
fn autonym(layout: &Layout4) -> String {
    let primary = layout.tag.split('-').next().unwrap_or(&layout.tag);
    layout
        .display_names
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(primary))
        .map(|(_, v)| v.clone())
        .unwrap_or_default()
}

fn header(doc: &mut Document, root: Element, layout: &Layout4) {
    if !layout.locales.is_empty() {
        let locales = add(doc, root, "locales", &[]);
        for l in &layout.locales {
            add(doc, locales, "locale", &[("id", l.clone())]);
        }
    }
    if let Some(v) = &layout.version {
        add(doc, root, "version", &[("number", v.clone())]);
    }
    let info = &layout.info;
    let mut attrs = vec![("name", info.name.clone().unwrap_or_else(|| autonym(layout)))];
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
    add(doc, root, "info", &attrs);
    // [spec:kbdgen:sem:ldml.yaml.normalization]
    if layout.normalization == Normalization::Disabled {
        add(
            doc,
            root,
            "settings",
            &[("normalization", "disabled".to_string())],
        );
    }
}

/// The automatic displays (`ldml.yaml.displays.auto`), each unless the
/// layout's own displays cover its output or key: dead-key markers, keys
/// whose output is only marks, and the `space` label.
fn auto_displays(parts: &Parts) -> Vec<(&'static str, String, String)> {
    let layout = parts.layout;
    let covers_output = |pieces: &[Piece]| {
        layout
            .displays
            .iter()
            .any(|d| matches!(&d.target, DisplayTarget4::Output(_, p) if p == pieces))
    };
    let covers_key = |id: &str| {
        layout
            .displays
            .iter()
            .any(|d| matches!(&d.target, DisplayTarget4::KeyId(k) if k == id))
    };
    let mut out = Vec::new();
    for (marker, display, _) in &parts.dead.markers {
        let pieces = [Piece::Marker(marker.clone())];
        if !covers_output(&pieces) {
            out.push(("output", encode_text(&pieces), encode_plain(display)));
        }
    }
    let mut seen: Vec<&[Piece]> = Vec::new();
    for entry in &parts.table.entries {
        let marks = !entry.output.is_empty()
            && entry
                .output
                .iter()
                .all(|p| matches!(p, Piece::Char(c) if is_mark(*c)));
        if !marks
            || seen.contains(&entry.output.as_slice())
            || covers_output(&entry.output)
            || covers_key(&entry.id)
        {
            continue;
        }
        seen.push(&entry.output);
        let mut shown = String::from('\u{25CC}');
        shown.extend(entry.output.iter().filter_map(|p| match p {
            Piece::Char(c) => Some(*c),
            Piece::Marker(_) => None,
        }));
        out.push(("output", encode_text(&entry.output), encode_plain(&shown)));
    }
    if let Some(label) = &layout.space_label
        && !covers_key("space")
    {
        out.push(("keyId", "space".to_string(), encode_plain(label)));
    }
    out
}

// [spec:kbdgen:sem:ldml.yaml.displays.auto]
fn displays(doc: &mut Document, root: Element, parts: &Parts) {
    let layout = parts.layout;
    let auto = auto_displays(parts);
    if layout.displays.is_empty() && auto.is_empty() && layout.display_base.is_none() {
        return;
    }
    let el = add(doc, root, "displays", &[]);
    for d in &layout.displays {
        let target = match &d.target {
            DisplayTarget4::Output(raw, _) => ("output", raw.clone()),
            DisplayTarget4::KeyId(id) => ("keyId", id.clone()),
        };
        add(
            doc,
            el,
            "display",
            &[target, ("display", d.display.clone())],
        );
    }
    for (kind, target, display) in auto {
        add(doc, el, "display", &[(kind, target), ("display", display)]);
    }
    if let Some(base) = &layout.display_base {
        add(
            doc,
            el,
            "displayOptions",
            &[("baseCharacter", base.clone())],
        );
    }
}

fn keys(doc: &mut Document, root: Element, parts: &Parts) {
    let layout = parts.layout;
    let made: Vec<_> = parts
        .table
        .entries
        .iter()
        .filter(|e| matches!(e.slot, Slot::Made { .. }))
        .collect();
    if layout.keys.is_empty() && made.is_empty() {
        return;
    }
    let el = add(doc, root, "keys", &[]);
    let mut roles = Vec::new();
    for k in &layout.keys {
        let mut attrs = vec![("id", k.id.clone())];
        let optional = [
            ("flickId", k.flick.clone()),
            ("gap", k.gap.then(|| "true".to_string())),
            ("output", k.output.clone()),
            ("longPressKeyIds", k.long_press.clone()),
            ("longPressDefaultKeyId", k.long_press_default.clone()),
            ("multiTapKeyIds", k.multi_tap.clone()),
            ("stretch", k.stretch.then(|| "true".to_string())),
            ("layerId", k.layer.clone()),
            ("width", k.width.clone()),
        ];
        attrs.extend(optional.into_iter().filter_map(|(n, v)| v.map(|v| (n, v))));
        add(doc, el, "key", &attrs);
        if let Some(role) = k.role {
            roles.push((k.id.clone(), role));
        }
    }
    for entry in made {
        let Slot::Made {
            def,
            long_press,
            flicks,
        } = &entry.slot
        else {
            continue;
        };
        let mut attrs = vec![("id", entry.id.clone())];
        if !flicks.is_empty() {
            attrs.push(("flickId", format!("flick-{}", entry.id)));
        }
        if def.gap {
            attrs.push(("gap", "true".to_string()));
        }
        if !def.output.is_empty() {
            attrs.push(("output", encode_text(&def.output)));
        }
        if !long_press.is_empty() {
            attrs.push(("longPressKeyIds", long_press.join(" ")));
        }
        if def.stretch {
            attrs.push(("stretch", "true".to_string()));
        }
        if let Some(layer) = &def.layer {
            attrs.push(("layerId", layer.clone()));
        }
        if def.width != DEFAULT_WIDTH {
            attrs.push(("width", encode_width(def.width)));
        }
        add(doc, el, "key", &attrs);
        if let Some(role) = def.role {
            roles.push((entry.id.clone(), role));
        }
    }
    if !roles.is_empty() {
        let special = add(doc, el, "special", &[]);
        for (id, role) in roles {
            add(
                doc,
                special,
                &kb("role"),
                &[("keyId", id), ("role", role.name().to_string())],
            );
        }
    }
}

/// A made key's flick segments: directions and target key id.
type FlickIds = Vec<(Vec<kbd_model::Direction>, String)>;

fn flicks(doc: &mut Document, root: Element, parts: &Parts) {
    let made: Vec<(&str, &FlickIds)> = parts
        .table
        .entries
        .iter()
        .filter_map(|e| match &e.slot {
            Slot::Made { flicks, .. } if !flicks.is_empty() => Some((e.id.as_str(), flicks)),
            _ => None,
        })
        .collect();
    if parts.layout.flicks.is_empty() && made.is_empty() {
        return;
    }
    let el = add(doc, root, "flicks", &[]);
    for f in &parts.layout.flicks {
        let flick = add(doc, el, "flick", &[("id", f.id.clone())]);
        for (directions, key) in &f.segments {
            add(
                doc,
                flick,
                "flickSegment",
                &[("directions", directions.clone()), ("keyId", key.clone())],
            );
        }
    }
    for (id, segments) in made {
        let flick = add(doc, el, "flick", &[("id", format!("flick-{id}"))]);
        for (directions, key) in segments {
            let names: Vec<&str> = directions.iter().map(|d| d.name()).collect();
            add(
                doc,
                flick,
                "flickSegment",
                &[("directions", names.join(" ")), ("keyId", key.clone())],
            );
        }
    }
}

fn rows(doc: &mut Document, layer: Element, name: &str, rows: &[Vec<String>]) {
    for row in rows {
        add(doc, layer, name, &[("keys", row.join(" "))]);
    }
}

// [spec:kbdgen:def:ldml.yaml.native]
/// The hardware layers; native-only layers and `extraModifiers` go in
/// kbdgen's namespace.
fn hardware(doc: &mut Document, root: Element, hw: &HardwareOut) {
    if hw.custom {
        let forms = add(doc, root, "forms", &[]);
        let form = add(doc, forms, "form", &[("id", hw.form.id.clone())]);
        for row in &hw.form.rows {
            let codes: Vec<String> = row.iter().map(|c| format!("{c:02X}")).collect();
            add(doc, form, "scanCodes", &[("codes", codes.join(" "))]);
        }
    }
    let layers = add(doc, root, "layers", &[("formId", hw.form.id.clone())]);
    let (native, plain): (Vec<_>, Vec<_>) =
        hw.layers.iter().partition(|l| is_kbdgen_layer(&l.sets));
    for layer in plain {
        let el = add(
            doc,
            layers,
            "layer",
            &[("modifiers", encode_modifiers(&layer.sets))],
        );
        rows(doc, el, "row", &layer.rows);
    }
    if native.is_empty() && hw.extra_modifiers.is_empty() {
        return;
    }
    let special = add(doc, layers, "special", &[]);
    for key in &hw.extra_modifiers {
        add(
            doc,
            special,
            &kb("extraModifier"),
            &[("key", key.name().to_string())],
        );
    }
    for layer in native {
        let el = add(
            doc,
            special,
            &kb("layer"),
            &[("modifiers", encode_modifiers(&layer.sets))],
        );
        rows(doc, el, &kb("row"), &layer.rows);
    }
}

fn touch(doc: &mut Document, root: Element, sets: &[TouchOut]) {
    for set in sets {
        let mut attrs = vec![("formId", "touch".to_string())];
        if let Some(w) = set.min_device_width {
            attrs.push(("minDeviceWidth", w.to_string()));
        }
        let layers = add(doc, root, "layers", &attrs);
        for (id, layer_rows) in &set.layers {
            let el = add(doc, layers, "layer", &[("id", id.clone())]);
            rows(doc, el, "row", layer_rows);
        }
        let special = add(doc, layers, "special", &[]);
        let bottom = match set.bottom_row {
            BottomRow::Host => "host",
            BottomRow::Authored => "authored",
        };
        add(
            doc,
            special,
            &kb("touchSet"),
            &[
                ("name", encode_plain(&set.name)),
                ("bottomRow", bottom.to_string()),
            ],
        );
    }
}

fn variables(doc: &mut Document, root: Element, layout: &Layout4) {
    if layout.variables.is_empty() {
        return;
    }
    let el = add(doc, root, "variables", &[]);
    for v in &layout.variables {
        let name = match v.kind {
            VarKind::String => "string",
            VarKind::Set => "set",
            VarKind::Uset => "uset",
        };
        add(
            doc,
            el,
            name,
            &[("id", v.id.clone()), ("value", v.value.clone())],
        );
    }
}

fn rules(doc: &mut Document, group: Element, rules: &[Rule]) {
    for (from, to) in rules {
        let mut attrs = vec![("from", from.clone())];
        if let Some(to) = to {
            attrs.push(("to", to.clone()));
        }
        add(doc, group, "transform", &attrs);
    }
}

fn user_group(doc: &mut Document, transforms: Element, group: &Group) {
    let el = add(doc, transforms, "transformGroup", &[]);
    match group {
        Group::Rules(list) => rules(doc, el, list),
        Group::Reorder(list) => {
            for r in list {
                let mut attrs = Vec::new();
                if let Some(b) = &r.before {
                    attrs.push(("before", b.clone()));
                }
                attrs.push(("from", r.from.clone()));
                for (name, value) in [
                    ("order", &r.order),
                    ("tertiary", &r.tertiary),
                    ("tertiaryBase", &r.tertiary_base),
                    ("preBase", &r.pre_base),
                ] {
                    if let Some(v) = value {
                        attrs.push((name, v.clone()));
                    }
                }
                add(doc, el, "reorder", &attrs);
            }
        }
    }
}

fn generated_group(doc: &mut Document, transforms: Element, list: &[Rule], by: &str) {
    let el = add(doc, transforms, "transformGroup", &[]);
    rules(doc, el, list);
    let special = add(doc, el, "special", &[]);
    add(doc, special, &kb("generated"), &[("by", by.to_string())]);
}

// [spec:kbdgen:sem:ldml.yaml.dead-keys.fallback]
// [spec:kbdgen:sem:ldml.yaml.dead-keys.backspace]
/// The `simple` groups are the compose group, the fallback group, then the
/// layout's own; the `backspace` groups are the layout's own, then the
/// generated one. Generated groups carry `kbdgen:generated`.
fn transforms(doc: &mut Document, root: Element, parts: &Parts) {
    let layout = parts.layout;
    let dead = &parts.dead;
    let generated = !dead.compose.is_empty();
    if generated || !layout.transforms.is_empty() {
        let el = add(doc, root, "transforms", &[("type", "simple".to_string())]);
        if generated {
            generated_group(doc, el, &dead.compose, "deadKeys-compose");
            generated_group(doc, el, &dead.fallback, "deadKeys-fallback");
        }
        for group in &layout.transforms {
            user_group(doc, el, group);
        }
    }
    if generated || !layout.backspace.is_empty() {
        let el = add(
            doc,
            root,
            "transforms",
            &[("type", "backspace".to_string())],
        );
        for group in &layout.backspace {
            user_group(doc, el, group);
        }
        if generated {
            generated_group(doc, el, &dead.backspace, "deadKeys-backspace");
        }
    }
}

// [spec:kbdgen:def:ldml.yaml.targets]
fn keyboard_special(doc: &mut Document, root: Element, parts: &Parts) {
    let layout = parts.layout;
    let special = add(doc, root, "special", &[]);
    for (marker, _, standalone) in &parts.dead.markers {
        add(
            doc,
            special,
            &kb("flush"),
            &[
                ("marker", marker.clone()),
                ("output", encode_plain(standalone)),
            ],
        );
    }
    for (marker, name) in &parts.dead.names {
        add(
            doc,
            special,
            &kb("deadKeyName"),
            &[("marker", marker.clone()), ("name", encode_plain(name))],
        );
    }
    let t = &layout.targets;
    if t.shift_lock || t.lrm_rlm {
        let mut attrs = Vec::new();
        if t.shift_lock {
            attrs.push(("shiftLock", "true".to_string()));
        }
        if t.lrm_rlm {
            attrs.push(("lrmRlm", "true".to_string()));
        }
        add(doc, special, &kb("windows"), &attrs);
    }
    for (key, name) in &t.key_names {
        add(
            doc,
            special,
            &kb("windowsKeyName"),
            &[("key", encode_plain(key)), ("name", encode_plain(name))],
        );
    }
    for dk in metadata(&layout.dead_keys) {
        let mut attrs = vec![
            ("identity", encode_plain(&dk.identity)),
            ("marker", dk.marker.clone()),
            ("display", encode_plain(&dk.display)),
            ("standalone", encode_plain(&dk.standalone)),
        ];
        if let Some(name) = &dk.name {
            attrs.push(("name", encode_plain(name)));
        }
        let el = add(doc, special, &kb("deadKey"), &attrs);
        compose_elements(doc, el, &dk.compose);
    }
    kbd_ldml::replace_extensions(doc, &layout_data(parts));
}

fn compose_elements(doc: &mut Document, parent: Element, compose: &[kbd_ldml::Compose]) {
    for c in compose {
        match &c.value {
            kbd_ldml::ComposeValue::Output(output) => {
                add(
                    doc,
                    parent,
                    &kb("compose"),
                    &[
                        ("input", encode_plain(&c.input)),
                        ("output", encode_plain(output)),
                    ],
                );
            }
            kbd_ldml::ComposeValue::Node {
                marker,
                standalone,
                compose,
            } => {
                let el = add(
                    doc,
                    parent,
                    &kb("compose"),
                    &[
                        ("input", encode_plain(&c.input)),
                        ("marker", marker.clone()),
                        ("standalone", encode_plain(standalone)),
                    ],
                );
                compose_elements(doc, el, compose);
            }
        }
    }
}

// [spec:kbdgen:def:ldml.yaml.targets]
/// The layout-level kbdgen data every document of the layout shares.
pub fn layout_data(parts: &Parts) -> LayoutData {
    let layout = parts.layout;
    LayoutData {
        tag: Some(layout.tag.clone()),
        host: None,
        decimal: layout.decimal.as_ref().map(|d| encode_text(d)),
        space_label: layout.space_label.clone(),
        return_label: layout.return_label.clone(),
        implied_layers: parts
            .hardware
            .as_ref()
            .map(|h| if h.implied { "macOS" } else { "none" }.to_string()),
        display_names: layout.display_names.clone(),
        targets: layout.targets.entries.clone(),
        emoji: layout.emoji.clone(),
    }
}

/// `conformsTo`: 45, or 47 when `info@attribution` is present
/// (`ldml.xml.export`).
pub fn conforms_to(layout: &Layout4) -> u8 {
    if layout.info.attribution.is_some() {
        47
    } else {
        45
    }
}

// [spec:kbdgen:sem:ldml.yaml.lowering]
/// Builds the source document: elements in the DTD's order, attributes in
/// the order it declares them.
pub fn build(parts: &Parts, host: Host) -> Document {
    let mut doc = Document::new("keyboard3");
    doc.set_declaration(Some(Declaration::v1_0()));
    let root = doc.root();
    root.set_attribute(&mut doc, "locale", &parts.layout.tag);
    root.set_attribute(&mut doc, "conformsTo", &parts.conforms_to.to_string());
    root.set_attribute(
        &mut doc,
        "xmlns",
        &kbd_ldml::read::keyboard_namespace(parts.conforms_to),
    );
    root.set_attribute(
        &mut doc,
        format!("xmlns:{KBDGEN_PREFIX}").as_str(),
        KBDGEN_NS,
    );
    header(&mut doc, root, parts.layout);
    displays(&mut doc, root, parts);
    keys(&mut doc, root, parts);
    flicks(&mut doc, root, parts);
    if let Some(hw) = &parts.hardware {
        hardware(&mut doc, root, hw);
    }
    touch(&mut doc, root, &parts.touch);
    variables(&mut doc, root, parts.layout);
    transforms(&mut doc, root, parts);
    keyboard_special(&mut doc, root, parts);
    kbd_ldml::set_host(&mut doc, host);
    doc
}
