//! `IBusText`, as libibus serializes it (`src/ibustext.c`,
//! `src/ibusattrlist.c`, `src/ibusattribute.c`): each object is a
//! structure of its type name, its attachments, which the engine never
//! sets, and its fields. A text is `(sa{sv}sv)`, its attribute list
//! `(sa{sv}av)`, an attribute `(sa{sv}uuuu)`.

use std::collections::HashMap;

use zbus::{Structure, Variant};

/// `IBUS_ATTR_TYPE_UNDERLINE`.
const UNDERLINE: u32 = 1;
/// `IBUS_ATTR_UNDERLINE_SINGLE`.
const UNDERLINE_SINGLE: u32 = 1;

fn attachments() -> HashMap<String, Variant<'static>> {
    HashMap::new()
}

/// An `IBusText` of `text`, underlined over its whole length in scalar
/// values when `underline` is set and it is not empty.
// [spec:kbdgen:req:linux.ibus.factory]
pub fn encode(text: &str, underline: bool) -> zbus::Result<Variant<'static>> {
    let mut attributes: Vec<Variant<'static>> = Vec::new();
    let length = u32::try_from(text.chars().count()).unwrap_or(u32::MAX);
    if underline && length > 0 {
        let attribute = Structure::builder()
            .add_field("IBusAttribute")
            .add_field(attachments())
            .add_field(UNDERLINE)
            .add_field(UNDERLINE_SINGLE)
            .add_field(0u32)
            .add_field(length)
            .build()?;
        attributes.push(Variant::Structure(attribute));
    }
    let list = Structure::builder()
        .add_field("IBusAttrList")
        .add_field(attachments())
        .add_field(attributes)
        .build()?;
    let text = Structure::builder()
        .add_field("IBusText")
        .add_field(attachments())
        .add_field(text.to_owned())
        .add_field(Variant::Structure(list))
        .build()?;
    Ok(Variant::Structure(text))
}

/// The text of an `IBusText`, or `None` for anything else.
// [spec:kbdgen:req:linux.ibus.factory]
pub fn decode(value: &Variant<'_>) -> Option<String> {
    let mut value = value;
    while let Variant::Variant(inner) = value {
        value = inner;
    }
    let Variant::Structure(structure) = value else {
        return None;
    };
    match structure.fields() {
        [Variant::Str(kind), _, Variant::Str(text), ..] if kind.as_str() == "IBusText" => {
            Some(text.as_str().to_owned())
        }
        _ => None,
    }
}
