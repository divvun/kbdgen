//! Reads a `.keylayout` the writer produced back into a keylayout input,
//! so tests simulate the bytes that ship rather than the generator's data.

use std::str::FromStr;

use anyhow::{Context, Result, anyhow, bail};
use indexmap::IndexMap;
use language_tags::LanguageTag;
use xmlem::{Document, Element};

use crate::build::macos::input::{Binding, Key, KeyMap, KeylayoutInput, When};

/// The XML reader rejects `&#x0000;`, which v3 writes for a U+0000 output,
/// so it is read as this noncharacter and turned back afterwards.
const NUL_STAND_IN: &str = "\u{FFFF}";

fn text(document: &Document, element: Element, name: &str) -> Option<String> {
    let value = element.attribute(document, name)?;
    Some(value.replace(NUL_STAND_IN, "\0"))
}

fn attribute(document: &Document, element: Element, name: &str) -> Result<String> {
    element
        .attribute(document, name)
        .map(str::to_owned)
        .ok_or_else(|| anyhow!("<{}> has no {name}", element.name(document)))
}

fn number(document: &Document, element: Element, name: &str) -> Result<usize> {
    let text = attribute(document, element, name)?;
    text.parse().with_context(|| format!("{name}={text:?}"))
}

fn children(document: &Document, element: Element, name: &str) -> Vec<Element> {
    element
        .children(document)
        .into_iter()
        .filter(|child| child.name(document) == name)
        .collect()
}

fn required(document: &Document, element: Element, name: &str) -> Result<Element> {
    let found = children(document, element, name);
    if found.len() != 1 {
        bail!("{} <{name}> elements, not one", found.len());
    }
    Ok(found[0])
}

fn when(document: &Document, element: Element) -> Result<When> {
    for unsupported in ["through", "multiplier"] {
        if element.attribute(document, unsupported).is_some() {
            bail!("<when {unsupported}> is not simulated");
        }
    }
    Ok(When {
        state: attribute(document, element, "state")?,
        output: text(document, element, "output"),
        next: element.attribute(document, "next").map(str::to_owned),
    })
}

fn whens(document: &Document, element: Element) -> Result<Vec<When>> {
    children(document, element, "when")
        .into_iter()
        .map(|w| when(document, w))
        .collect()
}

/// The actions by id. A repeated id keeps its first `<action>`.
fn actions(document: &Document, root: Element) -> Result<IndexMap<String, Vec<When>>> {
    let mut out = IndexMap::new();
    let Some(actions) = children(document, root, "actions").into_iter().next() else {
        return Ok(out);
    };
    for action in children(document, actions, "action") {
        let id = attribute(document, action, "id")?;
        if !out.contains_key(&id) {
            out.insert(id, whens(document, action)?);
        }
    }
    Ok(out)
}

fn key(
    document: &Document,
    element: Element,
    actions: &IndexMap<String, Vec<When>>,
) -> Result<Key> {
    let code = u16::try_from(number(document, element, "code")?)?;
    let binding = match (
        text(document, element, "output"),
        element.attribute(document, "action"),
    ) {
        (Some(output), None) => Binding::Output(output),
        (None, Some(id)) => Binding::Action {
            id: id.to_owned(),
            whens: actions
                .get(id)
                .cloned()
                .ok_or_else(|| anyhow!("key {code}: no action {id}"))?,
        },
        _ => bail!("key {code} needs exactly one of output and action"),
    };
    Ok(Key { code, binding })
}

fn maps(
    document: &Document,
    root: Element,
    actions: &IndexMap<String, Vec<When>>,
) -> Result<Vec<KeyMap>> {
    let modifier_map = required(document, root, "modifierMap")?;
    let key_map_set = required(document, root, "keyMapSet")?;
    let key_maps = children(document, key_map_set, "keyMap");
    let mut maps = vec![KeyMap::default(); key_maps.len()];
    for (index, element) in key_maps.into_iter().enumerate() {
        if number(document, element, "index")? != index {
            bail!("keyMap {index} is out of order");
        }
        for k in children(document, element, "key") {
            maps[index].keys.push(key(document, k, actions)?);
        }
    }
    for select in children(document, modifier_map, "keyMapSelect") {
        let index = number(document, select, "mapIndex")?;
        let map = maps
            .get_mut(index)
            .ok_or_else(|| anyhow!("keyMapSelect {index} has no keyMap"))?;
        for modifier in children(document, select, "modifier") {
            map.modifiers.push(attribute(document, modifier, "keys")?);
        }
    }
    Ok(maps)
}

/// Parses `xml` as the input of a layout tagged `tag`, without display
/// names, which live in the bundle.
pub fn parse(xml: &str, tag: &str) -> Result<KeylayoutInput> {
    let xml = xml.replace("&#x0000;", "&#xFFFF;");
    let document = Document::from_str(&xml).map_err(|e| anyhow!("{e:?}"))?;
    let root = document.root();
    let actions = actions(&document, root)?;
    let maps = maps(&document, root, &actions)?;
    let modifier_map = required(&document, root, "modifierMap")?;
    let terminators = match children(&document, root, "terminators").into_iter().next() {
        Some(element) => whens(&document, element)?,
        None => Vec::new(),
    };
    Ok(KeylayoutInput {
        name: attribute(&document, root, "name")?,
        tag: LanguageTag::from_str(tag)?,
        display_names: IndexMap::new(),
        default_index: number(&document, modifier_map, "defaultIndex")?,
        maps,
        terminators,
    })
}
