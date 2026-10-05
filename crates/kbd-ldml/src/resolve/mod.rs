//! Resolving a source document to the keyboard model (`ldml.xml.resolve`).

mod imports;
mod keys;
mod layers;
mod reorder;
mod specials;
mod transforms;
mod validate;
mod vars;

use kbd_model::{Class, ClassIndex, Info, Keyboard, MarkerIndex, Normalization, SetIndex, Text};

use crate::diag::{Diagnostic, Error, Result};
use crate::escape::{Piece, decode_text};
use crate::nfd::{IcuNfd, nfd_text};
use crate::read::SourceDocument;
use crate::special::{Extensions, kbdgen_prefix};
use crate::tree::El;
use vars::Vars;

pub use keys::implied_keys;
pub use layers::{encode_modifiers, implied_form, parse_modifiers, parse_width};

/// A resolved keyboard: the model, the kbdgen data outside it, and the
/// warnings of reading and resolving.
#[derive(Debug, Clone)]
pub struct Resolved {
    pub keyboard: Keyboard,
    pub extensions: Extensions,
    pub warnings: Vec<Diagnostic>,
}

/// State shared by the resolution steps.
pub(crate) struct Ctx {
    pub(crate) kb: Keyboard,
    pub(crate) vars: Vars,
    pub(crate) warnings: Vec<Diagnostic>,
    pub(crate) kbdgen: Option<String>,
    pub(crate) extensions: Extensions,
}

impl Ctx {
    pub(crate) fn enabled(&self) -> bool {
        self.kb.normalization == Normalization::Enabled
    }

    /// The index of marker `name`. A name is interned on first appearance,
    /// so the table follows document order.
    pub(crate) fn marker(&mut self, el: &El, name: &str) -> Result<MarkerIndex> {
        if let Some(i) = self.kb.marker_index(name) {
            return Ok(i);
        }
        let i = MarkerIndex::try_from(self.kb.markers.len())
            .map_err(|_| el.error("more than 65536 markers"))?;
        self.kb.markers.push(name.to_string());
        Ok(i)
    }

    /// A text from decoded pieces: markers interned, and NFD when
    /// normalization is enabled (`ldml.model.nfd`).
    pub(crate) fn text(&mut self, el: &El, pieces: &[Piece]) -> Result<Text> {
        let mut text = Text::new();
        for piece in pieces {
            match piece {
                Piece::Char(c) => text.push_char(*c),
                Piece::Marker(m) => {
                    let i = self.marker(el, m)?;
                    text.push_marker(i);
                }
            }
        }
        Ok(if self.enabled() {
            nfd_text(&text)
        } else {
            text
        })
    }

    /// Decodes a text-valued attribute (`ldml.xml.escape`).
    pub(crate) fn decode(&self, el: &El, attribute: &str, value: &str) -> Result<Vec<Piece>> {
        decode_text(value, &|id| self.vars.string_pieces(id))
            .map_err(|e| el.attr_error(attribute, e.to_string()))
    }

    /// The text of attribute `attribute`, empty when absent.
    pub(crate) fn text_attr(&mut self, el: &El, attribute: &str) -> Result<Text> {
        match el.attr(attribute) {
            None => Ok(Text::new()),
            Some(value) => {
                let pieces = self.decode(el, attribute, value)?;
                self.text(el, &pieces)
            }
        }
    }

    /// A display string: escapes decoded and variables substituted, never
    /// normalized, and holding no marker.
    pub(crate) fn display_attr(&self, el: &El, attribute: &str) -> Result<String> {
        let value = el.attr(attribute).unwrap_or("");
        let pieces = self.decode(el, attribute, value)?;
        pieces
            .into_iter()
            .map(|p| match p {
                Piece::Char(c) => Ok(c),
                Piece::Marker(m) => Err(el.attr_error(
                    attribute,
                    format!("a display string cannot hold the marker \\m{{{m}}}"),
                )),
            })
            .collect()
    }

    pub(crate) fn set_index(&mut self, el: &El, items: Vec<Text>) -> Result<SetIndex> {
        if let Some(i) = self.kb.sets.iter().position(|s| *s == items) {
            return SetIndex::try_from(i).map_err(|_| el.error("too many sets"));
        }
        let i = SetIndex::try_from(self.kb.sets.len()).map_err(|_| el.error("too many sets"))?;
        self.kb.sets.push(items);
        Ok(i)
    }

    pub(crate) fn class_index(&mut self, el: &El, class: Class) -> Result<ClassIndex> {
        if let Some(i) = self.kb.classes.iter().position(|c| *c == class) {
            return ClassIndex::try_from(i).map_err(|_| el.error("too many classes"));
        }
        let i = ClassIndex::try_from(self.kb.classes.len())
            .map_err(|_| el.error("too many classes"))?;
        self.kb.classes.push(class);
        Ok(i)
    }

    pub(crate) fn warn(&mut self, d: Diagnostic) {
        self.warnings.push(d);
    }
}

fn header(root: &El) -> Result<Keyboard> {
    let locale = root.attr("locale").unwrap_or("").to_string();
    let conforms_to = root
        .attr("conformsTo")
        .and_then(|v| v.parse::<u8>().ok())
        .filter(|v| (45..=49).contains(v))
        .ok_or_else(|| root.attr_error("conformsTo", "conformsTo is not 45 to 49"))?;
    let info_el = root
        .child("info")
        .ok_or_else(|| root.error("info is required"))?;
    let optional = |a: &str| info_el.attr(a).map(str::to_string);
    let info = Info {
        name: info_el.attr("name").unwrap_or("").to_string(),
        author: optional("author"),
        layout: optional("layout"),
        indicator: optional("indicator"),
        attribution: optional("attribution"),
    };
    let mut kb = Keyboard::new(locale, conforms_to, info);
    kb.locales = root
        .child("locales")
        .map(|l| {
            l.children_named("locale")
                .filter_map(|e| e.attr("id").map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    kb.version = root
        .child("version")
        .and_then(|v| v.attr("number"))
        .map(str::to_string);
    if root.child("settings").and_then(|s| s.attr("normalization")) == Some("disabled") {
        kb.normalization = Normalization::Disabled;
    }
    Ok(kb)
}

/// A model invariant violation, reported on the root since the model's
/// sites name table positions rather than elements.
fn invariant(root: &El, e: kbd_model::InvariantError) -> Error {
    root.error(format!("invariant violated: {e}"))
}

// [spec:kbdgen:sem:ldml.xml.resolve+1]
/// Resolves a keyboard document to the model. The steps run in the order
/// of `ldml.xml.resolve`: imports, implied data and overrides; validation;
/// variables; escapes and markers; normalization; keys, flicks and
/// displays; forms and layers; patterns and replacements; reorder
/// split-and-merge; `context_len`; extensions; and the model invariants.
/// Markers are interned in document order: display outputs, key outputs,
/// transforms, then extensions; variables contribute where they are used.
/// Any error stops resolution.
pub fn resolve(source: &SourceDocument) -> Result<Resolved> {
    let tree = El::from_document(&source.name, &source.document)?;
    let mut warnings = source.warnings.clone();
    let root = imports::expand(source, tree, &mut warnings)?;
    let kbdgen = kbdgen_prefix(&root);
    validate::validate(&root, &mut warnings)?;
    let mut ctx = Ctx {
        kb: header(&root)?,
        vars: Vars::read(&root)?,
        warnings,
        kbdgen,
        extensions: Extensions::default(),
    };
    let displays = keys::intern_displays(&mut ctx, &root)?;
    keys::keys(&mut ctx, &root)?;
    keys::displays(&mut ctx, &root, displays)?;
    layers::layers(&mut ctx, &root)?;
    transforms::transforms(&mut ctx, &root)?;
    specials::keyboard(&mut ctx, &root)?;
    let len = ctx
        .kb
        .computed_context_len()
        .map_err(|e| invariant(&root, e))?;
    ctx.kb.context_len = u8::try_from(len).unwrap_or(u8::MAX);
    ctx.kb
        .validate(Some(&IcuNfd))
        .map_err(|e| invariant(&root, e))?;
    Ok(Resolved {
        keyboard: ctx.kb,
        extensions: ctx.extensions,
        warnings: ctx.warnings,
    })
}
