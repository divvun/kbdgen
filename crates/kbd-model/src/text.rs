//! Texts: scalar values interleaved with out-of-band marker references.

use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

/// An index into the keyboard's marker table (`Keyboard::markers`).
pub type MarkerIndex = u16;

/// One element of a [`Text`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum TextElem {
    /// A Unicode scalar value.
    Char(char),
    /// A marker reference. Markers never occupy a scalar value, so
    /// application text may contain any scalar value.
    Marker(MarkerIndex),
}

// [spec:kbdgen:def:ldml.model.text]
/// A sequence of scalar values and marker references.
///
/// The marker table lists marker names in order of first appearance in
/// document order; `\m{.}` is never an entry and appears only in patterns
/// (`Atom::AnyMarker`).
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Text(pub Vec<TextElem>);

impl Text {
    pub const fn new() -> Self {
        Text(Vec::new())
    }

    /// A text holding the scalar values of `s` and no markers.
    pub fn plain_from(s: &str) -> Self {
        Text(s.chars().map(TextElem::Char).collect())
    }

    pub fn elements(&self) -> &[TextElem] {
        &self.0
    }

    /// The number of elements, markers included.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether the text has no elements at all.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn push_char(&mut self, c: char) {
        self.0.push(TextElem::Char(c));
    }

    pub fn push_marker(&mut self, marker: MarkerIndex) {
        self.0.push(TextElem::Marker(marker));
    }

    /// The plain text: the scalar values with every marker removed.
    pub fn plain(&self) -> String {
        self.chars().collect()
    }

    /// The scalar values in order, markers skipped.
    pub fn chars(&self) -> impl Iterator<Item = char> + '_ {
        self.0.iter().filter_map(|e| match e {
            TextElem::Char(c) => Some(*c),
            TextElem::Marker(_) => None,
        })
    }

    /// The marker references in order.
    pub fn markers(&self) -> impl Iterator<Item = MarkerIndex> + '_ {
        self.0.iter().filter_map(|e| match e {
            TextElem::Marker(m) => Some(*m),
            TextElem::Char(_) => None,
        })
    }

    /// Whether the text contains no marker.
    pub fn is_plain(&self) -> bool {
        self.markers().next().is_none()
    }
}

impl From<&str> for Text {
    fn from(s: &str) -> Self {
        Text::plain_from(s)
    }
}

/// Whether `c` is an XML 1.0 `NameStartChar`.
fn is_name_start_char(c: char) -> bool {
    matches!(c,
        ':' | 'A'..='Z' | '_' | 'a'..='z'
        | '\u{C0}'..='\u{D6}' | '\u{D8}'..='\u{F6}' | '\u{F8}'..='\u{2FF}'
        | '\u{370}'..='\u{37D}' | '\u{37F}'..='\u{1FFF}' | '\u{200C}'..='\u{200D}'
        | '\u{2070}'..='\u{218F}' | '\u{2C00}'..='\u{2FEF}' | '\u{3001}'..='\u{D7FF}'
        | '\u{F900}'..='\u{FDCF}' | '\u{FDF0}'..='\u{FFFD}' | '\u{10000}'..='\u{EFFFF}')
}

/// Whether `c` is an XML 1.0 `NameChar`.
fn is_name_char(c: char) -> bool {
    is_name_start_char(c)
        || matches!(c,
            '-' | '.' | '0'..='9' | '\u{B7}' | '\u{300}'..='\u{36F}' | '\u{203F}'..='\u{2040}')
}

/// Whether `s` is an XML 1.0 `Nmtoken`: one or more `NameChar`s.
pub fn is_nmtoken(s: &str) -> bool {
    !s.is_empty() && s.chars().all(is_name_char)
}

#[cfg(test)]
mod tests {
    use super::*;

    // [spec:kbdgen:def:ldml.model.text/test]
    #[test]
    fn plain_text_drops_markers() {
        let mut t = Text::plain_from("a");
        t.push_marker(3);
        t.push_char('b');
        assert_eq!(t.plain(), "ab");
        assert_eq!(t.len(), 3);
        assert_eq!(t.markers().collect::<Vec<_>>(), [3]);
        assert!(!t.is_plain());
        assert!(Text::from("\u{E000}").is_plain());
    }

    #[test]
    fn nmtoken_accepts_names_rejects_blanks() {
        assert!(is_nmtoken("dk_00B4"));
        assert!(is_nmtoken("u-0061-0301"));
        assert!(is_nmtoken("1"));
        assert!(!is_nmtoken(""));
        assert!(!is_nmtoken("a b"));
        assert!(!is_nmtoken("a{"));
    }
}
