use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};
use async_trait::async_trait;
use msvc_env::{CommandExt as _, MsvcArch};

use crate::build::pahkat::{install_msklc, prefix_dir};
use crate::{build::BuildStep, bundle::KbdgenBundle};

const ENVS: &[MsvcArch] = &[MsvcArch::X64, MsvcArch::X86, MsvcArch::Arm64];

pub struct BuildKlc {}

// [spec:kbdgen:req:windows.dll]
#[async_trait(?Send)]
impl BuildStep for BuildKlc {
    async fn build(&self, _bundle: &KbdgenBundle, output_path: &Path) -> Result<()> {
        for target in ENVS {
            if !target.is_valid_environment() {
                bail!("{} is not a valid environment", target);
            }
        }
        ms_klc(output_path).await
    }
}

// [spec:kbdgen:req:windows.dll]
async fn ms_klc(output_path: &Path) -> Result<()> {
    install_msklc().await;

    let mut layouts = Vec::new();
    for entry in output_path
        .read_dir()
        .context("read Windows build directory")?
    {
        let path = entry?.path();
        if path.extension().and_then(|value| value.to_str()) == Some("klc") {
            layouts.push(path);
        }
    }
    layouts.sort();
    for path in layouts {
        for target in ENVS {
            build_dll(&path, *target, output_path)
                .with_context(|| format!("build {} for {}", path.display(), target))?;
        }
    }
    Ok(())
}

// [spec:kbdgen:req:windows.dll]
fn build_dll(klc_path: &Path, target: MsvcArch, output_path: &Path) -> Result<()> {
    let prefix = klc_path
        .file_stem()
        .and_then(|value| value.to_str())
        .context("KLC path must have a Unicode file stem")?;
    let kbdutool = prefix_dir("windows")
        .join("pkg")
        .join("msklc")
        .join("bin")
        .join("i386")
        .join("kbdutool.exe");
    let architecture_dir = output_path.join(target.to_string().replace('"', ""));
    // Keep intermediates for different layouts separate, including on rebuilds.
    let current_dir = architecture_dir.join("build").join(prefix);
    std::fs::create_dir_all(&current_dir).context("create layout build directory")?;
    let current_dir = dunce::canonicalize(&current_dir)?;
    let include_path = current_dir.to_str().context("build path must be Unicode")?;
    run_command(
        Command::new(kbdutool)
            .args(["-n", "-s", "-u"])
            .arg(dunce::canonicalize(klc_path)?)
            .current_dir(&current_dir),
        "generate KLC sources",
    )?;

    // Only this KLC's sources have been generated. Never enumerate sibling KLCs here.
    for (stage, mut command) in [
        ("compile layout", cl_command(include_path, prefix)),
        ("compile resources", rc_command(include_path, prefix)),
        ("link layout", link_command(prefix)),
    ] {
        command
            .msvc_env(target)
            .context("configure MSVC environment")?;
        run_command(command.current_dir(&current_dir), stage)?;
    }
    std::fs::rename(
        current_dir.join(format!("{}.dll", prefix)),
        architecture_dir.join(format!("{}.dll", prefix)),
    )
    .context("move completed keyboard DLL")?;
    Ok(())
}

// [spec:kbdgen:req:windows.dll]
fn run_command(command: &mut Command, stage: &str) -> Result<()> {
    let program = command.get_program().to_owned();
    let status = command
        .status()
        .with_context(|| format!("{}: start {:?}", stage, program))?;
    if !status.success() {
        bail!("{}: {:?} failed with {}", stage, program, status);
    }
    Ok(())
}

// [spec:kbdgen:req:windows.dll.link]
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

// [spec:kbdgen:req:windows.dll.link]
fn link_command(name: &str) -> std::process::Command {
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
        // Match Microsoft's keyboard-layout samples: the layout loader expects
        // the descriptor, code, and tables together in executable read-only data.
        // https://github.com/microsoft/Windows-driver-samples/blob/main/input/layout/kbdus/kbdus.vcxproj
        .arg("-merge:.edata=.data")
        .arg("-merge:.rdata=.data")
        .arg("-merge:.text=.data")
        .arg("-merge:.bss=.data")
        .arg("-section:.data,re")
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

#[cfg(test)]
mod tests {
    use super::*;

    // [spec:kbdgen:req:windows.dll/test]
    #[test]
    fn failed_child_stops_the_build_with_stage_and_status() {
        let mut command = Command::new("cmd.exe");
        command.args(["/d", "/c", "exit", "7"]);
        let error = run_command(&mut command, "compile resources")
            .unwrap_err()
            .to_string();
        assert!(error.contains("compile resources"));
        assert!(error.contains("7"));
    }

    // [spec:kbdgen:req:windows.dll/test]
    #[test]
    fn missing_tool_is_an_error_and_success_is_accepted() {
        let mut command = Command::new("kbdgen-nonexistent-test-compiler.exe");
        assert!(run_command(&mut command, "compile layout").is_err());
        let mut command = Command::new("cmd.exe");
        command.args(["/d", "/c", "exit", "0"]);
        run_command(&mut command, "compile layout").unwrap();
    }
}
