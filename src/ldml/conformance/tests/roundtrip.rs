//! Round trips (`ldml.test.roundtrip`) on every vendored CLDR keyboard and
//! every export of the fixture bundle's layouts and keyboards: XML to XML,
//! model to XML, YAML to XML and back, and the binary model.

use kbd_ldml::{SourceDocument, read_keyboard, read_keyboard_file, resolve, write};
use kbd_model::{Host, Keyboard};

use super::*;
use crate::ldml::import::group;
use crate::ldml::yaml::import::import;
use crate::ldml::yaml::{load, lower};

/// The fixture documents: each host document of the golden layouts as
/// lowering writes it, the golden XML keyboards and the vendored CLDR
/// keyboards, each with a name for messages.
fn documents() -> Vec<(String, SourceDocument)> {
    let mut out = Vec::new();
    for path in golden_files("layouts", "yaml") {
        let tag = path.file_stem().unwrap().to_string_lossy().into_owned();
        for (host, source) in lower(&load(&path, &tag).unwrap()).unwrap() {
            out.push((format!("{tag}.{}.xml", host.name()), source));
        }
    }
    let cldr = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("crates/kbd-ldml/testdata/cldr/48/keyboards/3.0");
    let mut xml = golden_files("keyboards", "xml");
    let mut vendored: Vec<PathBuf> = std::fs::read_dir(cldr)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    vendored.sort();
    xml.extend(vendored);
    for path in xml {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        out.push((name, read_keyboard_file(&path).unwrap()));
    }
    out
}

/// The resolved keyboard of `source`; resolution errors name the file.
fn resolved_keyboard(source: &SourceDocument) -> Keyboard {
    match resolve(source) {
        Ok(resolved) => resolved.keyboard,
        Err(e) => panic!("{}: {e}", source.name),
    }
}

// [spec:kbdgen:req:ldml.test.roundtrip/test]
// [spec:kbdgen:thm:ldml.xml.roundtrip/test]
#[test]
fn xml_write_read_keeps_model_and_bytes() {
    for (name, source) in documents() {
        let written = write(&source.document);
        let again = read_keyboard(&name, None, written.as_bytes()).unwrap();
        assert_eq!(
            resolved_keyboard(&again),
            resolved_keyboard(&source),
            "{name}"
        );
        assert_eq!(write(&again.document), written, "{name}: second write");
    }
}

// [spec:kbdgen:req:ldml.test.roundtrip/test]
// [spec:kbdgen:thm:ldml.xml.superset-roundtrip/test]
#[test]
fn exported_models_resolve_to_themselves() {
    for (name, source) in documents() {
        let resolved = resolve(&source).unwrap();
        let xml = write(&kbd_ldml::export(&resolved.keyboard, &resolved.extensions));
        let again = resolve(&read_keyboard(&name, None, xml.as_bytes()).unwrap())
            .unwrap_or_else(|e| panic!("{name}: {e}\n{xml}"));
        assert_eq!(again.keyboard, resolved.keyboard, "{name}\n{xml}");
        assert_eq!(again.extensions, resolved.extensions, "{name}");
        let twice = write(&kbd_ldml::export(&again.keyboard, &again.extensions));
        assert_eq!(twice, xml, "{name}: second export");
    }
}

// [spec:kbdgen:req:ldml.test.roundtrip/test]
#[test]
fn encoded_models_decode_to_themselves() {
    for (name, source) in documents() {
        let mut keyboard = resolved_keyboard(&source);
        keyboard.host.get_or_insert(Host::Web);
        let bytes = keyboard.to_bytes().unwrap();
        let model = Model::from_bytes(&bytes).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(model.keyboard(), &keyboard, "{name}");
        assert_eq!(
            model.keyboard().to_bytes().unwrap(),
            bytes,
            "{name}: second encoding"
        );
    }
}

/// The models of the YAML `import` writes for the XML files at `paths`,
/// one group, by host; the default variant's documents have no host.
fn imported_models(dir: &Path, paths: &[PathBuf]) -> (String, Vec<(Host, Keyboard)>) {
    let groups = group(paths).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(groups.len(), 1);
    let g = &groups[0];
    let dest = dir.join(format!("{}.yaml", g.tag));
    let imported = import(&g.tag, &dest, &g.files).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(&dest, &imported.yaml).unwrap();
    let layout = load(&dest, &g.tag).unwrap_or_else(|e| panic!("{e}\n{}", imported.yaml));
    let models = lower(&layout)
        .unwrap()
        .into_iter()
        .map(|(host, source)| (host, resolved_keyboard(&source)))
        .collect();
    (imported.yaml, models)
}

// [spec:kbdgen:req:ldml.test.roundtrip/test]
// [spec:kbdgen:thm:ldml.yaml.roundtrip/test]
#[test]
fn layouts_survive_export_and_import() {
    for path in golden_files("layouts", "yaml") {
        let tag = path.file_stem().unwrap().to_string_lossy().into_owned();
        let documents = lower(&load(&path, &tag).unwrap()).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let paths: Vec<PathBuf> = documents
            .iter()
            .map(|(host, source)| {
                let p = dir.path().join(format!("{tag}.{}.xml", host.name()));
                std::fs::write(&p, write(&source.document)).unwrap();
                p
            })
            .collect();
        let (yaml, models) = imported_models(dir.path(), &paths);
        let original: Vec<(Host, Keyboard)> = documents
            .iter()
            .map(|(host, source)| (*host, resolved_keyboard(source)))
            .collect();
        assert_eq!(models, original, "{tag}\n{yaml}");
    }
}

// [spec:kbdgen:req:ldml.test.roundtrip/test]
// [spec:kbdgen:thm:ldml.yaml.roundtrip/test]
#[test]
fn xml_keyboards_survive_import_and_export() {
    for path in golden_files("keyboards", "xml") {
        let dir = tempfile::tempdir().unwrap();
        let original = resolved_keyboard(&read_keyboard_file(&path).unwrap());
        let (yaml, models) = imported_models(dir.path(), std::slice::from_ref(&path));
        let (_, mut web) = models
            .into_iter()
            .find(|(h, _)| *h == Host::Web)
            .unwrap_or_else(|| panic!("{}: no web document\n{yaml}", path.display()));
        web.host = None;
        assert_eq!(web, original, "{}\n{yaml}", path.display());
    }
}
