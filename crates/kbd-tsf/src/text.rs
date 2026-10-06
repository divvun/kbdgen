//! The text operations an edit needs, as a trait with no TSF calls, so
//! that the edit logic runs against a fake context on every host
//! (`tsf.test.host`). The Windows build implements it inside an edit
//! session with `ITfRange`, `ITfComposition` and `SendInput`.

/// Text read before the caret.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Read {
    pub text: String,
    /// The read stopped short of the request: `text` begins at the start
    /// of the document.
    pub at_start: bool,
}

/// A text operation the context refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Failed;

/// A text context, positioned at the caret. While a preedit is shown, the
/// caret for reading and replacing is the start of the preedit.
pub trait TextContext {
    /// Up to `max` scalar values before the caret, or `Failed` when the
    /// context cannot be read.
    fn read_before(&mut self, max: usize) -> Result<Read, Failed>;

    fn selection_empty(&mut self) -> bool;

    /// Replaces the `units` UTF-16 units before the caret, and any
    /// selection, with `text`; the caret ends after it (`tsf.edit.apply`).
    fn replace(&mut self, units: usize, text: &str) -> Result<(), Failed>;

    /// Shows `preedit` after the caret as the composition, replacing the
    /// one shown; an empty preedit removes the composition and its text
    /// (`tsf.edit.preedit`).
    fn set_preedit(&mut self, preedit: &str) -> Result<(), Failed>;

    /// Ends the composition, leaving its text committed.
    fn commit_preedit(&mut self) -> Result<(), Failed>;

    /// Inserts `text` at the selection, replacing it.
    fn insert(&mut self, text: &str) -> Result<(), Failed>;

    /// Sends `backspaces` Backspace presses, then `text` as Unicode key
    /// presses, all in one `SendInput` call (`tsf.edit.inject`). `Failed` if
    /// fewer events were sent than given.
    fn inject(&mut self, backspaces: usize, text: &str) -> Result<(), Failed>;
}

/// The context a read gave, from the UTF-16 `units` that a shift back by
/// `requested` units returned, keeping at most `max` scalar values. A low
/// surrogate whose high half lies before the read is dropped, and a read
/// that kept everything up to the start of the document sets `at_start`.
// [spec:kbdgen:req:tsf.edit.session+1]
pub fn decode_before(units: &[u16], requested: usize, max: usize) -> Read {
    let split = matches!(units.first(), Some(0xDC00..=0xDFFF)) && units.len() >= requested;
    let units = if split {
        units.get(1..).unwrap_or_default()
    } else {
        units
    };
    let scalars: Vec<char> = char::decode_utf16(units.iter().copied())
        .map(|c| c.unwrap_or(char::REPLACEMENT_CHARACTER))
        .collect();
    let skip = scalars.len().saturating_sub(max);
    Read {
        text: scalars.iter().skip(skip).collect(),
        at_start: units.len() < requested && skip == 0 && !split,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utf16(s: &str) -> Vec<u16> {
        s.encode_utf16().collect()
    }

    // [spec:kbdgen:req:tsf.edit.session+1/test]
    #[test]
    fn short_read_is_at_start() {
        let read = decode_before(&utf16("ab"), 8, 4);
        assert_eq!(read.text, "ab");
        assert!(read.at_start);
    }

    // [spec:kbdgen:req:tsf.edit.session+1/test]
    #[test]
    fn full_read_keeps_last_scalars() {
        let read = decode_before(&utf16("abcdefgh"), 8, 4);
        assert_eq!(read.text, "efgh");
        assert!(!read.at_start);
        let read = decode_before(&utf16("abc"), 8, 2);
        assert_eq!(read.text, "bc");
        assert!(!read.at_start);
    }

    // [spec:kbdgen:req:tsf.edit.session+1/test]
    // [spec:kbdgen:thm:tsf.edit.units/test]
    #[test]
    fn split_surrogate_at_read_start_dropped() {
        let mut units = utf16("𝕫a");
        units.remove(0);
        let read = decode_before(&units, 2, 4);
        assert_eq!(read.text, "a");
        assert!(!read.at_start);
        let read = decode_before(&utf16("𝕫a"), 4, 4);
        assert_eq!(read.text, "𝕫a");
        assert!(read.at_start);
    }
}
