//! Migrates every v3 layout of the bundles named on the command line in
//! memory, writing nothing, and reports per layout: blocked or not, the
//! defect codes, the caps differences, whether the v4 text loads with
//! `yaml::load`, and how its Windows layout DLL compares with the v3 one.
//!
//! `cargo run --example migrate-survey -- <bundle>…`

use std::collections::BTreeMap;
use std::path::PathBuf;

use kbdgen::build::windows::kbdl::migrated::compare;
use kbdgen::bundle::read_kbdgen_bundle;
use kbdgen::ldml::migrate::migrate_bundle_layout;
use kbdgen::ldml::yaml::load;

// [spec:kbdgen:req:ldml.migrate.survey]
fn main() -> anyhow::Result<()> {
    let mut unexplained = 0;
    for bundle_path in std::env::args().skip(1).map(PathBuf::from) {
        let bundle = read_kbdgen_bundle(&bundle_path)?;
        println!("== {}", bundle_path.display());
        for (tag, layout) in bundle.layouts.iter() {
            let Some(migration) = migrate_bundle_layout(&bundle.path, tag.as_str())? else {
                continue;
            };
            let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
            for d in &migration.defects {
                *counts.entry(d.code.name()).or_default() += 1;
            }
            let codes: Vec<String> = counts.iter().map(|(c, n)| format!("{c}×{n}")).collect();
            println!(
                "{tag}: {} [{}]",
                if migration.blocked() { "BLOCKED" } else { "ok" },
                codes.join(" ")
            );
            for diff in &migration.caps_diffs {
                println!(
                    "  caps {} {}: {}",
                    diff.platform,
                    diff.state.name(),
                    diff.positions.join(" ")
                );
            }
            if migration.blocked() {
                for d in migration.defects.iter().filter(|d| d.code.blocks()) {
                    println!("  {d}");
                }
                continue;
            }
            if let Some(yaml) = &migration.yaml {
                let dir = tempfile::tempdir()?;
                let path = dir.path().join(format!("{tag}.yaml"));
                std::fs::write(&path, yaml)?;
                match load(&path, tag.as_str()) {
                    Ok(_) => println!("  yaml::load ok"),
                    Err(e) => {
                        unexplained += 1;
                        println!("  yaml::load FAILED: {e}");
                    }
                }
            }
            if layout.windows.is_some() {
                let comparison = compare(&bundle, tag)?;
                println!(
                    "  windows DLL: {} explained difference(s), {} unexplained",
                    comparison.explained.len(),
                    comparison.unexplained.len()
                );
                for line in &comparison.unexplained {
                    println!("    UNEXPLAINED {line}");
                }
                unexplained += comparison.unexplained.len();
            }
        }
    }
    println!("unexplained: {unexplained}");
    Ok(())
}
