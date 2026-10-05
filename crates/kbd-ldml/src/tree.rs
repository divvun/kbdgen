//! A read-only view of a source document's elements for resolution: each
//! element with its local name, namespace prefix, attributes in source
//! order, child elements, and where it came from for messages.
//!
//! Resolution splices imports into this view, never into the source
//! document, so the document stays exactly what was read and what export
//! writes (`ldml.xml.document`).

use std::sync::Arc;

use xmlem::{Document, Element};

use crate::diag::{Diagnostic, Error, Result};

/// Deeper nesting than any keyboard3 document has is refused, which bounds
/// the recursion that builds and walks the view.
const MAX_DEPTH: usize = 32;

// [spec:kbdgen:def:ldml.xml.document]
/// An element of the view; `path` names it in messages.
#[derive(Debug, Clone)]
pub(crate) struct El {
    pub(crate) prefix: Option<String>,
    pub(crate) name: String,
    pub(crate) attrs: Vec<(String, String)>,
    pub(crate) children: Vec<El>,
    pub(crate) file: Arc<str>,
    pub(crate) path: String,
}

/// The identity used in element paths: `[id=…]` and the like, else the
/// 1-based position among same-named siblings.
fn path_segment(name: &str, attrs: &[(String, String)], position: usize) -> String {
    for key in [
        "id",
        "keyId",
        "output",
        "formId",
        "type",
        "modifiers",
        "from",
        "path",
    ] {
        if let Some((_, v)) = attrs.iter().find(|(k, _)| k == key) {
            return format!("{name}[{key}={v}]");
        }
    }
    if position > 1 {
        format!("{name}[{position}]")
    } else {
        name.to_string()
    }
}

impl El {
    pub(crate) fn from_document(file: &str, document: &Document) -> Result<El> {
        let file: Arc<str> = Arc::from(file);
        build(&file, document, document.root(), "", 1, 0)
    }

    pub(crate) fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }

    /// The child elements named `name` in the default namespace.
    pub(crate) fn children_named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a El> + 'a {
        self.children
            .iter()
            .filter(move |c| c.prefix.is_none() && c.name == name)
    }

    pub(crate) fn child<'a>(&'a self, name: &'a str) -> Option<&'a El> {
        self.children_named(name).next()
    }

    pub(crate) fn error(&self, message: impl Into<String>) -> Error {
        Diagnostic::new(&self.file, &self.path, message).into()
    }

    pub(crate) fn attr_error(&self, attribute: &str, message: impl Into<String>) -> Error {
        Diagnostic::new(&self.file, &self.path, message)
            .at(attribute)
            .into()
    }

    pub(crate) fn warning(&self, message: impl Into<String>) -> Diagnostic {
        Diagnostic::new(&self.file, &self.path, message)
    }

    /// The qualified name as written.
    pub(crate) fn qualified(&self) -> String {
        match &self.prefix {
            Some(p) => format!("{p}:{}", self.name),
            None => self.name.clone(),
        }
    }
}

fn bump(counts: &mut Vec<(String, usize)>, name: &str) -> usize {
    match counts.iter_mut().find(|(n, _)| n == name) {
        Some((_, count)) => {
            *count += 1;
            *count
        }
        None => {
            counts.push((name.to_string(), 1));
            1
        }
    }
}

fn build(
    file: &Arc<str>,
    document: &Document,
    element: Element,
    parent: &str,
    position: usize,
    depth: usize,
) -> Result<El> {
    let qname = element.qname(document);
    let name = qname.local_part().to_string();
    let prefix = qname.namespace().map(str::to_string);
    let attrs: Vec<(String, String)> = element
        .attributes(document)
        .iter()
        .map(|(k, v)| (k.prefixed_name().to_string(), v.clone()))
        .collect();
    let qualified = qname.prefixed_name().to_string();
    let segment = path_segment(&qualified, &attrs, position);
    let path = if parent.is_empty() {
        segment
    } else {
        format!("{parent}/{segment}")
    };
    if depth > MAX_DEPTH {
        return Err(Diagnostic::new(file, &path, "elements are nested too deeply").into());
    }
    let mut counts: Vec<(String, usize)> = Vec::new();
    let mut children = Vec::new();
    for child in element.children(document) {
        let n = bump(&mut counts, child.qname(document).prefixed_name());
        children.push(build(file, document, child, &path, n, depth + 1)?);
    }
    Ok(El {
        prefix,
        name,
        attrs,
        children,
        file: file.clone(),
        path,
    })
}
