//! The fixtures built into DLLs for every variant, their `KBDTABLES`
//! decoded and compared with a golden file. The DLL bytes themselves are
//! not golden: with `/Brepro` they are reproducible for one toolchain, but
//! code generation and section layout change with the Rust release, while
//! the decoded tables do not.

use crate::build::BuildStep;

use super::super::{
    BuildKbdl, CRATES_DIR, GenerateKbdl,
    build::{VARIANTS, output_dll},
    differential::fixture_bundle,
    image,
};
use super::{FIXTURES, WORKSPACE, generated, golden_text, kbdtables};

/// Needs the three `*-pc-windows-msvc` targets (`kbdl.build.toolchain`).
// [spec:kbdgen:req:kbdl.build/test/golden]
// [spec:kbdgen:req:kbdl.image/test/golden]
// [spec:kbdgen:req:kbdl.wow64/test/golden]
// [spec:kbdgen:thm:kbdl.structs.layout/test/golden]
// [spec:kbdgen:req:kbdl.structs.pointers/test/golden]
// [spec:kbdgen:req:kbdl.export/test/golden]
// [spec:kbdgen:def:kbdl.tables/test/golden]
// [spec:kbdgen:def:kbdl.crate/test/golden]
#[tokio::test]
#[ignore = "builds DLLs with the Windows targets of the Rust toolchain"]
async fn fixture_dlls_hold_their_golden_tables() {
    for fixture in &FIXTURES {
        let (layout, _) = generated(fixture);
        let path = std::path::Path::new(WORKSPACE).join(fixture.path);
        let yaml = std::fs::read_to_string(&path).unwrap();
        let bundle = fixture_bundle(fixture.path, fixture.tag, &yaml);
        let out = tempfile::tempdir().unwrap();
        GenerateKbdl
            .build(&bundle.bundle, out.path())
            .await
            .unwrap();
        let crate_dir = out.path().join(CRATES_DIR).join(&layout.name);
        let res = format!("{}.res", layout.name);
        for (name, generated) in [
            ("Cargo.toml", layout.cargo_toml.as_bytes()),
            ("lib.rs", layout.lib_rs.as_bytes()),
            (res.as_str(), layout.res.as_slice()),
        ] {
            let written = std::fs::read(crate_dir.join(name)).unwrap();
            assert!(
                written == generated,
                "{}: {name} is written as generated",
                fixture.dir
            );
        }
        BuildKbdl.build(&bundle.bundle, out.path()).await.unwrap();
        let mut decoded = Vec::new();
        for variant in &VARIANTS {
            let bytes = std::fs::read(output_dll(out.path(), &layout.name, variant)).unwrap();
            image::verify(&bytes, variant.machine).unwrap();
            let wide = variant.wow64 || variant.machine != 0x014c;
            let tables = kbdtables::decode(&bytes, wide)
                .unwrap_or_else(|e| panic!("{} ({}): {e:#}", fixture.dir, variant.name));
            decoded.push((variant.name, tables));
        }
        for (name, tables) in &decoded[1..] {
            assert!(
                *tables == decoded[0].1,
                "{} ({name}) decodes differently from {}:\n{}",
                fixture.dir,
                decoded[0].0,
                super::line_diff(&decoded[0].1, tables)
            );
        }
        golden_text(fixture, "kbdtables.txt", &decoded[0].1);
    }
}
