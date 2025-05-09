use std::{
    path::{Path, PathBuf},
    str::FromStr,
};

use anyhow::Result;
use async_trait::async_trait;
use xmlem::Document;

use crate::{build::BuildStep, bundle::KbdgenBundle};

fn generate_distribution_xml(
    work_dir: &Path,
    bundle_name: &str,
    bundle_id: &str,
) -> Result<PathBuf, std::io::Error> {
    let mut doc = Document::from_str(
        r#"
        <?xml version="1.1" encoding="UTF-8"?>
        <installer-gui-script minSpecVersion="2" />
    "#,
    )
    .unwrap();

    let dist_path = work_dir.join("distribution.xml");

    let root = doc.root();
    let title = root.append_new_element(&mut doc, "title");
    title.append_text(&mut doc, bundle_name);
    let _options = root.append_new_element(
        &mut doc,
        (
            "options",
            [("customize", "never"), ("rootVolumeOnly", "true")],
        ),
    );
    let choices_outline = root.append_new_element(&mut doc, "choices-outline");
    let line = choices_outline.append_new_element(&mut doc, ("line", [("choice", "default")]));
    line.append_new_element(&mut doc, ("line", [("choice", bundle_id.to_string())]));

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

    pkg_ref.append_text(&mut doc, "inner.pkg");

    let out = doc.to_string_pretty();
    tracing::trace!("dist.xml: {:#}", &out);
    std::fs::write(&dist_path, &out)?;

    Ok(dist_path)
}

fn create_component_pkg(working_path: &Path, bundle_path: &Path, version: &str) -> PathBuf {
    let pkg_path = working_path.join(format!("inner.pkg"));
    std::process::Command::new("pkgbuild")
        .arg("--component")
        .arg(bundle_path)
        .args(&[
            "--ownership",
            "recommended",
            "--install-location",
            "/Library/Keyboard Layouts",
            "--version",
        ])
        .arg(version)
        .arg(&pkg_path)
        .status()
        .unwrap();

    pkg_path
}

pub struct GenerateInstaller;

#[async_trait(?Send)]
impl BuildStep for GenerateInstaller {
    async fn build(&self, bundle: &KbdgenBundle, output_path: &Path) -> Result<()> {
        create_installer(bundle, output_path);
        Ok(())
    }
}

fn run_productbuild(
    working_path: &Path,
    output_path: &Path,
    dist_xml_path: &Path,
    bundle_id: &str,
    version: &str,
) {
    let output = std::process::Command::new("productbuild")
        .arg("--distribution")
        .arg(dist_xml_path)
        .args(&["--version", version, "--package-path"])
        .arg(working_path)
        .arg(output_path.join(format!("{bundle_id}.pkg")))
        .status()
        .unwrap();

    tracing::debug!("{:?}", output);
}

fn create_installer(bundle: &KbdgenBundle, output_path: &Path) {
    tracing::info!("Creating installer at {:?}...", output_path);

    let working_path = tempfile::tempdir().unwrap();
    let working_path = working_path.path();

    let target = bundle.targets.macos.as_ref().unwrap();
    let version = &*target.version;

    let bundle_id = format!("{}.keyboardlayout.{}", target.package_id, bundle.name());
    let _inner_pkg_path = create_component_pkg(
        working_path,
        &output_path.join(format!("{bundle_id}.bundle")),
        version,
    );

    let dist_xml =
        generate_distribution_xml(working_path, &target.bundle_name, &bundle_id).unwrap();

    tracing::info!("Running 'productbuild'...");
    run_productbuild(working_path, output_path, &dist_xml, &bundle_id, version);
}
