use zbus::{Structure, Variant};

use crate::dbus::text::{decode, encode};

fn fields<'a>(value: &'a Variant<'a>) -> &'a [Variant<'a>] {
    let Variant::Structure(structure) = value else {
        panic!("a structure, got {value:?}");
    };
    structure.fields()
}

fn inner<'a>(value: &'a Variant<'a>) -> &'a Variant<'a> {
    let Variant::Variant(inner) = value else {
        panic!("a variant, got {value:?}");
    };
    inner
}

// [spec:kbdgen:req:linux.ibus.factory/test]
#[test]
fn preedit_text_is_underlined_ibus_text() {
    let text = encode("a𝕫", true).unwrap();
    assert_eq!(text.value_signature().to_string(), "(sa{sv}sv)");
    let [kind, attachments, s, attributes] = fields(&text) else {
        panic!("four fields");
    };
    assert_eq!(kind, &Variant::from("IBusText"));
    assert_eq!(attachments.value_signature().to_string(), "a{sv}");
    assert_eq!(s, &Variant::from("a𝕫"));
    let list = inner(attributes);
    assert_eq!(list.value_signature().to_string(), "(sa{sv}av)");
    let [kind, _, Variant::Array(attributes)] = fields(list) else {
        panic!("three fields");
    };
    assert_eq!(kind, &Variant::from("IBusAttrList"));
    let [attribute] = attributes.inner() else {
        panic!("one attribute");
    };
    let attribute = inner(attribute);
    assert_eq!(attribute.value_signature().to_string(), "(sa{sv}uuuu)");
    assert_eq!(
        &fields(attribute)[2..],
        [1u32, 1, 0, 2].map(Variant::from).as_slice(),
        "underline, single, over two scalar values"
    );
}

// [spec:kbdgen:req:linux.ibus.factory/test]
#[test]
fn committed_text_has_no_attributes() {
    for (text, underline) in [("ʠ", false), ("", true)] {
        let value = encode(text, underline).unwrap();
        let list = inner(&fields(&value)[3]);
        let Variant::Array(attributes) = &fields(list)[2] else {
            panic!("an array");
        };
        assert!(attributes.is_empty(), "{text:?}");
        assert_eq!(decode(&value).as_deref(), Some(text));
    }
}

// [spec:kbdgen:req:linux.ibus.factory/test]
#[test]
fn surrounding_text_decodes_through_variants() {
    let value = encode("ab", false).unwrap();
    assert_eq!(decode(&Variant::new(value.clone())).as_deref(), Some("ab"));
    let other = Variant::Structure(Structure::from(("IBusProperty", "x", "y")));
    assert_eq!(decode(&other), None);
    assert_eq!(decode(&Variant::from("ab")), None);
}
