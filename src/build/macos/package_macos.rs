use std::{
    path::{Path, PathBuf},
    str::FromStr,
    sync::Arc,
};

use async_trait::async_trait;
use xmlem::{Document, NewElement};

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
    let title = root.append_new_element(
        &mut doc,
        NewElement {
            name: "title".into(),
            attrs: [].into(),
        },
    );
    title.append_text(&mut doc, bundle_name);
    let _options = root.append_new_element(
        &mut doc,
        NewElement {
            name: "options".into(),
            attrs: [
                ("customize".to_string(), "never".to_string()),
                ("rootVolumeOnly".to_string(), "true".to_string()),
            ]
            .into(),
        },
    );
    let choices_outline = root.append_new_element(
        &mut doc,
        NewElement {
            name: "choices-outline".into(),
            attrs: [].into(),
        },
    );
    let line = choices_outline.append_new_element(
        &mut doc,
        NewElement {
            name: "line".into(),
            attrs: [("choice".to_string(), "default".to_string())].into(),
        },
    );
    line.append_new_element(
        &mut doc,
        NewElement {
            name: "line".into(),
            attrs: [("choice".to_string(), bundle_id.to_string())].into(),
        },
    );

    root.append_new_element(
        &mut doc,
        NewElement {
            name: "choice".into(),
            attrs: [("id".to_string(), "default".to_string())].into(),
        },
    );

    let choice = root.append_new_element(
        &mut doc,
        NewElement {
            name: "choice".into(),
            attrs: [
                ("id".to_string(), bundle_id.to_string()),
                ("visible".to_string(), "false".to_string()),
            ]
            .into(),
        },
    );

    choice.append_new_element(
        &mut doc,
        NewElement {
            name: "pkg-ref".into(),
            attrs: [("id".to_string(), bundle_id.to_string())].into(),
        },
    );

    let pkg_ref = root.append_new_element(
        &mut doc,
        NewElement {
            name: "pkg-ref".into(),
            attrs: [
                ("id".into(), bundle_id.to_string()),
                ("version".into(), "0".into()),
                ("auth".into(), "root".into()),
                ("onConclusion".into(), "RequireRestart".into()),
            ]
            .into(),
        },
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

pub(crate) struct GenerateInstaller;

#[async_trait(?Send)]
impl BuildStep for GenerateInstaller {
    async fn build(&self, bundle: &KbdgenBundle, output_path: &Path) {
        create_installer(bundle, output_path);
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
