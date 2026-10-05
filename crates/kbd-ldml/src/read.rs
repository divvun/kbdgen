//! Reading source documents (`ldml.xml.read`) and reporting what `xmlem`
//! does not preserve (`ldml.xml.lossy`).

use std::path::{Path, PathBuf};
use std::str::FromStr;

use xmlem::Document;

use crate::diag::{Diagnostic, Error, Result};

/// The roots an import file may have (`ldml.xml.document`).
pub const IMPORT_ROOTS: [&str; 8] = [
    "displays",
    "flicks",
    "forms",
    "keys",
    "layers",
    "transformGroup",
    "transforms",
    "variables",
];

/// A source document: the tree `xmlem` read, the name used in messages,
/// the path local imports resolve against, and the reader's warnings.
#[derive(Debug, Clone)]
pub struct SourceDocument {
    pub name: String,
    pub path: Option<PathBuf>,
    pub document: Document,
    pub warnings: Vec<Diagnostic>,
}

impl SourceDocument {
    /// A generated document, which has no file and no warnings.
    pub fn generated(name: &str, document: Document) -> Self {
        SourceDocument {
            name: name.to_string(),
            path: None,
            document,
            warnings: Vec::new(),
        }
    }
}

/// A kind of input that `xmlem` does not preserve.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Lossy {
    ProcessingInstruction,
    WhitespaceText,
    Reference,
    SingleQuotes,
    EmptyPair,
    RawWhitespaceInAttribute,
}

impl Lossy {
    fn message(self) -> &'static str {
        match self {
            Lossy::ProcessingInstruction => {
                "processing instructions other than the XML declaration are dropped"
            }
            Lossy::WhitespaceText => {
                "whitespace-only text (indentation, blank lines) is not kept; output is re-indented"
            }
            Lossy::Reference => {
                "character and entity references are read as the characters they name"
            }
            Lossy::SingleQuotes => "single-quoted attributes are written with double quotes",
            Lossy::EmptyPair => "an element written <a></a> is written back as <a />",
            Lossy::RawWhitespaceInAttribute => {
                "a raw tab, CR or LF inside an attribute value is written as a character reference"
            }
        }
    }
}

/// What the pre-scan of the raw input finds.
#[derive(Debug, Default)]
pub(crate) struct Scan {
    pub(crate) lossy: Vec<Lossy>,
    pub(crate) comments: usize,
}

struct Scanner<'a> {
    text: &'a str,
    pos: usize,
    name: &'a str,
    scan: Scan,
}

fn position(text: &str, offset: usize) -> (usize, usize) {
    let before = text.get(..offset).unwrap_or(text);
    let line = before.matches('\n').count() + 1;
    let column = before.rsplit('\n').next().map_or(0, |l| l.chars().count()) + 1;
    (line, column)
}

impl<'a> Scanner<'a> {
    fn rest(&self) -> &'a str {
        self.text.get(self.pos..).unwrap_or("")
    }

    fn error(&self, offset: usize, message: impl Into<String>) -> Error {
        let mut d = Diagnostic::new(self.name, "", message);
        d.position = Some(position(self.text, offset));
        d.into()
    }

    fn note(&mut self, kind: Lossy) {
        if !self.scan.lossy.contains(&kind) {
            self.scan.lossy.push(kind);
        }
    }

    /// Skips to just past `end`, returning the text before it.
    fn skip_past(&mut self, end: &str, what: &str) -> Result<&'a str> {
        let start = self.pos;
        let Some(i) = self.rest().find(end) else {
            return Err(self.error(start, format!("unterminated {what}")));
        };
        let body = self.rest().get(..i).unwrap_or("");
        self.pos += i + end.len();
        Ok(body)
    }

    fn note_references(&mut self, s: &str, in_attribute: bool) {
        if s.contains("&#") || (!in_attribute && (s.contains("&quot;") || s.contains("&apos;"))) {
            self.note(Lossy::Reference);
        }
    }

    fn skip_ws(&mut self) {
        let n = self.rest().len() - self.rest().trim_start().len();
        self.pos += n;
    }

    fn take_name(&mut self) -> &'a str {
        let rest = self.rest();
        let end = rest
            .find(|c: char| c.is_whitespace() || matches!(c, '=' | '>' | '/' | '<'))
            .unwrap_or(rest.len());
        self.pos += end;
        rest.get(..end).unwrap_or("")
    }

    /// Scans a start tag after its `<`; returns its name and whether it
    /// closes itself.
    fn start_tag(&mut self) -> Result<(&'a str, bool)> {
        let start = self.pos.saturating_sub(1);
        let name = self.take_name();
        if name.is_empty() {
            return Err(self.error(start, "malformed start tag"));
        }
        let mut seen: Vec<&str> = Vec::new();
        loop {
            self.skip_ws();
            let rest = self.rest();
            if rest.starts_with("/>") {
                self.pos += 2;
                return Ok((name, true));
            }
            if rest.starts_with('>') {
                self.pos += 1;
                return Ok((name, false));
            }
            let at = self.pos;
            let attribute = self.take_name();
            if attribute.is_empty() {
                return Err(self.error(at, format!("malformed attribute in <{name}>")));
            }
            self.skip_ws();
            if !self.rest().starts_with('=') {
                return Err(self.error(at, format!("attribute {attribute} has no value")));
            }
            self.pos += 1;
            self.skip_ws();
            let quote = match self.rest().chars().next() {
                Some(q @ ('"' | '\'')) => q,
                _ => return Err(self.error(at, format!("attribute {attribute} is not quoted"))),
            };
            if quote == '\'' {
                self.note(Lossy::SingleQuotes);
            }
            self.pos += 1;
            let value = self.skip_past(if quote == '"' { "\"" } else { "'" }, "attribute value")?;
            if value.contains(['\t', '\r', '\n']) {
                self.note(Lossy::RawWhitespaceInAttribute);
            }
            self.note_references(value, true);
            if seen.contains(&attribute) {
                return Err(self.error(at, format!("duplicate attribute {attribute} on <{name}>")));
            }
            seen.push(attribute);
        }
    }

    /// Scans the whole input. Tracks open elements, so unbalanced or
    /// mismatched tags, which `xmlem` would accept at end of input, are
    /// errors.
    fn run(mut self) -> Result<Scan> {
        let mut open: Vec<(&str, bool, usize)> = Vec::new();
        let mut seen_root = false;
        while self.pos < self.text.len() {
            let at = self.pos;
            let rest = self.rest();
            if rest.starts_with("<!--") {
                self.pos += 4;
                self.skip_past("-->", "comment")?;
                self.scan.comments += 1;
                if let Some(top) = open.last_mut() {
                    top.1 = true;
                }
            } else if rest.starts_with("<![CDATA[") {
                self.pos += 9;
                self.skip_past("]]>", "CDATA section")?;
                if let Some(top) = open.last_mut() {
                    top.1 = true;
                }
            } else if rest.starts_with("<!") {
                self.skip_doctype(at)?;
            } else if rest.starts_with("<?") {
                self.pos += 2;
                let body = self.skip_past("?>", "processing instruction")?;
                let target = body.split_whitespace().next().unwrap_or("");
                if target != "xml" || at != 0 {
                    self.note(Lossy::ProcessingInstruction);
                } else {
                    self.check_encoding(body)?;
                }
            } else if rest.starts_with("</") {
                self.pos += 2;
                let name = self.take_name();
                self.skip_ws();
                if !self.rest().starts_with('>') {
                    return Err(self.error(at, "malformed end tag"));
                }
                self.pos += 1;
                match open.pop() {
                    Some((open_name, content, _)) if open_name == name => {
                        if !content {
                            self.note(Lossy::EmptyPair);
                        }
                    }
                    Some((open_name, _, _)) => {
                        return Err(
                            self.error(at, format!("</{name}> does not close <{open_name}>"))
                        );
                    }
                    None => return Err(self.error(at, format!("unexpected </{name}>"))),
                }
            } else if rest.starts_with('<') {
                self.pos += 1;
                if open.is_empty() && seen_root {
                    return Err(self.error(at, "a second root element"));
                }
                seen_root = true;
                if let Some(top) = open.last_mut() {
                    top.1 = true;
                }
                let (name, closed) = self.start_tag()?;
                if !closed {
                    open.push((name, false, at));
                }
            } else {
                let end = rest.find('<').unwrap_or(rest.len());
                let text = rest.get(..end).unwrap_or("");
                self.pos += end;
                if text.trim().is_empty() {
                    self.note(Lossy::WhitespaceText);
                } else {
                    if let Some(top) = open.last_mut() {
                        top.1 = true;
                    }
                    self.note_references(text, false);
                }
            }
        }
        if let Some((name, _, at)) = open.last() {
            return Err(self.error(*at, format!("<{name}> is never closed")));
        }
        if !seen_root {
            return Err(self.error(0, "no root element"));
        }
        Ok(self.scan)
    }

    fn skip_doctype(&mut self, at: usize) -> Result<()> {
        let mut depth = 0usize;
        let mut quote: Option<char> = None;
        for (i, c) in self.rest().char_indices() {
            match (quote, c) {
                (Some(q), c) if c == q => quote = None,
                (Some(_), _) => {}
                (None, '"' | '\'') => quote = Some(c),
                (None, '[') => depth += 1,
                (None, ']') => depth = depth.saturating_sub(1),
                (None, '>') if depth == 0 => {
                    self.pos += i + 1;
                    return Ok(());
                }
                _ => {}
            }
        }
        Err(self.error(at, "unterminated declaration"))
    }

    fn check_encoding(&self, declaration: &str) -> Result<()> {
        let Some(i) = declaration.find("encoding") else {
            return Ok(());
        };
        let value = declaration
            .get(i + "encoding".len()..)
            .unwrap_or("")
            .trim_start()
            .trim_start_matches('=')
            .trim_start();
        let value: String = value
            .chars()
            .skip(1)
            .take_while(|c| *c != '"' && *c != '\'')
            .collect();
        if value.eq_ignore_ascii_case("utf-8") || value.eq_ignore_ascii_case("utf8") {
            Ok(())
        } else {
            Err(self.error(0, format!("encoding {value} is not supported; use UTF-8")))
        }
    }
}

/// Pre-scans raw UTF-8 input whose byte-order mark is already removed.
pub(crate) fn prescan(name: &str, text: &str) -> Result<Scan> {
    Scanner {
        text,
        pos: 0,
        name,
        scan: Scan::default(),
    }
    .run()
}

/// Decodes raw input as UTF-8 and drops a leading byte-order mark.
fn utf8<'a>(name: &str, bytes: &'a [u8]) -> Result<&'a str> {
    if bytes.starts_with(&[0xFE, 0xFF]) || bytes.starts_with(&[0xFF, 0xFE]) {
        return Err(Diagnostic::new(name, "", "UTF-16 input is not supported; use UTF-8").into());
    }
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    std::str::from_utf8(bytes)
        .map_err(|e| Diagnostic::new(name, "", format!("input is not UTF-8: {e}")).into())
}

// [spec:kbdgen:req:ldml.xml.read]
// [spec:kbdgen:req:ldml.xml.lossy]
/// Reads a document with any root: UTF-8 only, a leading byte-order mark
/// skipped, through `Document::from_str`. The pre-scan rejects malformed
/// structure and duplicate attributes with a position, and warns once for
/// each kind of input that `xmlem` does not preserve.
pub fn read_document(name: &str, path: Option<&Path>, bytes: &[u8]) -> Result<SourceDocument> {
    let text = utf8(name, bytes)?;
    let scan = prescan(name, text)?;
    let document = Document::from_str(text)
        .map_err(|e| Error::from(Diagnostic::new(name, "", format!("malformed XML: {e}"))))?;
    let mut warnings: Vec<Diagnostic> = scan
        .lossy
        .iter()
        .map(|kind| Diagnostic::new(name, "", kind.message()))
        .collect();
    warnings.sort_by(|a, b| a.message.cmp(&b.message));
    Ok(SourceDocument {
        name: name.to_string(),
        path: path.map(Path::to_path_buf),
        document,
        warnings,
    })
}

/// The `xmlns` value of keyboard3 documents conforming to `version`.
pub fn keyboard_namespace(version: u8) -> String {
    format!("https://schemas.unicode.org/cldr/{version}/keyboard3")
}

/// Reads a keyboard: a document whose root is `keyboard3`, with `locale`
/// and a `conformsTo` from 45 to 49. An `xmlns` other than a keyboard3
/// namespace of CLDR 45 to 49 is a warning.
pub fn read_keyboard(name: &str, path: Option<&Path>, bytes: &[u8]) -> Result<SourceDocument> {
    let mut source = read_document(name, path, bytes)?;
    let root = source.document.root();
    let doc = &source.document;
    let root_name = root.name(doc);
    if root_name != "keyboard3" {
        return Err(
            Diagnostic::new(name, root_name, "the root of a keyboard must be keyboard3").into(),
        );
    }
    let attribute = |a: &str| {
        root.attributes(doc)
            .iter()
            .find(|(k, _)| k.prefixed_name() == a)
            .map(|(_, v)| v.clone())
    };
    if attribute("locale").is_none() {
        return Err(Diagnostic::new(name, "keyboard3", "locale is required")
            .at("locale")
            .into());
    }
    let conforms = attribute("conformsTo").ok_or_else(|| {
        Error::from(Diagnostic::new(name, "keyboard3", "conformsTo is required").at("conformsTo"))
    })?;
    match conforms.parse::<u8>() {
        Ok(45..=49) => {}
        _ => {
            return Err(Diagnostic::new(
                name,
                "keyboard3",
                format!("conformsTo {conforms:?} is not 45 to 49"),
            )
            .at("conformsTo")
            .into());
        }
    }
    if let Some(ns) = attribute("xmlns")
        && !(45..=49).any(|v| ns == keyboard_namespace(v))
    {
        source.warnings.push(
            Diagnostic::new(
                name,
                "keyboard3",
                format!("xmlns {ns:?} is not a keyboard3 namespace"),
            )
            .at("xmlns"),
        );
    }
    Ok(source)
}

/// Reads a keyboard file.
pub fn read_keyboard_file(path: &Path) -> Result<SourceDocument> {
    let name = path.display().to_string();
    let bytes = std::fs::read(path)
        .map_err(|e| Error::from(Diagnostic::new(&name, "", format!("cannot read: {e}"))))?;
    read_keyboard(&name, Some(path), &bytes)
}

/// Reads an import file: a document whose root is one of [`IMPORT_ROOTS`].
pub fn read_import(name: &str, path: Option<&Path>, bytes: &[u8]) -> Result<SourceDocument> {
    let source = read_document(name, path, bytes)?;
    let root = source.document.root().name(&source.document).to_string();
    if !IMPORT_ROOTS.contains(&root.as_str()) {
        return Err(Diagnostic::new(name, &root, "not an importable root element").into());
    }
    Ok(source)
}

/// The number of comments in raw input, for reports of what a conversion
/// drops.
pub fn count_comments(name: &str, bytes: &[u8]) -> Result<usize> {
    let text = utf8(name, bytes)?;
    Ok(prescan(name, text)?.comments)
}
