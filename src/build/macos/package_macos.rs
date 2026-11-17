use std::path::Path;
use std::process::Command;
use std::str::FromStr;

use anyhow::{Context, Result, bail};
use async_trait::async_trait;
use xmlem::Document;

use super::macos_bundle::{bundle_id, macos_target};
use super::util::run;
use crate::{build::BuildStep, bundle::KbdgenBundle};

/// The component package's file name, which `distribution.xml` refers to.
const INNER_PKG: &str = "inner.pkg";

// [spec:kbdgen:req:macbundle.installer+1]
fn distribution_xml(bundle_name: &str, bundle_id: &str) -> String {
    let mut doc = Document::from_str(
        r#"
        <?xml version="1.1" encoding="UTF-8"?>
        <installer-gui-script minSpecVersion="2" />
    "#,
    )
    .expect("the distribution template parses");

    let root = doc.root();
    let title = root.append_new_element(&mut doc, "title");
    title.append_text(&mut doc, bundle_name);
    root.append_new_element(
        &mut doc,
        (
            "options",
            [("customize", "never"), ("rootVolumeOnly", "true")],
        ),
    );
    let choices_outline = root.append_new_element(&mut doc, "choices-outline");
    let line = choices_outline.append_new_element(&mut doc, ("line", [("choice", "default")]));
    line.append_new_element(&mut doc, ("line", [("choice", bundle_id)]));

    root.append_new_element(&mut doc, ("choice", [("id", "default")]));

    let choice = root.append_new_element(
        &mut doc,
        ("choice", [("id", bundle_id), ("visible", "false")]),
    );

    choice.append_new_element(&mut doc, ("pkg-ref", [("id", bundle_id)]));

    let pkg_ref = root.append_new_element(
        &mut doc,
        (
            "pkg-ref",
            [
                ("id", bundle_id),
                ("version", "0"),
                ("auth", "root"),
                ("onConclusion", "RequireRestart"),
            ],
        ),
    );

    pkg_ref.append_text(&mut doc, INNER_PKG);

    doc.to_string_pretty()
}

// [spec:kbdgen:req:macbundle.installer+1]
fn pkgbuild(bundle_path: &Path, version: &str, pkg_path: &Path) -> Result<()> {
    tracing::info!("Packaging {} with pkgbuild", bundle_path.display());
    run(Command::new("pkgbuild")
        .arg("--component")
        .arg(bundle_path)
        .args([
            "--ownership",
            "recommended",
            "--install-location",
            "/Library/Keyboard Layouts",
            "--version",
        ])
        .arg(version)
        .arg(pkg_path))
}

// [spec:kbdgen:req:macbundle.installer+1]
fn productbuild(
    dist_xml_path: &Path,
    version: &str,
    package_path: &Path,
    pkg_path: &Path,
) -> Result<()> {
    tracing::info!("Building {} with productbuild", pkg_path.display());
    run(Command::new("productbuild")
        .arg("--distribution")
        .arg(dist_xml_path)
        .args(["--version", version, "--package-path"])
        .arg(package_path)
        .arg(pkg_path))
}

pub struct GenerateInstaller;

#[async_trait(?Send)]
impl BuildStep for GenerateInstaller {
    // [spec:kbdgen:req:macbundle.installer+1]
    async fn build(&self, bundle: &KbdgenBundle, output_path: &Path) -> Result<()> {
        let target = macos_target(bundle)?;
        let version = target.version.as_str();
        let bundle_id = bundle_id(target, bundle);
        let bundle_path = output_path.join(format!("{bundle_id}.bundle"));
        if !bundle_path.is_dir() {
            bail!(
                "{} does not exist; run the macOS generate step first",
                bundle_path.display()
            );
        }

        let working_dir = tempfile::tempdir().context("create a directory for the installer")?;
        let working_path = working_dir.path();
        pkgbuild(&bundle_path, version, &working_path.join(INNER_PKG))?;

        let dist_xml = distribution_xml(&target.bundle_name, &bundle_id);
        tracing::trace!("distribution.xml:\n{dist_xml}");
        let dist_xml_path = working_path.join("distribution.xml");
        std::fs::write(&dist_xml_path, dist_xml)
            .with_context(|| format!("write {}", dist_xml_path.display()))?;

        productbuild(
            &dist_xml_path,
            version,
            working_path,
            &output_path.join(format!("{bundle_id}.pkg")),
        )
    }
}
