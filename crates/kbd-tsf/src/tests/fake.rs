//! An in-memory text context: a UTF-16 document with a selection and a
//! composition, standing in for a TSF context (`tsf.test.host`).

use crate::text::{Failed, Read, TextContext, decode_before};

#[derive(Debug, Clone, Default)]
pub struct Fake {
    pub units: Vec<u16>,
    pub anchor: usize,
    pub caret: usize,
    pub composition: Option<(usize, usize)>,
    pub unreadable: bool,
    /// How many events `SendInput` accepts; all when `None`.
    pub inject_limit: Option<usize>,
    pub injected: Vec<(usize, String)>,
}

impl Fake {
    pub fn with_text(text: &str) -> Fake {
        let units: Vec<u16> = text.encode_utf16().collect();
        let end = units.len();
        Fake {
            units,
            anchor: end,
            caret: end,
            ..Fake::default()
        }
    }

    /// The whole document, composition included.
    pub fn text(&self) -> String {
        String::from_utf16(&self.units).unwrap()
    }

    pub fn preedit(&self) -> String {
        self.composition
            .map(|(s, e)| String::from_utf16(&self.units[s..e]).unwrap())
            .unwrap_or_default()
    }

    pub fn select(&mut self, anchor: usize, caret: usize) {
        self.anchor = anchor;
        self.caret = caret;
    }

    fn selection(&self) -> (usize, usize) {
        (self.anchor.min(self.caret), self.anchor.max(self.caret))
    }

    fn point(&self) -> usize {
        self.composition.map_or(self.selection().0, |(s, _)| s)
    }

    fn splice(&mut self, start: usize, end: usize, text: &str) -> usize {
        let new: Vec<u16> = text.encode_utf16().collect();
        let len = new.len();
        self.units.splice(start..end, new);
        if let Some((s, e)) = self.composition
            && s >= end
        {
            let shift = |p: usize| p + len - (end - start);
            self.composition = Some((shift(s), shift(e)));
        }
        start + len
    }

    fn collapse(&mut self, at: usize) {
        self.anchor = at;
        self.caret = at;
    }
}

impl TextContext for Fake {
    fn read_before(&mut self, max: usize) -> Result<Read, Failed> {
        if self.unreadable {
            return Err(Failed);
        }
        let point = self.point();
        let requested = 2 * max;
        let start = point.saturating_sub(requested);
        Ok(decode_before(&self.units[start..point], requested, max))
    }

    fn selection_empty(&mut self) -> bool {
        self.anchor == self.caret
    }

    fn replace(&mut self, units: usize, text: &str) -> Result<(), Failed> {
        let (start, end) = match self.composition {
            Some((s, _)) => (s, s),
            None => self.selection(),
        };
        let from = start.checked_sub(units).ok_or(Failed)?;
        let at = self.splice(from, end, text);
        if self.composition.is_none() {
            self.collapse(at);
        }
        Ok(())
    }

    fn set_preedit(&mut self, preedit: &str) -> Result<(), Failed> {
        let (start, end) = self
            .composition
            .unwrap_or((self.selection().0, self.selection().0));
        self.composition = None;
        let at = self.splice(start, end, preedit);
        if !preedit.is_empty() {
            self.composition = Some((start, at));
        }
        self.collapse(at);
        Ok(())
    }

    fn commit_preedit(&mut self) -> Result<(), Failed> {
        if let Some((_, end)) = self.composition.take() {
            self.collapse(end);
        }
        Ok(())
    }

    fn insert(&mut self, text: &str) -> Result<(), Failed> {
        let (start, end) = self.selection();
        let at = self.splice(start, end, text);
        self.collapse(at);
        Ok(())
    }

    fn inject(&mut self, backspaces: usize, text: &str) -> Result<(), Failed> {
        let events = 2 * (backspaces + text.encode_utf16().count());
        if self.inject_limit.is_some_and(|limit| limit < events) {
            return Err(Failed);
        }
        self.injected.push((backspaces, text.to_owned()));
        for _ in 0..backspaces {
            let before = String::from_utf16(&self.units[..self.caret]).unwrap();
            let Some(last) = before.chars().last() else {
                break;
            };
            let at = self.caret - last.len_utf16();
            self.splice(at, self.caret, "");
            self.collapse(at);
        }
        self.insert(text)
    }
}
