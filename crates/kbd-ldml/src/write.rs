//! Serializing source documents (`ldml.xml.write`).

use xmlem::Document;
use xmlem::display::Config;

// [spec:kbdgen:thm:ldml.xml.roundtrip]
// [spec:kbdgen:req:ldml.xml.write]
/// Serializes a document with `xmlem`'s default pretty printing: two-space
/// indent, lines up to 120 characters, standard entities. Line breaks are
/// LF and the text ends with a newline. The document is never sorted, so
/// elements and attributes keep their order, and the same document always
/// gives the same bytes.
pub fn write(document: &Document) -> String {
    let mut out = document
        .to_string_pretty_with_config(&Config::default_pretty())
        .replace("\r\n", "\n");
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}
