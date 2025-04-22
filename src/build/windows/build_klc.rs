use std::ffi::OsStr;
use std::{path::Path, sync::Arc};

use anyhow::Result;
use async_trait::async_trait;
use msvc_env::{CommandExt as _, MsvcArch};

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
                build_dll(&path, MsvcArch::X64, &output_path);
                build_dll(&path, MsvcArch::X86, &output_path);
                build_dll(&path, MsvcArch::Arm64, &output_path);
            }
        }
    }
}

fn build_dll(klc_path: &Path, target: MsvcArch, output_path: &Path) {
    let kbdutool = prefix_dir("windows")
        .join("pkg")
        .join("msklc")
        .join("bin")
        .join("i386")
        .join("kbdutool.exe");
    let current_dir = output_path
        .join(target.to_string().replace("\"", ""))
        .join("build");
    println!("current_dir: {:?}", &current_dir);
    std::fs::create_dir_all(&current_dir).unwrap();
    let current_dir = dunce::canonicalize(&current_dir).unwrap();
    let mut proc = std::process::Command::new(kbdutool)
        .arg("-n")
        .arg("-s")
        .arg("-u")
        .arg(dunce::canonicalize(klc_path).unwrap())
        .current_dir(current_dir.to_str().unwrap())
        .spawn()
        .unwrap();
    proc.wait().unwrap();
    println!("{:?}", output_path);
    // List files in current_dir and filter by ends with .C, then collcet the names without the .C extension
    let prefixes = std::fs::read_dir(&output_path)
        .unwrap()
        .filter_map(Result::ok)
        .filter_map(|entry| {
            if entry.path().extension() == Some(OsStr::new("klc")) {
                Some(
                    entry
                        .path()
                        .file_name()
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .replace(".klc", ""),
                )
            } else {
                None
            }
        })
        .collect::<Vec<String>>();

    println!("prefixes: {:?}", prefixes);

    for prefix in prefixes {
        let mut cmd = cl_command(current_dir.to_str().unwrap(), &prefix);
        cmd.msvc_env(target)
            .unwrap()
            .current_dir(&current_dir)
            .spawn()
            .unwrap()
            .wait()
            .unwrap();
        let mut cmd = rc_command(current_dir.to_str().unwrap(), &prefix);
        cmd.msvc_env(target)
            .unwrap()
            .current_dir(&current_dir)
            .spawn()
            .unwrap()
            .wait()
            .unwrap();
        let mut cmd = link_command(current_dir.to_str().unwrap(), &prefix);
        cmd.msvc_env(target)
            .unwrap()
            .current_dir(&current_dir)
            .spawn()
            .unwrap()
            .wait()
            .unwrap();
        std::fs::rename(
            current_dir.join(format!("{}.dll", prefix)),
            current_dir
                .parent()
                .unwrap()
                .join(format!("{}.dll", prefix)),
        )
        .unwrap();
    }
}

fn cl_command(include_path: &str, name: &str) -> std::process::Command {
    let mut cmd = std::process::Command::new("cl.exe");
    cmd.arg("-nologo")
        .arg(format!("-I{}", include_path))
        .arg("-DNODGICAPMASKS")
        .arg("-DNO_WIN_MESSAGES")
        .arg("-DNO_WIN_STYLES")
        .arg("-DNO_SYSMETRICS")
        .arg("-DNOMENUS")
        .arg("-DNOCIONS")
        .arg("-DNOSYSCOMMANDS")
        .arg("-DNORASTEROPS")
        .arg("-DNOSHOWWINDOW")
        .arg("-DOEMRESOURCE")
        .arg("-DONATOM")
        .arg("-DNOCURSOR")
        .arg("-DNOCOLOR")
        .arg("-DNOCTLMGR")
        .arg("-DNODRAWTEXT")
        .arg("-DNOGDI")
        .arg("-DNOKERNEL")
        .arg("-DNONLS")
        .arg("-DNOMB")
        .arg("-DNOMEMMGR")
        .arg("-DNOMETAFILE")
        .arg("-DNOMINMAX")
        .arg("-DNOMSG")
        .arg("-DNOOPENFILE")
        .arg("-DNOSCROLL")
        .arg("-DNOSERVICE")
        .arg("-DNOSOUND")
        .arg("-DNOTEXTMETRIC")
        .arg("-DNOWINOFFSETS")
        .arg("-DNOWH")
        .arg("-DNOCOMM")
        .arg("-DNOKANJI")
        .arg("-DJI")
        .arg("-DNOHELP")
        .arg("-DNOPROFILER")
        .arg("-DNODEFERWINDOWPOS")
        .arg("-DNOMCX")
        .arg("-DWIN32_LEAN_AND_MEAN")
        .arg("-Droster")
        .arg("-DSTDCALL")
        .arg("-D_WIN32_W")
        .arg("-DNT=0x0500")
        .arg("/DWINVER=0x0500")
        .arg("-D_WIN32_IE=0x0500")
        .arg("/MD")
        .arg("/c")
        .arg("/Zp8")
        .arg("/Gy")
        .arg("/W3")
        .arg("/WX")
        .arg("/Gz")
        .arg("/Gm-")
        .arg("/EHs-c-")
        .arg("/GR-")
        .arg("/GF")
        .arg("-Z7")
        .arg("/Oxs")
        // .arg(name)
        .arg(format!("{}.C", name));
    cmd
}

fn rc_command(include_path: &str, name: &str) -> std::process::Command {
    let mut cmd = std::process::Command::new("rc.exe");
    cmd.arg("-r")
        .arg(format!("-I{}", include_path))
        .arg("-DSTDCALL")
        .arg("-DCONDITION_HANDLING=1")
        .arg("-DNT_UP=1")
        .arg("-DNT_INST=0")
        .arg("-DWIN32=100")
        .arg("-D_NT1X_=100")
        .arg("-DWINNT=1")
        .arg("-D_WIN32_WINNT=0x0500")
        .arg("/DWINVER=0x0400")
        .arg("-D_WIN32_IE=0x0400")
        .arg("-DWIN32_LEAN_AND_MEAN=1")
        .arg("-DDEVEL=1")
        .arg("-DFPO=1")
        .arg("-DNDEBUG")
        .arg("-l")
        .arg("409")
        .arg(format!("{}.RC", name));
    cmd
}

fn link_command(current_dir: &str, name: &str) -> std::process::Command {
    let mut cmd = std::process::Command::new("link.exe");
    cmd.arg("-nologo")
        // .arg(name)
        // .arg(name)
        .arg("-SECTION:INIT,D")
        .arg("-OPT:REF")
        .arg("-OPT:ICF")
        .arg("-IGNORE:4039,4078")
        .arg("-noentry")
        .arg("-dll")
        // .arg(format!("-libpath:{}", lib_path))
        .arg("-subsystem:native,5.0")
        .arg("-merge:.rdata=.text")
        // .arg("-PDBPATH:NONE")
        .arg("-STACK:0x40000,0x1000")
        // .arg("/opt:nowin98")
        .arg("-osversion:4.0")
        .arg("-version:4.0")
        .arg("/release")
        .arg(format!("-def:{}.def", name))
        .arg(format!("{}.res", name))
        .arg(format!("{}.obj", name));
    cmd
}
