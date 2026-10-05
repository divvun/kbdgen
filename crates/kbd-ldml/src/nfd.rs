//! NFD over ICU4X's compiled data, for the texts of keyboards whose
//! normalization is enabled (`ldml.model.nfd`).

use icu_normalizer::DecomposingNormalizerBorrowed;
use icu_normalizer::properties::{
    CanonicalCombiningClassMapBorrowed, CanonicalDecompositionBorrowed, Decomposed,
};
use kbd_model::{MarkerIndex, NfdCheck, Text, TextElem};

/// The NFD check that model validation needs.
#[derive(Debug, Clone, Copy, Default)]
pub struct IcuNfd;

impl NfdCheck for IcuNfd {
    fn is_nfd(&self, text: &str) -> bool {
        DecomposingNormalizerBorrowed::new_nfd().is_normalized(text)
    }

    /// A single scalar value is NFD exactly when it has no canonical
    /// decomposition.
    fn first_non_nfd(&self, lo: char, hi: char) -> Option<char> {
        let decomposition = CanonicalDecompositionBorrowed::new();
        (lo..=hi).find(|c| decomposition.decompose(*c) != Decomposed::Default)
    }
}

/// Whether the scalar value `c` is NFD on its own.
pub(crate) fn is_nfd_char(c: char) -> bool {
    CanonicalDecompositionBorrowed::new().decompose(c) == Decomposed::Default
}

pub(crate) fn nfd_str(s: &str) -> String {
    DecomposingNormalizerBorrowed::new_nfd()
        .normalize(s)
        .into_owned()
}

/// `text` in NFD by the marker algorithm of §Normalization and Markers:
/// each marker is glued to the scalar value after it, or to the end, and
/// travels with the first scalar value of that value's decomposition
/// through canonical ordering.
pub(crate) fn nfd_text(text: &Text) -> Text {
    let classes = CanonicalCombiningClassMapBorrowed::new();
    let decomposer = DecomposingNormalizerBorrowed::new_nfd();
    let mut glued: Vec<(u8, char, Vec<MarkerIndex>)> = Vec::new();
    let mut pending = Vec::new();
    for e in text.elements() {
        match e {
            TextElem::Marker(m) => pending.push(*m),
            TextElem::Char(c) => {
                let mut markers = Some(std::mem::take(&mut pending));
                let mut buf = [0u8; 4];
                for d in decomposer.normalize(c.encode_utf8(&mut buf)).chars() {
                    glued.push((classes.get_u8(d), d, markers.take().unwrap_or_default()));
                }
            }
        }
    }
    let mut start = 0;
    while start < glued.len() {
        let len = glued
            .get(start..)
            .map_or(0, |rest| rest.iter().take_while(|g| g.0 != 0).count());
        if len == 0 {
            start += 1;
            continue;
        }
        if let Some(run) = glued.get_mut(start..start + len) {
            run.sort_by_key(|g| g.0);
        }
        start += len;
    }
    let mut out = Text::new();
    for (_, c, markers) in glued {
        for m in markers {
            out.push_marker(m);
        }
        out.push_char(c);
    }
    for m in pending {
        out.push_marker(m);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use TextElem::{Char, Marker};

    #[test]
    fn marker_travels_with_its_glue_character() {
        let input = Text(vec![Char('e'), Char('\u{300}'), Marker(0), Char('\u{320}')]);
        assert_eq!(
            nfd_text(&input).0,
            [Char('e'), Marker(0), Char('\u{320}'), Char('\u{300}')]
        );
        let precomposed = Text(vec![Marker(1), Char('\u{E8}'), Marker(2)]);
        assert_eq!(
            nfd_text(&precomposed).0,
            [Marker(1), Char('e'), Char('\u{300}'), Marker(2)]
        );
    }

    #[test]
    fn icu_check_answers_nfd() {
        assert!(IcuNfd.is_nfd("e\u{301}"));
        assert!(!IcuNfd.is_nfd("\u{E9}"));
        assert_eq!(IcuNfd.first_non_nfd('a', 'z'), None);
        assert_eq!(IcuNfd.first_non_nfd('\u{BF}', '\u{FF}'), Some('\u{C0}'));
        assert!(!is_nfd_char('\u{9CB}'));
        assert_eq!(nfd_str("\u{E9}"), "e\u{301}");
    }
}
