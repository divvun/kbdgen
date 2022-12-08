use std::{path::Path, sync::Arc};

use anyhow::Result;
use async_trait::async_trait;

use crate::build::pahkat::{install_msklc, prefix_dir};

use crate::{build::BuildStep, bundle::KbdgenBundle};

pub struct BuildKlc {}

#[async_trait(?Send)]
impl BuildStep for BuildKlc {
    async fn build(&self, _bundle: &KbdgenBundle, output_path: &Path) -> Result<()> {
        ms_klc(output_path).await;
        Ok(())
    }
}

#[cfg(target_os = "windows")]
async fn ms_klc(output_path: &Path) {
    install_msklc().await;

    for entry in output_path.read_dir().unwrap().filter_map(Result::ok) {
        let path = entry.path();
        if let Some(extension) = path.extension() {
            if extension == "klc" {
                build_dll(&path, KlcBuildTarget::Amd64, &output_path);
                build_dll(&path, KlcBuildTarget::I386, &output_path);
                build_dll(&path, KlcBuildTarget::Wow64, &output_path);
            }
        }
    }
}

fn build_dll(klc_path: &Path, target: KlcBuildTarget, output_path: &Path) {
    let kbdutool = prefix_dir("windows")
        .join("pkg")
        .join("msklc")
        .join("bin")
        .join("i386")
        .join("kbdutool.exe");
    let current_dir = output_path.join(target.arch());
    std::fs::create_dir_all(&current_dir).unwrap();
    let mut proc = std::process::Command::new(kbdutool)
        .arg("-n")
        .arg(target.flag())
        .arg("-u")
        .arg(dunce::canonicalize(klc_path).unwrap())
        .current_dir(dunce::canonicalize(current_dir).unwrap())
        .spawn()
        .unwrap();
    proc.wait().unwrap();
}

enum KlcBuildTarget {
    Wow64,
    I386,
    Amd64,
}

impl KlcBuildTarget {
    fn flag(&self) -> &str {
        match self {
            KlcBuildTarget::Wow64 => "-o",
            KlcBuildTarget::I386 => "-x",
            KlcBuildTarget::Amd64 => "-m",
        }
    }

    fn arch(&self) -> &str {
        match self {
            KlcBuildTarget::Wow64 => "wow64",
            KlcBuildTarget::I386 => "i386",
            KlcBuildTarget::Amd64 => "amd64",
        }
    }
}
