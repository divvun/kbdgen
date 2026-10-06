//! Resolution: imports, implied data, validation, variables, markers,
//! normalization, layers and transforms.

use kbd_model::{
    Atom, BottomRow, Component, DisplayTarget, ModifierSet, Modifiers, Normalization,
    ReplacementItem, Text, TextElem, TransformGroup,
};

use super::*;

const DISABLED: &str = r#"<settings normalization="disabled"/>"#;

/// The error of resolving `body` with a hardware layer set; every error
/// names the file and an element path.
fn error_of(body: &str) -> String {
    let err = resolve_error(&with_layers(body));
    assert!(err.starts_with("test.xml: keyboard3"), "{err}");
    err
}

const LAYERS: &str =
    r#"<layers formId="iso"><layer modifiers="none"><row keys="a"/></layer></layers>"#;

/// `body` with a hardware layer set at its place in the DTD order.
fn with_layers(body: &str) -> String {
    let at = ["<variables", "<transforms", "<special"]
        .iter()
        .filter_map(|tag| body.find(tag))
        .min()
        .unwrap_or(body.len());
    keyboard("", &format!("{}{}\n{}", &body[..at], LAYERS, &body[at..]))
}

fn simple_rules(r: &Resolved) -> &[kbd_model::Rule] {
    match &r.keyboard.simple[0] {
        TransformGroup::Rules(rules) => rules,
        TransformGroup::Reorder(_) => panic!("expected rules"),
    }
}

// [spec:kbdgen:sem:ldml.xml.implied+1/test]
#[test]
fn implied_keys_come_first_and_override_in_place() {
    let r = resolved(&with_layers(
        r#"<keys><key id="xx" output="x"/><key id="a" output="á"/></keys>"#,
    ));
    let ids: Vec<&str> = r.keyboard.keys.iter().map(|k| k.id.as_str()).collect();
    assert_eq!(&ids[..4], ["gap", "space", "0", "1"]);
    assert_eq!(ids.len(), 65);
    assert_eq!(ids.last(), Some(&"xx"));
    let a = r.keyboard.key_index("a").unwrap();
    assert_eq!(a, 38, "a keeps its implied position");
    assert_eq!(r.keyboard.key(a).unwrap().output, Text::from("a\u{301}"));
    assert!(r.keyboard.key(0).unwrap().gap);
    assert_eq!(r.keyboard.key(1).unwrap().output, Text::from(" "));
}

// [spec:kbdgen:sem:ldml.xml.implied+1/test]
#[test]
fn implied_and_custom_forms() {
    let iso = resolved(&with_layers(""));
    let form = &iso.keyboard.hardware.as_ref().unwrap().form;
    assert_eq!(form.id, "iso");
    assert_eq!(
        form.rows.iter().map(Vec::len).collect::<Vec<_>>(),
        [13, 12, 12, 11, 1]
    );
    assert_eq!(form.rows[3][0], 0x56);
    let custom = resolved(&keyboard(
        "",
        r#"<forms><form id="iso"><scanCodes codes="1E 30"/></form></forms>
           <layers formId="iso"><layer modifiers="none"><row keys="a b"/></layer></layers>"#,
    ));
    assert_eq!(
        custom.keyboard.hardware.unwrap().form.rows,
        vec![vec![0x1E, 0x30]]
    );
    let unknown = resolve_error(&keyboard(
        "",
        r#"<layers formId="dvorak"><layer modifiers="none"/></layers>"#,
    ));
    assert!(unknown.contains("no form has id dvorak"), "{unknown}");
}

// [spec:kbdgen:sem:ldml.xml.import/test]
#[test]
fn local_and_cldr_imports_splice_in_place() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("sub")).unwrap();
    std::fs::write(
        dir.path().join("sub/extra.xml"),
        r#"<keys><import path="more.xml"/><key id="k1" output="from import"/><key id="k2" output="y"/></keys>"#,
    )
    .unwrap();
    std::fs::write(
        dir.path().join("sub/more.xml"),
        r#"<keys><key id="k3" output="z"/></keys>"#,
    )
    .unwrap();
    let xml = keyboard(
        "",
        r#"<keys><import base="cldr" path="46/keys-Zyyy-currency.xml"/><import path="sub/extra.xml"/><key id="k1" output="local"/></keys>
           <layers formId="iso"><layer modifiers="none"><row keys="k1 k2 k3 dollar"/></layer></layers>"#,
    );
    let path = dir.path().join("k.xml");
    std::fs::write(&path, &xml).unwrap();
    let r = resolve(&crate::read_keyboard_file(&path).unwrap()).unwrap();
    let ids: Vec<&str> = r.keyboard.keys[64..]
        .iter()
        .map(|k| k.id.as_str())
        .collect();
    assert_eq!(
        ids,
        [
            "dollar", "euro", "pound", "yen", "cruzeiro", "cent", "k3", "k1", "k2"
        ]
    );
    let x = r.keyboard.key(r.keyboard.key_index("k1").unwrap()).unwrap();
    assert_eq!(
        x.output,
        Text::from("local"),
        "the later element overrides in place"
    );
}

// [spec:kbdgen:sem:ldml.xml.import/test]
#[test]
fn import_errors_name_the_problem() {
    let twice = error_of(
        r#"<keys><import base="cldr" path="45/keys-Zyyy-currency.xml"/><import base="cldr" path="45/keys-Zyyy-currency.xml"/></keys>"#,
    );
    assert!(twice.contains("imported twice"), "{twice}");
    let missing =
        error_of(r#"<keys><import base="cldr" path="44/keys-Zyyy-currency.xml"/></keys>"#);
    assert!(missing.contains("not an embedded CLDR import"), "{missing}");
    let wrong_root =
        error_of(r#"<displays><import base="cldr" path="45/keys-Zyyy-currency.xml"/></displays>"#);
    assert!(
        wrong_root.contains("does not match the importing displays"),
        "{wrong_root}"
    );
    let local = error_of(r#"<keys><import path="x.xml"/></keys>"#);
    assert!(
        local.contains("needs a document read from a file"),
        "{local}"
    );
    assert!(
        local.starts_with("test.xml: keyboard3/keys/import[path=x.xml]@path"),
        "{local}"
    );
}

// [spec:kbdgen:req:ldml.xml.validate+1/test]
#[test]
fn validation_errors_carry_element_paths() {
    let cases = [
        ("<bogus/>", "keyboard3/bogus: unknown element bogus"),
        (
            r#"<keys><key id="q" outptu="q"/></keys>"#,
            "keyboard3/keys/key[id=q]@outptu",
        ),
        (
            r#"<keys><key id="q" gap="true" output="q"/></keys>"#,
            "a gap key may not have",
        ),
        (
            r#"<keys><key id="q"/></keys>"#,
            "needs output, layerId or gap",
        ),
        (
            r#"<keys><key id="q" output="q" width="0"/></keys>"#,
            "not a valid width",
        ),
        (
            r#"<keys><key id="q" output="q" width="1.2345"/></keys>"#,
            "not a valid width",
        ),
        (r#"<keys/><displays/>"#, "displays is not allowed here"),
        (
            r#"<displays><display output="a" keyId="a" display="b"/></displays>"#,
            "exactly one of output and keyId",
        ),
        (
            r#"<displays><display output="a" display="a"/></displays>"#,
            "extraneous",
        ),
        (
            r#"<variables><string id="v" value="1"/><set id="v" value="2"/></variables>"#,
            "used twice",
        ),
        (
            r#"<variables><string id="bad-id" value="1"/></variables>"#,
            "not a valid id",
        ),
        (
            r#"<transforms type="simple"><transformGroup/></transforms>"#,
            "may not be empty",
        ),
        (
            r#"<transforms type="simple"><transformGroup><transform from="a"/><reorder from="b"/></transformGroup></transforms>"#,
            "not both",
        ),
        (
            r#"<settings normalization="enabled"/>"#,
            "not a valid normalization",
        ),
        (
            r#"<special><foo:bar xmlns:foo="urn:x"/></special><foo:baz xmlns:foo="urn:x"/>"#,
            "outside special",
        ),
    ];
    for (body, needle) in cases {
        let err = error_of(body);
        assert!(err.contains(needle), "{body}: {err}");
        assert!(err.starts_with("test.xml: keyboard3"), "{err}");
    }
    assert!(resolve_error(&keyboard("", "")).contains("at least one layers"));
    let locale = resolve_error(&with_layers("").replace(
        "<info",
        r#"<locales><locale id="not a tag"/></locales><info"#,
    ));
    assert!(
        locale.contains("keyboard3/locales/locale[id=not a tag]@id"),
        "{locale}"
    );
    let info = resolve_error(&keyboard("", "").replace("<info name=\"Test\"/>", ""));
    assert!(info.contains("exactly one info"), "{info}");
}

// [spec:kbdgen:req:ldml.xml.validate+1/test]
#[test]
fn layer_constraints() {
    let two = resolve_error(&keyboard(
        "",
        r#"<layers formId="iso"><layer modifiers="none"><row keys="a"/></layer></layers>
           <layers formId="us"><layer modifiers="none"><row keys="a"/></layer></layers>"#,
    ));
    assert!(two.contains("at most one hardware"), "{two}");
    let no_base = resolve_error(&keyboard(
        "",
        r#"<layers formId="touch"><layer id="x"><row keys="a"/></layer></layers>"#,
    ));
    assert!(no_base.contains("id base"), "{no_base}");
    let no_mods = resolve_error(&keyboard(
        "",
        r#"<layers formId="iso"><layer id="x"><row keys="a"/></layer></layers>"#,
    ));
    assert!(no_mods.contains("needs modifiers"), "{no_mods}");
    let widths = resolve_error(&keyboard(
        "",
        r#"<layers formId="touch"><layer id="base"><row keys="a"/></layer></layers>
           <layers formId="touch"><layer id="base"><row keys="a"/></layer></layers>"#,
    ));
    assert!(widths.contains("distinct minDeviceWidth"), "{widths}");
    let overlap = resolve_error(&keyboard(
        "",
        r#"<layers formId="iso"><layer modifiers="alt shift"><row keys="a"/></layer><layer modifiers="altR shift"><row keys="b"/></layer></layers>"#,
    ));
    assert!(overlap.contains("overlap"), "{overlap}");
    let long = resolve_error(&keyboard(
        "",
        r#"<layers formId="iso"><layer modifiers="none"><row keys="a b c d e f g h i j k l m n"/></layer></layers>"#,
    ));
    assert!(
        long.contains("row 1 has 14 keys; form iso row 1 has 13"),
        "{long}"
    );
}

// [spec:kbdgen:sem:ldml.xml.resolve+1/test]
#[test]
fn modifier_sets_are_canonical() {
    let r = resolved(&keyboard(
        "",
        r#"<layers formId="iso">
             <layer modifiers="shift caps, none"><row keys="a"/></layer>
             <layer modifiers="altR  shift"><row keys="b"/></layer>
             <layer modifiers="other"><row keys="c"/></layer>
           </layers>"#,
    ));
    let hw = r.keyboard.hardware.unwrap();
    assert_eq!(
        hw.layers[0].modifiers,
        [
            ModifierSet::Set(Modifiers::NONE),
            ModifierSet::Set(Modifiers::of(&[Component::Caps, Component::Shift]))
        ]
    );
    assert_eq!(
        hw.layers[1].modifiers,
        [ModifierSet::Set(Modifiers::of(&[
            Component::AltR,
            Component::Shift
        ]))]
    );
    assert_eq!(hw.layers[2].modifiers, [ModifierSet::Other]);
    for bad in [
        "none shift",
        "alt altR",
        "shift shift",
        "hyper",
        "none, none",
    ] {
        let err = resolve_error(&keyboard(
            "",
            &format!(
                r#"<layers formId="iso"><layer modifiers="{bad}"><row keys="a"/></layer></layers>"#
            ),
        ));
        assert!(err.contains("@modifiers"), "{bad}: {err}");
    }
}

// [spec:kbdgen:sem:ldml.xml.resolve+1/test]
#[test]
fn touch_sets_sort_by_width_with_base() {
    let r = resolved(&keyboard(
        "",
        r#"<keys><key id="sw" layerId="shift"/></keys>
           <layers formId="touch" minDeviceWidth="200"><layer id="shift"><row keys="A"/></layer><layer id="base"><row keys="a sw"/></layer></layers>
           <layers formId="touch"><layer id="base"><row keys="a"/></layer><layer id="shift"><row keys="A"/></layer></layers>"#,
    ));
    let touch = &r.keyboard.touch;
    assert_eq!(
        touch.iter().map(|s| s.min_device_width).collect::<Vec<_>>(),
        [None, Some(200)]
    );
    assert_eq!((touch[0].base, touch[1].base), (0, 1));
    assert_eq!(
        touch[0].bottom_row,
        BottomRow::Authored,
        "foreign touch sets draw their own rows"
    );
    assert!(r.keyboard.hardware.is_none());
}

// [spec:kbdgen:sem:ldml.xml.resolve+1/test]
#[test]
fn key_references_resolve_forwards() {
    let r = resolved(&with_layers(
        r#"<keys>
             <key id="o" output="o" longPressKeyIds="o2 o3" longPressDefaultKeyId="o3" multiTapKeyIds="o2" flickId="f" width="1.25" stretch="true"/>
             <key id="o2" output="ó"/><key id="o3" output="ö"/>
           </keys>
           <flicks><flick id="f"><flickSegment directions="ne s" keyId="o2"/></flick></flicks>"#,
    ));
    let kb = &r.keyboard;
    let o = kb.key(kb.key_index("o").unwrap()).unwrap();
    let (o2, o3) = (kb.key_index("o2").unwrap(), kb.key_index("o3").unwrap());
    assert_eq!(o.long_press, [o2, o3]);
    assert_eq!(o.long_press_default, Some(o3));
    assert_eq!(o.multi_tap, [o2]);
    assert_eq!((o.width, o.stretch, o.flick), (1250, true, Some(0)));
    assert_eq!(kb.flicks[0].segments[0].key, o2);
    let bad_default = error_of(
        r#"<keys><key id="o" output="o" longPressKeyIds="a" longPressDefaultKeyId="b"/></keys>"#,
    );
    assert!(
        bad_default.contains("not in longPressKeyIds"),
        "{bad_default}"
    );
    let unknown = error_of(r#"<keys><key id="o" output="o" longPressKeyIds="nope"/></keys>"#);
    assert!(unknown.contains("no key has id nope"), "{unknown}");
    let flick = error_of(r#"<keys><key id="o" output="o" flickId="nope"/></keys>"#);
    assert!(flick.contains("no flick has id nope"), "{flick}");
}

// [spec:kbdgen:syn:ldml.xml.escape+1/test]
#[test]
fn variables_substitute_into_outputs_displays_and_to() {
    let r = resolved(&with_layers(&format!(
        r#"{DISABLED}
           <displays><display output="${{g}}" display="${{shown}}"/></displays>
           <keys><key id="g" output="${{g}}x"/></keys>
           <variables><string id="g" value="\m{{grave}}"/><string id="shown" value="\u{{60}}"/></variables>
           <transforms type="simple"><transformGroup><transform from="${{g}}a" to="à${{g}}"/></transformGroup></transforms>"#
    )));
    let kb = &r.keyboard;
    assert_eq!(kb.markers, ["grave"]);
    let g = kb.key(kb.key_index("g").unwrap()).unwrap();
    assert_eq!(
        g.output,
        Text(vec![TextElem::Marker(0), TextElem::Char('x')])
    );
    assert_eq!(kb.displays.entries[0].display, "`");
    assert_eq!(
        kb.displays.entries[0].target,
        DisplayTarget::Output(Text(vec![TextElem::Marker(0)]))
    );
    let rule = &simple_rules(&r)[0];
    assert_eq!(
        rule.to,
        [ReplacementItem::Text(Text(vec![
            TextElem::Char('à'),
            TextElem::Marker(0)
        ]))]
    );
    assert!(error_of(r#"<keys><key id="q" output="${nope}"/></keys>"#).contains("not defined"));
}

// [spec:kbdgen:def:ldml.model.text+1/test]
#[test]
fn markers_intern_in_document_order() {
    let r = resolved(&with_layers(
        r#"<displays><display output="\m{d}" display="D"/></displays>
           <keys><key id="k" output="\m{k}"/></keys>
           <variables><string id="never" value="\m{never}"/><set id="s" value="\m{unused} \m{s}"/></variables>
           <transforms type="simple"><transformGroup>
             <transform from="\m{t}$[s]\m{.}" to="\m{u}"/>
           </transformGroup></transforms>"#,
    ));
    assert_eq!(r.keyboard.markers, ["d", "k", "t", "unused", "s", "u"]);
}

// [spec:kbdgen:sem:ldml.xml.resolve+1/test]
#[test]
fn mapped_sets_and_groups_resolve() {
    let r = resolved(&with_layers(
        r#"<variables><set id="up" value="A B"/><set id="low" value="a b"/><uset id="v" value="[aeiou]"/></variables>
           <transforms type="simple"><transformGroup>
             <transform from="x($[up])$[v]" to="$[1:low]$1$0"/>
           </transformGroup></transforms>"#,
    ));
    let rule = &simple_rules(&r)[0];
    assert_eq!(
        rule.to,
        [
            ReplacementItem::MapSet {
                group: 1,
                from: 0,
                to: 1
            },
            ReplacementItem::Group(1),
            ReplacementItem::Group(0)
        ]
    );
    assert_eq!(r.keyboard.classes.len(), 1);
    let errors = [
        (
            r#"<transform from="(a)" to="$2"/>"#,
            "names a group that from lacks",
        ),
        (
            r#"<transform from="(a$[up])" to="$[1:low]"/>"#,
            "exactly one set",
        ),
        (r#"<transform from="($[up])" to="$[1:v]"/>"#, "not a set"),
        (
            r#"<transform from="($[up])" to="$[1:three]"/>"#,
            "has 2 items but three has 3",
        ),
        (r#"<transform from="$[nope]"/>"#, "names no set or uset"),
        (r#"<transform from="a?"/>"#, "can match the empty text"),
    ];
    for (rule, needle) in errors {
        let err = error_of(&format!(
            r#"<variables><set id="up" value="A B"/><set id="low" value="a b"/><set id="three" value="a b c"/><uset id="v" value="[a]"/></variables>
               <transforms type="simple"><transformGroup>{rule}</transformGroup></transforms>"#
        ));
        assert!(err.contains(needle), "{rule}: {err}");
        assert!(err.contains("transformGroup/transform[from="), "{err}");
    }
}

// [spec:kbdgen:syn:ldml.xml.from+1/test]
#[test]
fn any_marker_in_a_class_becomes_a_group() {
    let r = resolved(&with_layers(
        r#"<transforms type="simple"><transformGroup><transform from="[\m{.}a]x"/></transformGroup></transforms>"#,
    ));
    let rule = &simple_rules(&r)[0];
    assert!(matches!(
        rule.from.nodes[0].alternatives[0][0].atom,
        Atom::Group(1)
    ));
    assert_eq!(rule.from.nodes[1].alternatives[0][0].atom, Atom::AnyMarker);
    assert_eq!(r.keyboard.context_len, 3);
}

// [spec:kbdgen:req:ldml.model.nfd+1/test]
#[test]
fn enabled_normalization_decomposes_texts() {
    let body = r#"<displays><display output="é" display="é!"/></displays>
                  <keys><key id="e" output="é"/></keys>
                  <variables><set id="s" value="ñ"/></variables>
                  <transforms type="simple"><transformGroup><transform from="é?$[s]é" to="ö"/></transformGroup></transforms>"#;
    let on = resolved(&with_layers(body));
    assert_eq!(on.keyboard.normalization, Normalization::Enabled);
    let kb = &on.keyboard;
    assert_eq!(
        kb.key(kb.key_index("e").unwrap()).unwrap().output,
        Text::from("e\u{301}")
    );
    assert_eq!(
        kb.displays.entries[0].target,
        DisplayTarget::Output(Text::from("e\u{301}"))
    );
    assert_eq!(
        kb.displays.entries[0].display, "é!",
        "display strings are never normalized"
    );
    assert_eq!(kb.sets[0], [Text::from("n\u{303}")]);
    let rule = &simple_rules(&on)[0];
    assert_eq!(rule.to, [ReplacementItem::Text(Text::from("o\u{308}"))]);
    assert!(matches!(
        rule.from.nodes[0].alternatives[0][0].atom,
        Atom::Group(1)
    ));
    let off = resolved(&with_layers(&format!("{DISABLED}{body}")));
    let kb = &off.keyboard;
    assert_eq!(
        kb.key(kb.key_index("e").unwrap()).unwrap().output,
        Text::from("é")
    );
}

// [spec:kbdgen:req:ldml.xml.nfd-classes+1/test]
#[test]
fn non_nfd_class_members_error_or_warn() {
    let listed = error_of(
        r#"<transforms type="simple"><transformGroup><transform from="[aé]"/></transformGroup></transforms>"#,
    );
    assert!(listed.contains("U+00E9 is not NFD"), "{listed}");
    let uset = error_of(
        r#"<variables><uset id="u" value="[é]"/></variables>
           <transforms type="simple"><transformGroup><transform from="$[u]"/></transformGroup></transforms>"#,
    );
    assert!(uset.contains("U+00E9 is not NFD"), "{uset}");
    let r = resolved(&with_layers(
        r#"<transforms type="simple"><transformGroup><transform from="[\u{20}-\u{1FF}]"/></transformGroup></transforms>"#,
    ));
    assert!(
        r.warnings
            .iter()
            .any(|w| w.message.contains("not NFD; they are removed"))
    );
    let ranges = &r.keyboard.classes[0].ranges;
    assert!(
        ranges
            .iter()
            .all(|range| !(range.lo..=range.hi).contains(&'é'))
    );
    assert!(
        ranges
            .iter()
            .any(|range| (range.lo..=range.hi).contains(&'a'))
    );
    let off = resolved(&with_layers(&format!(
        r#"{DISABLED}<transforms type="simple"><transformGroup><transform from="[é]"/></transformGroup></transforms>"#
    )));
    assert!(off.warnings.iter().all(|w| !w.message.contains("NFD")));
}

// [spec:kbdgen:req:ldml.xml.nfd-classes+1/test]
#[test]
fn adjacent_listed_non_nfd_values_error() {
    for from in [
        r"[\u{C0}\u{C1}]",
        r"[a\u{C1}\u{C0}b]",
        r"[\u{C0 C1}]",
        r"[^\u{C0}\u{C1}]",
    ] {
        let e = error_of(&format!(
            r#"<transforms type="simple"><transformGroup><transform from="{from}"/></transformGroup></transforms>"#
        ));
        assert!(
            e.contains("is not NFD and may not be listed"),
            "{from}: {e}"
        );
    }
    for value in [r"[\u{C0}\u{C1}]", r"[\u{BF}\u{C0}]"] {
        let e = error_of(&format!(
            r#"<variables><uset id="u" value="{value}"/><uset id="v" value="[$[u] a]"/></variables>
               <transforms type="simple"><transformGroup><transform from="$[v]"/></transformGroup></transforms>"#
        ));
        assert!(e.contains("U+00C0 is not NFD"), "{value}: {e}");
    }
}

// [spec:kbdgen:req:ldml.xml.nfd-classes+1/test]
#[test]
fn ranges_over_non_nfd_values_only_warn() {
    for from in [r"[\u{C0}-\u{C1}]", r"[\u{BF}-\u{C0}\u{C1}-\u{C2}]", "$[u]"] {
        let r = resolved(&with_layers(&format!(
            r#"<variables><uset id="u" value="[\u{{BF}}-\u{{C1}}]"/></variables>
               <transforms type="simple"><transformGroup><transform from="{from}"/></transformGroup></transforms>"#
        )));
        assert!(
            r.warnings
                .iter()
                .any(|w| w.message.contains("not NFD; they are removed")),
            "{from}"
        );
        let ranges = &r.keyboard.classes[0].ranges;
        assert!(
            ranges.iter().all(|range| !range.contains('\u{C0}')),
            "{from}"
        );
    }
    let single = resolved(&with_layers(
        r#"<transforms type="simple"><transformGroup><transform from="[\u{BF}]"/></transformGroup></transforms>"#,
    ));
    assert!(single.warnings.iter().all(|w| !w.message.contains("NFD")));
}

// [spec:kbdgen:sem:ldml.xml.resolve+1/test]
#[test]
fn reorder_groups_merge_and_sort() {
    let r = resolved(&with_layers(
        r#"<transforms type="simple"><transformGroup>
             <reorder from="[\u{103D}\u{1082}]" order="25"/>
             <reorder from="\u{1004}\u{103A}\u{1039}" order="-1"/>
             <reorder before="\u{1A6B}" from="\u{1A60}\u{1A45}" order="10"/>
             <reorder from="\u{103D}" preBase="1" order="25"/>
           </transformGroup></transforms>"#,
    ));
    let TransformGroup::Reorder(rules) = &r.keyboard.simple[0] else {
        panic!("expected reorder");
    };
    let shapes: Vec<(usize, usize)> = rules.iter().map(|rule| rule.priority()).collect();
    assert_eq!(shapes, [(3, 0), (2, 1), (1, 0), (1, 0)]);
    assert_eq!(rules[0].order, [-1, -1, -1]);
    assert_eq!(
        rules[2].pre_base,
        [true],
        "the later rule overrides the intersection"
    );
    assert_eq!(rules[3].pre_base, [false]);
    let bad = error_of(
        r#"<transforms type="simple"><transformGroup><reorder from="a" order="1" tertiary="2"/></transformGroup></transforms>"#,
    );
    assert!(
        bad.contains("tertiary character must have order 0"),
        "{bad}"
    );
    let marker = error_of(
        r#"<transforms type="simple"><transformGroup><reorder from="\m{x}"/></transformGroup></transforms>"#,
    );
    assert!(marker.contains("does not match markers"), "{marker}");
}

// [spec:kbdgen:sem:ldml.xml.resolve+1/test]
#[test]
fn displays_override_by_target() {
    let r = resolved(&with_layers(
        r#"<displays>
             <display output="\u{301}" display="◌́"/>
             <display keyId="a" display="A!"/>
             <display output="&#x301;" display="´"/>
             <display keyId="nope" display="?"/>
             <displayOptions baseCharacter="x"/>
           </displays>"#,
    ));
    let d = &r.keyboard.displays;
    assert_eq!(d.entries.len(), 2);
    assert_eq!(
        d.entries[0].display, "´",
        "the later display replaces in place"
    );
    assert_eq!(d.display_base.as_deref(), Some("x"));
    assert!(
        r.warnings
            .iter()
            .any(|w| w.message.contains("no key has id nope"))
    );
}

// [spec:kbdgen:req:ldml.model.context-len+1/test]
#[test]
fn context_len_is_computed() {
    let r = resolved(&with_layers(
        r#"<transforms type="simple"><transformGroup><transform from="ab{1,3}"/></transformGroup></transforms>
           <transforms type="backspace"><transformGroup><transform from="xyz.{0,9}"/></transformGroup></transforms>"#,
    ));
    assert_eq!(r.keyboard.context_len, 13);
    let too_long = error_of(
        r#"<transforms type="simple"><transformGroup><transform from=".{9,9}.{9,9}.{9,9}.{9,9}.{9,9}.{9,9}.{9,9}.{9,9}"/></transformGroup></transforms>"#,
    );
    assert!(too_long.contains("context_len exceeds 64"), "{too_long}");
}
