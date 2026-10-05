//! Derives the generator input from the current bundle format: a layout's
//! `windows` section, its `transforms`, `decimal` and display names, and the
//! project and Windows target metadata.

use anyhow::{Result, bail};
use indexmap::IndexMap;
use language_tags::LanguageTag;

use crate::{
    bundle::{
        KbdgenBundle,
        layout::{Layout, Transform, WindowsTarget, windows::WindowsKbdLayer},
    },
    util::{UNICODE_ESCAPES, split_keys},
};

use super::{
    diag::Diagnostics,
    input::{B11, DeadKeyNode, KeyValue, Layer, LayoutInput, Metadata, POSITION_NAMES},
};

/// Number of tokens a desktop layer string assigns, `E00`..`B10`.
const DESKTOP_KEY_COUNT: usize = 48;

/// The token that marks a position as having no key.
const NO_KEY_TOKEN: &str = r"\u{0}";

/// LCID of a locale that has no LCID of its own (`LOCALE_CUSTOM_UNSPECIFIED`).
const LCID_UNSPECIFIED: u32 = 0x2000;

/// Builds the input for one layout with a `windows` section.
// [spec:kbdgen:req:kbdl.input.bundle+1]
pub fn layout_input(
    bundle: &KbdgenBundle,
    language_tag: &LanguageTag,
    layout: &Layout,
    target: &WindowsTarget,
) -> Result<(LayoutInput, Diagnostics)> {
    let metadata = metadata(bundle, language_tag, layout, target)?;
    let mut diag = Diagnostics::new(metadata.name.clone());

    let mut layers = IndexMap::new();
    for (windows_layer, tokens) in &target.primary.layers {
        let layer = layer_of(windows_layer);
        let tokens = split_keys(tokens);
        if tokens.len() < DESKTOP_KEY_COUNT {
            bail!(
                "{}: layer {} has {} keys, expected at least {}",
                metadata.name,
                layer,
                tokens.len(),
                DESKTOP_KEY_COUNT
            );
        }
        let dead_keys = target
            .dead_keys
            .as_ref()
            .and_then(|dead_keys| dead_keys.get(windows_layer));

        let mut values = Vec::with_capacity(POSITION_NAMES.len());
        for (index, token) in tokens.iter().take(DESKTOP_KEY_COUNT).enumerate() {
            let position = POSITION_NAMES[index];
            let value = token_value(token, &metadata.name, position, layer, &mut diag)?;
            values.push(value.map(|text| {
                let dead = dead_keys.is_some_and(|list| list.contains(&text));
                KeyValue { text, dead }
            }));
        }
        debug_assert_eq!(values.len(), B11);
        values.push(None);
        layers.insert(layer, values);
    }

    let dead_key_tree = layout
        .transforms
        .as_ref()
        .map(|transforms| {
            transforms
                .iter()
                .map(|(input, node)| (input.clone(), dead_key_node(node)))
                .collect()
        })
        .unwrap_or_default();

    let input = LayoutInput {
        metadata,
        decimal: layout.decimal.clone(),
        layers,
        dead_key_tree,
        ..Default::default()
    };
    Ok((input, diag))
}

fn layer_of(layer: &WindowsKbdLayer) -> Layer {
    match layer {
        WindowsKbdLayer::Default => Layer::Default,
        WindowsKbdLayer::Shift => Layer::Shift,
        WindowsKbdLayer::Caps => Layer::Caps,
        WindowsKbdLayer::CapsAndShift => Layer::CapsShift,
        WindowsKbdLayer::Alt => Layer::Alt,
        WindowsKbdLayer::AltAndShift => Layer::AltShift,
        WindowsKbdLayer::AltAndCaps => Layer::AltCaps,
        WindowsKbdLayer::Ctrl => Layer::Ctrl,
    }
}

/// Decodes one layer token into its value text, `None` meaning "no key".
fn token_value(
    token: &str,
    layout: &str,
    position: &str,
    layer: Layer,
    diag: &mut Diagnostics,
) -> Result<Option<String>> {
    if token == NO_KEY_TOKEN {
        return Ok(None);
    }
    let decoded = decode_escapes(token).map_err(|scalar| {
        anyhow::anyhow!(
            "{layout}: key {position} in layer {layer}: escape \\u{{{scalar:x}}} is not a Unicode scalar value"
        )
    })?;
    if decoded.is_empty() || decoded.starts_with('\0') {
        diag.warn(format!(
            "key {position} in layer {layer}: value {:?} is empty or starts with U+0000; treated as no key",
            token
        ));
        return Ok(None);
    }
    Ok(Some(decoded))
}

/// Replaces every `\u{H}` escape with its character, failing with the value
/// of the first escape that is not a Unicode scalar value.
// [spec:kbdgen:syn:keys.escape+1]
fn decode_escapes(token: &str) -> std::result::Result<String, u32> {
    let mut output = String::with_capacity(token.len());
    let mut last = 0;
    for captures in UNICODE_ESCAPES.captures_iter(token) {
        let (Some(whole), Some(hex)) = (captures.get(0), captures.get(1)) else {
            continue;
        };
        let scalar = u32::from_str_radix(hex.as_str(), 16).map_err(|_| u32::MAX)?;
        let character = char::from_u32(scalar).ok_or(scalar)?;
        output.push_str(&token[last..whole.start()]);
        output.push(character);
        last = whole.end();
    }
    output.push_str(&token[last..]);
    Ok(output)
}

fn dead_key_node(transform: &Transform) -> DeadKeyNode {
    match transform {
        Transform::End(output) => DeadKeyNode::Leaf(output.clone()),
        Transform::More(children) => DeadKeyNode::Branch(
            children
                .iter()
                .map(|(input, node)| (input.clone(), dead_key_node(node)))
                .collect(),
        ),
    }
}

/// `kbd` followed by `windows.config.id`, or else by the first five scalar
/// values of the language tag.
// [spec:kbdgen:req:kbdl.metadata.bundle]
pub fn keyboard_name(language_tag: &LanguageTag, target: &WindowsTarget) -> String {
    format!(
        "kbd{}",
        target
            .config
            .as_ref()
            .and_then(|config| config.id.clone())
            .unwrap_or_else(|| language_tag.as_str().chars().take(5).collect())
    )
}

// [spec:kbdgen:req:kbdl.metadata.bundle]
fn metadata(
    bundle: &KbdgenBundle,
    language_tag: &LanguageTag,
    layout: &Layout,
    target: &WindowsTarget,
) -> Result<Metadata> {
    let name = keyboard_name(language_tag, target);

    let primary = language_tag.primary_language();
    let Some(display_name) = layout
        .display_names
        .iter()
        .find(|(tag, _)| tag.as_str() == primary)
        .map(|(_, name)| name.clone())
    else {
        bail!("{name}: displayNames has no entry for the primary language subtag {primary:?}");
    };

    let (lcid, locale_name) = locale(language_tag, target);
    let windows = bundle.targets.windows.as_ref();

    Ok(Metadata {
        name,
        description: display_name.clone(),
        language_name: display_name,
        locale_name,
        lcid,
        company: bundle.project.organisation.clone(),
        copyright: bundle.project.copyright.clone(),
        version: windows.map(|windows| windows.version.clone()),
        build: windows.map(|windows| windows.build.clone()),
    })
}

/// The LCID and locale name of a layout. The LCID always comes from the
/// layout's own language tag, never from `windows.config.locale`.
// [spec:kbdgen:req:kbdl.metadata.locale]
fn locale(language_tag: &LanguageTag, target: &WindowsTarget) -> (u32, String) {
    let record = iso639::lcid::get(
        language_tag.primary_language(),
        language_tag.script(),
        language_tag.region(),
    );
    let lcid = record.map_or(LCID_UNSPECIFIED, |record| record.lcid);
    let locale_name = target
        .config
        .as_ref()
        .and_then(|config| config.locale.as_ref())
        .map(|locale| locale.to_string())
        .unwrap_or_else(|| match record {
            Some(_) => language_tag.to_string(),
            None => format!(
                "{}-{}-{}",
                language_tag.primary_language(),
                language_tag.script().unwrap_or("Latn"),
                language_tag.region().unwrap_or("001")
            ),
        });
    (lcid, locale_name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bundle::KbdgenBundle;

    const KEYS_48: &str = "a b c d e f g h i j k l m n o p q r s t u v w x y z 1 2 3 4 5 6 7 8 9 0 A B C D E F G H I J K L";

    fn layout(yaml: &str) -> Layout {
        serde_yaml::from_str(yaml).unwrap()
    }

    fn bundle_with(tag: &str, layout: Layout) -> KbdgenBundle {
        let mut layouts = IndexMap::new();
        layouts.insert(tag.parse().unwrap(), layout);
        KbdgenBundle::new_test("test".into(), layouts)
    }

    fn input_for(yaml: &str) -> Result<(LayoutInput, Diagnostics)> {
        let layout = layout(yaml);
        let tag = layout.language_tag.clone();
        let bundle = bundle_with(tag.as_str(), layout);
        let layout = bundle.layouts.get(&tag).unwrap();
        layout_input(&bundle, &tag, layout, layout.windows.as_ref().unwrap())
    }

    fn simple_yaml(tag: &str, config: &str) -> String {
        format!(
            "languageTag: {tag}\ndisplayNames:\n  {primary}: Autonym\nwindows:\n{config}  primary:\n    layers:\n      default: {KEYS_48}\n",
            primary = tag.split('-').next().unwrap()
        )
    }

    // [spec:kbdgen:req:kbdl.input.bundle+1/test]
    #[test]
    fn tokens_decode_and_mark_dead_keys_with_b11_absent() {
        let yaml = format!(
            r#"languageTag: se
displayNames:
  se: Davvisámegiella
decimal: ","
windows:
  primary:
    layers:
      default: '\u{{0}} \u{{00}} \u{{301}}a ´ {rest}'
      shift: '{keys}'
  deadKeys:
    default: ['´']
transforms:
  ´:
    ' ': ´
    a: á
"#,
            rest = KEYS_48.split(' ').skip(4).collect::<Vec<_>>().join(" "),
            keys = KEYS_48
        );
        let (input, diag) = input_for(&yaml).unwrap();
        let default = &input.layers[&Layer::Default];
        assert_eq!(default.len(), 49);
        assert_eq!(default[0], None);
        assert_eq!(default[1], None);
        assert_eq!(default[2], Some(KeyValue::new("\u{301}a")));
        assert_eq!(default[3], Some(KeyValue::dead("´")));
        assert_eq!(default[48], None);
        assert_eq!(input.layers[&Layer::Shift][48], None);
        assert_eq!(input.layers[&Layer::Shift][0], Some(KeyValue::new("a")));
        assert_eq!(diag.warnings().len(), 1);
        assert!(diag.warnings()[0].contains("E01"));
        assert!(diag.warnings()[0].contains("default"));
        assert_eq!(input.decimal.as_deref(), Some(","));
        let DeadKeyNode::Branch(children) = &input.dead_key_tree["´"] else {
            panic!("´ must be a branch");
        };
        assert_eq!(children[" "], DeadKeyNode::Leaf("´".into()));
        assert!(input.extra_modifiers.is_empty());
        assert!(input.dead_key_names.is_empty());
        assert!(input.key_name_overrides.is_empty());
        assert!(!input.shift_lock && !input.lrm_rlm);
    }

    // [spec:kbdgen:req:kbdl.input.bundle+1/test]
    #[test]
    fn short_layers_and_invalid_escapes_are_errors() {
        let yaml = "languageTag: se\ndisplayNames:\n  se: X\nwindows:\n  primary:\n    layers:\n      default: a b c\n";
        let error = input_for(yaml).unwrap_err().to_string();
        assert!(
            error.contains("default") && error.contains("3 keys"),
            "{error}"
        );

        let yaml = simple_yaml("se", "").replace("default: a ", "default: \\u{d800} ");
        let error = input_for(&yaml).unwrap_err().to_string();
        assert!(error.contains("E00") && error.contains("d800"), "{error}");
    }

    // [spec:kbdgen:req:kbdl.metadata.bundle/test]
    #[test]
    fn keyboard_name_comes_from_the_id_or_the_tag() {
        let (input, _) = input_for(&simple_yaml("sjd-Cyrl-RU", "")).unwrap();
        assert_eq!(input.metadata.name, "kbdsjd-C");
        assert_eq!(input.metadata.description, "Autonym");
        assert_eq!(input.metadata.language_name, "Autonym");
        assert_eq!(input.metadata.company, "Test");
        assert_eq!(input.metadata.version, None);

        let (input, _) = input_for(&simple_yaml("se-FI", "")).unwrap();
        assert_eq!(input.metadata.name, "kbdse-FI");

        let (input, _) = input_for(&simple_yaml("myv", "  config:\n    id: myvwi\n")).unwrap();
        assert_eq!(input.metadata.name, "kbdmyvwi");
    }

    // [spec:kbdgen:req:kbdl.metadata.bundle/test]
    #[test]
    fn missing_display_name_is_an_error() {
        let yaml = simple_yaml("se", "").replace("  se: Autonym", "  fi: Autonym");
        let error = input_for(&yaml).unwrap_err().to_string();
        assert!(error.contains("displayNames"), "{error}");
    }

    // [spec:kbdgen:req:kbdl.metadata.locale/test]
    #[test]
    fn lcid_and_locale_name_fall_back() {
        let (input, _) = input_for(&simple_yaml("se-NO", "")).unwrap();
        assert_eq!(input.metadata.lcid, 0x043b);
        assert_eq!(input.metadata.locale_name, "se-NO");

        let (input, _) = input_for(&simple_yaml("vro", "")).unwrap();
        assert_eq!(input.metadata.lcid, 0x2000);
        assert_eq!(input.metadata.locale_name, "vro-Latn-001");

        let (input, _) = input_for(&simple_yaml("sjd-Cyrl", "")).unwrap();
        assert_eq!(input.metadata.lcid, 0x2000);
        assert_eq!(input.metadata.locale_name, "sjd-Cyrl-001");

        let (input, _) =
            input_for(&simple_yaml("se-FI", "  config:\n    locale: se-Latn-NO\n")).unwrap();
        assert_eq!(input.metadata.lcid, 0x0c3b);
        assert_eq!(input.metadata.locale_name, "se-Latn-NO");
    }
}
