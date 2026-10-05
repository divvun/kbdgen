//! Generates, for each layout with Windows input, the `#![no_std]` Rust
//! crate of its keyboard layout DLL and the `.res` file it links.

use std::path::Path;

use anyhow::{Context, Result, bail};
use async_trait::async_trait;

use language_tags::LanguageTag;

use crate::{
    build::BuildStep,
    bundle::{
        KbdgenBundle,
        layout::{Layout, WindowsTarget},
    },
};

pub mod adapter;
pub mod build;
pub mod bundle;
pub mod diag;
pub mod image;
pub mod input;
pub mod resources;
pub mod source;
pub mod tables;

use diag::Diagnostics;
use input::{LayoutInput, Metadata};

/// Directory under the output directory that holds one crate per layout.
pub const CRATES_DIR: &str = "build";

/// Build output directories inside a crate, which regeneration keeps.
const KEPT_ENTRIES: [&str; 2] = ["target", "target-wow64"];

/// The files of one generated layout crate.
#[derive(Debug, Clone)]
pub struct GeneratedLayout {
    pub name: String,
    pub cargo_toml: String,
    pub lib_rs: String,
    pub res: Vec<u8>,
}

/// Rejects metadata that cannot name a crate, DLL or resource.
// [spec:kbdgen:req:kbdl.metadata]
fn check_metadata(metadata: &Metadata) -> Result<()> {
    let name = &metadata.name;
    let valid_name = name
        .chars()
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic())
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if !valid_name {
        bail!(
            "keyboard name {name:?} must start with an ASCII letter and contain only ASCII letters, digits, '-' and '_'"
        );
    }
    let strings = [
        ("description", Some(&metadata.description)),
        ("language name", Some(&metadata.language_name)),
        ("locale name", Some(&metadata.locale_name)),
        ("company", Some(&metadata.company)),
        ("copyright", Some(&metadata.copyright)),
        ("version", metadata.version.as_ref()),
        ("build", metadata.build.as_ref()),
    ];
    for (field, value) in strings {
        if value.is_some_and(|value| value.contains('\0')) {
            bail!("{name}: the {field} contains U+0000");
        }
    }
    Ok(())
}

/// Generates one layout's crate files from its input and, when it has
/// one, its encoded engine model. Nothing is written; an error means the
/// layout has no output.
pub fn generate(
    input: &LayoutInput,
    model: Option<&[u8]>,
    diag: &mut Diagnostics,
) -> Result<GeneratedLayout> {
    check_metadata(&input.metadata)?;
    let tables = tables::build(input, diag)?;
    let version = resources::file_version(&input.metadata, diag);
    Ok(GeneratedLayout {
        name: input.metadata.name.clone(),
        cargo_toml: source::cargo_toml(&input.metadata.name),
        lib_rs: source::lib_rs(&tables),
        res: resources::res_file(&input.metadata, version, model),
    })
}

/// A layout of the bundle with Windows output, of either format.
enum Source<'a> {
    V3(&'a LanguageTag, &'a Layout, &'a WindowsTarget),
    V4(&'a LanguageTag, &'a Path),
}

impl Source<'_> {
    fn tag(&self) -> &LanguageTag {
        match self {
            Source::V3(tag, ..) | Source::V4(tag, _) => tag,
        }
    }
}

/// Every layout that may have Windows output, in bundle layout order: v3
/// layouts with a `windows` section and all v4 layouts.
fn sources(bundle: &KbdgenBundle) -> Vec<Source<'_>> {
    let mut sources: Vec<Source> = bundle
        .layouts
        .iter()
        .filter_map(|(tag, layout)| Some(Source::V3(tag, layout, layout.windows.as_ref()?)))
        .chain(
            bundle
                .v4_layouts
                .iter()
                .map(|(tag, path)| Source::V4(tag, path)),
        )
        .collect();
    sources.sort_by(|a, b| a.tag().as_str().cmp(b.tag().as_str()));
    sources
}

/// The input of one v3 layout. Its engine model would come from migrating
/// it in memory, which this build cannot do, so the DLL has no model.
// [spec:kbdgen:req:ldml.kbdl.model-resource]
fn v3_layout(
    bundle: &KbdgenBundle,
    tag: &LanguageTag,
    layout: &Layout,
    target: &WindowsTarget,
) -> Result<GeneratedLayout> {
    let (input, mut diag) = bundle::layout_input(bundle, tag, layout, target)?;
    diag.warn(
        "v3 layout: no engine model is embedded, because in-memory migration (ldml.migrate) is unavailable; the text service stays inert for this layout",
    );
    generate(&input, None, &mut diag)
}

/// The generated crate of one layout, or `None` for a v4 layout without a
/// `windows` document.
// [spec:kbdgen:def:ldml.kbdl.adapter]
fn generate_source(bundle: &KbdgenBundle, source: &Source) -> Result<Option<GeneratedLayout>> {
    match source {
        Source::V3(tag, layout, target) => v3_layout(bundle, tag, layout, target).map(Some),
        Source::V4(tag, path) => {
            let Some(layout) = adapter::load(tag, path)? else {
                return Ok(None);
            };
            let mut adapted = adapter::adapt(bundle, &layout)?;
            generate(&adapted.input, Some(&adapted.model), &mut adapted.diag).map(Some)
        }
    }
}

/// Generates every layout of the bundle with Windows output, v3 and v4 side
/// by side in bundle layout order. Any fatal condition fails the whole
/// bundle.
// [spec:kbdgen:req:kbdl.metadata]
pub fn generate_bundle(bundle: &KbdgenBundle) -> Result<Vec<GeneratedLayout>> {
    let mut generated: Vec<GeneratedLayout> = Vec::new();
    for source in sources(bundle) {
        let layout = generate_source(bundle, &source)
            .with_context(|| format!("Windows layout for {}", source.tag()))?;
        let Some(layout) = layout else {
            continue;
        };
        if generated.iter().any(|other| other.name == layout.name) {
            bail!(
                "two Windows layouts are named {}; set the Windows id to tell them apart",
                layout.name
            );
        }
        generated.push(layout);
    }
    Ok(generated)
}

/// Writes each layout's crate to `<output>/build/<name>/`, removing stale
/// files but keeping cargo's build directories.
// [spec:kbdgen:def:kbdl.crate]
pub fn write(output_path: &Path, layouts: &[GeneratedLayout]) -> Result<()> {
    for layout in layouts {
        let dir = output_path.join(CRATES_DIR).join(&layout.name);
        std::fs::create_dir_all(&dir).with_context(|| format!("create {}", dir.display()))?;
        let res_name = format!("{}.res", layout.name);
        let files: [(&str, &[u8]); 3] = [
            ("Cargo.toml", layout.cargo_toml.as_bytes()),
            ("lib.rs", layout.lib_rs.as_bytes()),
            (&res_name, &layout.res),
        ];
        for entry in std::fs::read_dir(&dir).with_context(|| format!("read {}", dir.display()))? {
            let entry = entry?;
            let file_name = entry.file_name();
            let keep = KEPT_ENTRIES.iter().any(|kept| file_name == *kept)
                || files.iter().any(|(name, _)| file_name == *name);
            if keep {
                continue;
            }
            let path = entry.path();
            if entry.file_type()?.is_dir() {
                std::fs::remove_dir_all(&path)
            } else {
                std::fs::remove_file(&path)
            }
            .with_context(|| format!("remove stale {}", path.display()))?;
        }
        for (name, contents) in files {
            let path = dir.join(name);
            std::fs::write(&path, contents).with_context(|| format!("write {}", path.display()))?;
        }
    }
    Ok(())
}

/// The keyboard names of the bundle's Windows layouts, in the order
/// [`generate_bundle`] produces them.
pub fn layout_names(bundle: &KbdgenBundle) -> Result<Vec<String>> {
    let mut names = Vec::new();
    for source in sources(bundle) {
        match source {
            Source::V3(tag, _, target) => {
                names.push(bundle::keyboard_name(tag, bundle::config(target).0))
            }
            Source::V4(tag, path) => names.extend(
                adapter::layout_name(tag, path)
                    .with_context(|| format!("Windows layout for {tag}"))?,
            ),
        }
    }
    Ok(names)
}

/// The build step that generates every layout crate of the bundle.
pub struct GenerateKbdl;

#[async_trait(?Send)]
impl BuildStep for GenerateKbdl {
    async fn build(&self, bundle: &KbdgenBundle, output_path: &Path) -> Result<()> {
        let layouts = generate_bundle(bundle)?;
        write(output_path, &layouts)
    }
}

/// The build step that builds every crate [`GenerateKbdl`] wrote into
/// verified DLLs at `<out>/<variant>/<name>.dll`.
pub struct BuildKbdl;

#[async_trait(?Send)]
impl BuildStep for BuildKbdl {
    // [spec:kbdgen:req:kbdl.build]
    // [spec:kbdgen:req:kbdl.build.toolchain+1]
    async fn build(&self, bundle: &KbdgenBundle, output_path: &Path) -> Result<()> {
        let names = layout_names(bundle)?;
        if names.is_empty() {
            return Ok(());
        }
        build::check_toolchain(&output_path.join(CRATES_DIR))?;
        for name in names {
            build::build_layout(output_path, &name)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use indexmap::IndexMap;

    const KEYS: &str = "´ 1 2 3 4 5 6 7 8 9 0 + ' q w e r t y u i o p å ¨ a s d f g h j k l ø æ @ < z x c v b n m , . -";

    fn layout(tag: &str, id: Option<&str>) -> Layout {
        let config = id.map_or(String::new(), |id| format!("  config:\n    id: {id}\n"));
        let yaml = format!(
            "languageTag: {tag}\ndisplayNames:\n  {primary}: Sámi\nwindows:\n{config}  primary:\n    layers:\n      default: \"{KEYS}\"\n      shift: \"{shift}\"\n      alt: \"{alt}\"\n  deadKeys:\n    default: ['´', '¨']\ntransforms:\n  ´:\n    ' ': ´\n    a: á\n  ¨:\n    ' ': ¨\n    a: ä\n",
            primary = tag.split('-').next().unwrap(),
            shift = KEYS.to_uppercase(),
            alt = KEYS.replace("t ", "t\u{301} ")
        );
        serde_yaml::from_str(&yaml).unwrap()
    }

    fn bundle(layouts: Vec<Layout>) -> KbdgenBundle {
        let layouts: IndexMap<_, _> = layouts
            .into_iter()
            .map(|layout| (layout.language_tag.clone(), layout))
            .collect();
        KbdgenBundle::new_test("test".into(), layouts)
    }

    fn metadata(name: &str) -> Metadata {
        Metadata {
            name: name.into(),
            ..Default::default()
        }
    }

    // [spec:kbdgen:req:kbdl.metadata/test]
    #[test]
    fn keyboard_names_and_metadata_strings_are_validated() {
        for name in ["kbdse-FI", "kbd_x", "K1"] {
            check_metadata(&metadata(name)).unwrap();
        }
        for name in ["", "1kbd", "-kbd", "kbd x", "kbd.x", "kbdø", "kbd/x"] {
            assert!(check_metadata(&metadata(name)).is_err(), "{name:?}");
        }
        let mut with_nul = metadata("kbdx");
        with_nul.copyright = "(c)\0".into();
        let error = check_metadata(&with_nul).unwrap_err().to_string();
        assert!(error.contains("copyright"), "{error}");
        let mut with_nul = metadata("kbdx");
        with_nul.version = Some("1\0".into());
        assert!(check_metadata(&with_nul).is_err());
    }

    // [spec:kbdgen:req:kbdl.metadata/test]
    #[test]
    fn duplicate_keyboard_names_are_fatal() {
        let bundle = bundle(vec![layout("se-FI", Some("x")), layout("se-NO", Some("x"))]);
        let error = generate_bundle(&bundle).unwrap_err().to_string();
        assert!(error.contains("kbdx"), "{error}");
    }

    // [spec:kbdgen:syn:kbdl.source/test]
    // [spec:kbdgen:def:kbdl.crate/test]
    // [spec:kbdgen:req:kbdl.build/test]
    #[test]
    fn generation_is_deterministic_and_written_as_a_crate() {
        let bundle = bundle(vec![layout("se-NO", None), layout("se-FI", None)]);
        let first = generate_bundle(&bundle).unwrap();
        assert_eq!(
            layout_names(&bundle).unwrap(),
            first
                .iter()
                .map(|layout| layout.name.clone())
                .collect::<Vec<_>>(),
            "the build step builds exactly the generated crates"
        );
        let second = generate_bundle(&bundle).unwrap();
        let names: Vec<&str> = first.iter().map(|layout| layout.name.as_str()).collect();
        assert_eq!(names, vec!["kbdse-FI", "kbdse-NO"]);
        for (a, b) in first.iter().zip(&second) {
            assert_eq!(a.lib_rs, b.lib_rs);
            assert_eq!(a.res, b.res);
            assert_eq!(a.cargo_toml, b.cargo_toml);
            assert!(a.lib_rs.is_ascii());
        }
        assert!(
            first[0]
                .lib_rs
                .contains("static aLigature: [LIGATURE<2>; 2] = [")
        );

        let out = tempfile::tempdir().unwrap();
        let crate_dir = out.path().join("build").join("kbdse-FI");
        std::fs::create_dir_all(crate_dir.join("target").join("keep")).unwrap();
        std::fs::create_dir_all(crate_dir.join(".cargo")).unwrap();
        std::fs::write(crate_dir.join("stale.def"), "x").unwrap();
        write(out.path(), &first).unwrap();
        let mut entries: Vec<String> = std::fs::read_dir(&crate_dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        entries.sort();
        assert_eq!(
            entries,
            vec!["Cargo.toml", "kbdse-FI.res", "lib.rs", "target"]
        );
        assert!(crate_dir.join("target").join("keep").is_dir());
        assert_eq!(
            std::fs::read(crate_dir.join("lib.rs")).unwrap(),
            first[0].lib_rs.as_bytes()
        );
        assert!(
            out.path()
                .join("build")
                .join("kbdse-NO")
                .join("kbdse-NO.res")
                .is_file()
        );
    }
}
