//! Unicode normalization with markers (`ldml.engine.normalization`).
//!
//! The marker algorithm of UTS #35 Part 7 §Normalization and Markers glues
//! each marker to the scalar value that follows it, or to the end. Here
//! each marker travels with the position of its glue character through
//! decomposition and canonical ordering, so markers keep their glue even
//! when the same character occurs twice.
//!
//! Without the `normalization` feature these functions are the identity.
//! That is sound because `Model` refuses every keyboard with normalization
//! `Enabled` in that build, and only such keyboards call them.

use alloc::string::String;
use alloc::vec::Vec;

use kbd_model::{MarkerIndex, TextElem};

use crate::api::OutputForm;

/// A scalar value with the markers glued to it.
pub(crate) struct Glued {
    pub(crate) c: char,
    pub(crate) markers: Vec<MarkerIndex>,
}

/// Splits `elems` into scalar values with their glued markers and the
/// markers glued to the end: the marker algorithm's removal step, without
/// decomposition.
pub(crate) fn unglue(elems: &[TextElem]) -> (Vec<Glued>, Vec<MarkerIndex>) {
    let mut glued = Vec::new();
    let mut pending = Vec::new();
    for e in elems {
        match e {
            TextElem::Marker(m) => pending.push(*m),
            TextElem::Char(c) => glued.push(Glued {
                c: *c,
                markers: core::mem::take(&mut pending),
            }),
        }
    }
    (glued, pending)
}

/// The marker algorithm's re-adding step: each scalar value preceded by its
/// glued markers, then the markers glued to the end.
pub(crate) fn reglue(glued: Vec<Glued>, end: Vec<MarkerIndex>) -> Vec<TextElem> {
    let mut out = Vec::new();
    for g in glued {
        out.extend(g.markers.into_iter().map(TextElem::Marker));
        out.push(TextElem::Char(g.c));
    }
    out.extend(end.into_iter().map(TextElem::Marker));
    out
}

#[cfg(feature = "normalization")]
mod icu {
    use alloc::vec::Vec;

    use icu_normalizer::DecomposingNormalizerBorrowed;
    use icu_normalizer::properties::{
        CanonicalCombiningClassMapBorrowed, CanonicalDecompositionBorrowed, Decomposed,
    };
    use kbd_model::NfdCheck;

    use super::Glued;

    /// Decomposes every scalar value, gluing its markers to the first
    /// scalar value of its decomposition, then puts each run of non-zero
    /// combining classes into canonical order with a stable sort. The
    /// result is NFD, because NFD is the canonical ordering of the
    /// concatenated full decompositions.
    pub(crate) fn nfd_glued(glued: Vec<Glued>) -> Vec<Glued> {
        let classes = CanonicalCombiningClassMapBorrowed::new();
        let decomposer = DecomposingNormalizerBorrowed::new_nfd();
        let mut out: Vec<(u8, Glued)> = Vec::with_capacity(glued.len());
        for g in glued {
            let mut markers = Some(g.markers);
            let mut buf = [0u8; 4];
            for c in decomposer.normalize(g.c.encode_utf8(&mut buf)).chars() {
                out.push((
                    classes.get_u8(c),
                    Glued {
                        c,
                        markers: markers.take().unwrap_or_default(),
                    },
                ));
            }
        }
        let mut start = 0;
        while start < out.len() {
            let len = out.get(start..).map_or(0, |rest| {
                rest.iter().take_while(|(ccc, _)| *ccc != 0).count()
            });
            if len == 0 {
                start = start.saturating_add(1);
                continue;
            }
            let end = start.saturating_add(len);
            if let Some(run) = out.get_mut(start..end) {
                run.sort_by_key(|(ccc, _)| *ccc);
            }
            start = end;
        }
        out.into_iter().map(|(_, g)| g).collect()
    }

    /// The NFD check that model validation needs, over ICU4X's compiled
    /// data.
    pub(crate) struct IcuNfd;

    impl NfdCheck for IcuNfd {
        fn is_nfd(&self, text: &str) -> bool {
            DecomposingNormalizerBorrowed::new_nfd().is_normalized(text)
        }

        /// A single scalar value is NFD exactly when it has no canonical
        /// decomposition, which one table lookup answers.
        fn first_non_nfd(&self, lo: char, hi: char) -> Option<char> {
            let decomposition = CanonicalDecompositionBorrowed::new();
            (lo..=hi).find(|c| decomposition.decompose(*c) != Decomposed::Default)
        }
    }
}

#[cfg(feature = "normalization")]
pub(crate) use icu::IcuNfd;

// [spec:kbdgen:sem:ldml.engine.normalization+1]
/// `elems` in NFD with the marker algorithm. The engine normalizes the
/// context (with [`nfd_chars`], as it has no markers until the state's are
/// restored), C after appending an output, and C after each transform
/// group, and only for keyboards with normalization `Enabled`; `Disabled`
/// keyboards keep exactly the scalar values of the model and the context.
pub(crate) fn nfd_elems(elems: &[TextElem]) -> Vec<TextElem> {
    let (glued, end) = unglue(elems);
    #[cfg(feature = "normalization")]
    let glued = icu::nfd_glued(glued);
    reglue(glued, end)
}

/// The scalar values of `chars` in NFD.
pub(crate) fn nfd_chars(chars: &[char]) -> Vec<char> {
    #[cfg(feature = "normalization")]
    {
        let s: String = chars.iter().collect();
        icu_normalizer::DecomposingNormalizerBorrowed::new_nfd()
            .normalize(&s)
            .chars()
            .collect()
    }
    #[cfg(not(feature = "normalization"))]
    {
        chars.to_vec()
    }
}

/// Whether `c` has canonical combining class 0, so that text can be split
/// before it without affecting normalization.
pub(crate) fn is_starter(c: char) -> bool {
    #[cfg(feature = "normalization")]
    {
        icu_normalizer::properties::CanonicalCombiningClassMapBorrowed::new().get_u8(c) == 0
    }
    #[cfg(not(feature = "normalization"))]
    {
        let _ = c;
        true
    }
}

/// `text` in the output form: NFC composes it, NFD keeps it, since the
/// engine's text is already NFD.
pub(crate) fn to_output_form(text: String, form: OutputForm) -> String {
    match form {
        #[cfg(feature = "normalization")]
        OutputForm::Nfc => icu_normalizer::ComposingNormalizerBorrowed::new_nfc()
            .normalize(&text)
            .into_owned(),
        _ => text,
    }
}

#[cfg(all(test, feature = "normalization"))]
mod tests {
    use super::*;
    use alloc::vec;
    use kbd_model::NfdCheck;
    use kbd_model::TextElem::{Char, Marker};

    // [spec:kbdgen:sem:ldml.engine.normalization+1/test]
    #[test]
    fn marker_glues_to_following_scalar() {
        // UTS #35 Part 7, Example 1b.
        let input = [Char('e'), Char('\u{300}'), Marker(0), Char('\u{320}')];
        assert_eq!(
            nfd_elems(&input),
            [Char('e'), Marker(0), Char('\u{320}'), Char('\u{300}')]
        );
    }

    // [spec:kbdgen:sem:ldml.engine.normalization+1/test]
    #[test]
    fn trailing_marker_stays_at_end() {
        // UTS #35 Part 7, Example 2.
        let input = [
            Char('e'),
            Marker(0),
            Char('\u{300}'),
            Marker(1),
            Char('\u{320}'),
            Marker(2),
        ];
        assert_eq!(
            nfd_elems(&input),
            [
                Char('e'),
                Marker(1),
                Char('\u{320}'),
                Marker(0),
                Char('\u{300}'),
                Marker(2)
            ]
        );
    }

    // [spec:kbdgen:sem:ldml.engine.normalization+1/test]
    #[test]
    fn markers_stay_in_their_segment() {
        // UTS #35 Part 7, Example 3.
        let input = [
            Char('e'),
            Char('\u{300}'),
            Marker(1),
            Char('\u{320}'),
            Char('a'),
            Char('\u{300}'),
            Marker(2),
            Char('\u{320}'),
        ];
        assert_eq!(
            nfd_elems(&input),
            [
                Char('e'),
                Marker(1),
                Char('\u{320}'),
                Char('\u{300}'),
                Char('a'),
                Marker(2),
                Char('\u{320}'),
                Char('\u{300}'),
            ]
        );
    }

    // [spec:kbdgen:sem:ldml.engine.normalization+1/test]
    #[test]
    fn precomposed_decomposes_with_marker_first() {
        let input = [Marker(3), Char('\u{E8}'), Char('\u{320}')];
        assert_eq!(
            nfd_elems(&input),
            [Marker(3), Char('e'), Char('\u{320}'), Char('\u{300}')]
        );
        assert_eq!(
            nfd_chars(&['\u{AC01}']),
            vec!['\u{1100}', '\u{1161}', '\u{11A8}']
        );
    }

    #[test]
    fn repeated_glue_character_keeps_markers() {
        // The marker glued to the first `a` survives a second `a`.
        let input = [Marker(0), Char('a'), Char('b'), Char('a')];
        assert_eq!(nfd_elems(&input), input);
    }

    #[test]
    fn icu_check_answers_nfd() {
        assert!(IcuNfd.is_nfd("e\u{301}"));
        assert!(!IcuNfd.is_nfd("\u{E9}"));
        assert_eq!(IcuNfd.first_non_nfd('a', 'z'), None);
        assert_eq!(IcuNfd.first_non_nfd('\u{BF}', '\u{FF}'), Some('\u{C0}'));
        assert_eq!(to_output_form("e\u{301}".into(), OutputForm::Nfc), "\u{E9}");
        assert_eq!(
            to_output_form("e\u{301}".into(), OutputForm::Nfd),
            "e\u{301}"
        );
        assert!(!is_starter('\u{301}'));
        assert!(is_starter('a'));
    }
}
