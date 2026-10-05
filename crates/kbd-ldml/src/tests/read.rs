//! Reading, the lossy pre-scan and writing.

use xmlem::{Document, Element, Node};

use super::*;
use crate::{read_document, read_import, write};

fn read_err(bytes: &[u8]) -> String {
    read_keyboard("k.xml", None, bytes).unwrap_err().to_string()
}

fn warnings(xml: &str) -> Vec<String> {
    read_keyboard("k.xml", None, xml.as_bytes())
        .unwrap()
        .warnings
        .iter()
        .map(|w| w.message.clone())
        .collect()
}

const MINIMAL: &str =
    r#"<keyboard3 locale="sme" conformsTo="45"><info name="T"/><layers formId="iso"/></keyboard3>"#;

// [spec:kbdgen:req:ldml.xml.read/test]
#[test]
fn reads_utf8_with_or_without_bom() {
    let mut bom = vec![0xEF, 0xBB, 0xBF];
    bom.extend_from_slice(MINIMAL.as_bytes());
    let source = read_keyboard("k.xml", None, &bom).unwrap();
    assert_eq!(source.document.root().name(&source.document), "keyboard3");
    assert!(read_keyboard("k.xml", None, MINIMAL.as_bytes()).is_ok());
    assert!(read_err(&[0xFF, 0xFE, b'<', 0]).contains("UTF-16"));
    assert!(read_err(b"<keyboard3 locale=\"\xE9\"/>").contains("not UTF-8"));
    let latin = format!("<?xml version=\"1.0\" encoding=\"ISO-8859-1\"?>{MINIMAL}");
    assert!(read_err(latin.as_bytes()).contains("ISO-8859-1"));
}

// [spec:kbdgen:req:ldml.xml.read/test]
#[test]
fn malformed_input_names_file_and_position() {
    let dup =
        "<keyboard3 locale=\"a\"\n  conformsTo=\"45\" locale=\"b\"><info name=\"T\"/></keyboard3>";
    let err = read_keyboard("dup.xml", None, dup.as_bytes()).unwrap_err();
    assert_eq!(err.diagnostic().position, Some((2, 19)));
    assert!(err.to_string().starts_with("dup.xml:2:19"), "{err}");
    assert!(err.to_string().contains("duplicate attribute locale"));
    assert!(
        read_err(b"<keyboard3 locale=\"a\" conformsTo=\"45\"><info name=\"T\"/>")
            .contains("never closed")
    );
    assert!(read_err(b"<keyboard3><a></b></keyboard3>").contains("does not close"));
    assert!(read_err(b"<keyboard3/><keyboard3/>").contains("second root"));
    assert!(read_err(b"<keyboard3 locale=a/>").contains("not quoted"));
    assert!(read_err(b"<!-- only a comment -->").contains("no root"));
}

// [spec:kbdgen:req:ldml.xml.read/test]
#[test]
fn keyboard_root_needs_locale_and_conforms_to() {
    assert!(read_err(b"<keys/>").contains("keyboard3"));
    assert!(read_err(b"<keyboard3 conformsTo=\"45\"/>").contains("locale"));
    assert!(read_err(b"<keyboard3 locale=\"a\"/>").contains("conformsTo"));
    assert!(read_err(b"<keyboard3 locale=\"a\" conformsTo=\"44\"/>").contains("45 to 49"));
    assert!(
        read_keyboard(
            "k.xml",
            None,
            b"<keyboard3 locale=\"a\" conformsTo=\"49\"/>"
        )
        .is_ok()
    );
    let foreign = r#"<keyboard3 xmlns="urn:x" locale="a" conformsTo="45"/>"#;
    assert!(
        warnings(foreign)
            .iter()
            .any(|w| w.contains("not a keyboard3 namespace"))
    );
    let ours = r#"<keyboard3 xmlns="https://schemas.unicode.org/cldr/47/keyboard3" locale="a" conformsTo="45"/>"#;
    assert!(warnings(ours).is_empty());
    assert!(read_import("i.xml", None, b"<keys/>").is_ok());
    assert!(read_import("i.xml", None, b"<keyboard3/>").is_err());
}

// [spec:kbdgen:req:ldml.xml.lossy/test]
#[test]
fn warns_once_per_lossy_kind() {
    let xml = "<?xml version=\"1.0\"?>\n<?pi data?><keyboard3 locale='a' conformsTo=\"45\">\n  \
               <info name=\"&#x41;\tB\"/><keys></keys><?pi again?>\n  <displays></displays>\n</keyboard3>";
    let found = warnings(xml);
    assert_eq!(found.len(), 6, "{found:#?}");
    for needle in [
        "processing instructions",
        "whitespace-only text",
        "references",
        "single-quoted",
        "<a></a>",
        "raw tab",
    ] {
        assert_eq!(
            found.iter().filter(|w| w.contains(needle)).count(),
            1,
            "{needle}"
        );
    }
    assert!(warnings(MINIMAL).is_empty());
}

// [spec:kbdgen:req:ldml.xml.write/test]
#[test]
fn writes_pretty_lf_and_deterministic() {
    let xml = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<!--a\r\nb-->\r\n<keyboard3 locale=\"a\" conformsTo=\"45\"><info name=\"T\"/><keys><key id=\"x\" output=\"&lt;\"/></keys></keyboard3>";
    let source = read_keyboard("k.xml", None, xml.as_bytes()).unwrap();
    let out = write(&source.document);
    assert_eq!(
        out,
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!--a\nb-->\n<keyboard3 locale=\"a\" conformsTo=\"45\">\n  \
         <info name=\"T\" />\n  <keys>\n    <key id=\"x\" output=\"&lt;\" />\n  </keys>\n</keyboard3>\n"
    );
    assert!(!out.contains('\r'));
    assert_eq!(write(&source.document), out);
}

/// One node of a document for comparison: what `ldml.xml.roundtrip`
/// promises to keep.
#[derive(Debug, PartialEq)]
enum Shape {
    Element(String, Vec<(String, String)>, Vec<Shape>),
    Comment(String),
    CData(String),
    Text(String),
}

fn shape(doc: &Document, el: Element) -> Shape {
    let attrs = el
        .attributes(doc)
        .iter()
        .map(|(k, v)| (k.prefixed_name().to_string(), v.clone()))
        .collect();
    let children = el
        .child_nodes(doc)
        .iter()
        .filter_map(|n| match n {
            Node::Element(e) => Some(shape(doc, *e)),
            Node::Comment(c) => Some(Shape::Comment(c.as_str(doc).to_string())),
            Node::CDataSection(c) => Some(Shape::CData(c.as_str(doc).to_string())),
            Node::Text(t) => {
                let t = t.as_str(doc).trim();
                (!t.is_empty()).then(|| Shape::Text(t.to_string()))
            }
            _ => None,
        })
        .collect();
    Shape::Element(el.name(doc).to_string(), attrs, children)
}

// [spec:kbdgen:thm:ldml.xml.roundtrip/test]
#[test]
fn read_write_read_keeps_structure() {
    let mut files: Vec<PathBuf> = cldr_keyboards();
    for dir in ["cldr/48/keyboards/test", "cldr/48/keyboards/import"] {
        files.extend(
            std::fs::read_dir(testdata(dir))
                .unwrap()
                .map(|e| e.unwrap().path()),
        );
    }
    files.push(PathBuf::from("inline"));
    for path in files {
        let bytes = if path.as_os_str() == "inline" {
            b"<?xml version=\"1.0\" standalone=\"yes\"?><!DOCTYPE keyboard3 SYSTEM \"x.dtd\"><!--pre-->\
              <keyboard3 a=\"&amp; &lt;&gt;&quot;'\"><b><![CDATA[x<y]]></b><!--in-->text</keyboard3><!--post-->"
                .to_vec()
        } else {
            std::fs::read(&path).unwrap()
        };
        let d = read_document("d.xml", None, &bytes).unwrap().document;
        let written = write(&d);
        let again = read_document("again.xml", None, written.as_bytes())
            .unwrap()
            .document;
        assert_eq!(
            shape(&again, again.root()),
            shape(&d, d.root()),
            "{}",
            path.display()
        );
        assert_eq!(again.doctype(), d.doctype());
        let decl = |doc: &Document| {
            doc.declaration()
                .map(|x| (x.version.clone(), x.encoding.clone(), x.standalone.clone()))
        };
        assert_eq!(decl(&again), decl(&d));
        assert_eq!(
            write(&again),
            written,
            "{}: comments around the root survive",
            path.display()
        );
    }
}
