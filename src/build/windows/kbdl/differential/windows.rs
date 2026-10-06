//! `tsf.test.differential` against the real `ToUnicodeEx`: each fixture
//! layout is built, installed under a test KLID, loaded, and every case is
//! typed through it. The result must equal the simulation exactly, and may
//! differ from the engine only by a listed exception.
//!
//! Run elevated in an interactive session, since it installs layouts:
//! `cargo test real_dlls -- --ignored`. KLIDs `a0f00409` upwards and files
//! `kbdzf<n>.dll` in System32 are used and removed again.

use std::path::Path;
use std::process::Command;

use super::compare::{Case, Outcome, Report, cases};
use super::*;
use crate::build::BuildStep;
use crate::build::windows::kbdl::BuildKbdl;

const KLF_NOTELLSHELL: u32 = 0x80;
const MAPVK_VSC_TO_VK: u32 = 1;
const TOUNICODE_KEEP_STATE: u32 = 0x4;
const VK_SPACE: u32 = 0x20;
const VK_CAPITAL: usize = 0x14;
const VK_LSHIFT: usize = 0xa0;
const VK_LCONTROL: usize = 0xa2;
const VK_RMENU: usize = 0xa5;
const LAYOUTS_KEY: &str = r"HKLM\SYSTEM\CurrentControlSet\Control\Keyboard Layouts";

#[link(name = "user32")]
unsafe extern "system" {
    fn LoadKeyboardLayoutW(klid: *const u16, flags: u32) -> isize;
    fn UnloadKeyboardLayout(hkl: isize) -> i32;
    fn MapVirtualKeyExW(code: u32, map_type: u32, hkl: isize) -> u32;
    fn ToUnicodeEx(
        vk: u32,
        scan: u32,
        state: *const u8,
        buf: *mut u16,
        len: i32,
        flags: u32,
        hkl: isize,
    ) -> i32;
}

/// A layout DLL installed under a test KLID; dropping it unloads and
/// removes it.
struct Installed {
    klid: String,
    file: std::path::PathBuf,
    hkl: isize,
}

fn reg(args: &[&str]) {
    let status = Command::new("reg").args(args).status().unwrap();
    assert!(status.success(), "reg {args:?}: {status}");
}

impl Installed {
    fn new(dll: &Path, index: usize) -> Installed {
        let klid = format!("a0f{index:x}0409");
        let name = format!("kbdzf{index}.dll");
        let system = std::env::var("SystemRoot").unwrap();
        let file = Path::new(&system).join("System32").join(&name);
        std::fs::copy(dll, &file).unwrap_or_else(|e| panic!("{}: {e}", file.display()));
        let key = format!(r"{LAYOUTS_KEY}\{klid}");
        for (value, data) in [
            ("Layout File", name.as_str()),
            ("Layout Text", "kbdgen differential test"),
            ("Layout Id", &format!("00f{index:x}")),
        ] {
            reg(&["add", &key, "/v", value, "/t", "REG_SZ", "/d", data, "/f"]);
        }
        let wide: Vec<u16> = klid.encode_utf16().chain([0]).collect();
        let hkl = unsafe { LoadKeyboardLayoutW(wide.as_ptr(), KLF_NOTELLSHELL) };
        let installed = Installed { klid, file, hkl };
        assert_ne!(hkl, 0, "LoadKeyboardLayout({})", installed.klid);
        installed
    }

    /// One `ToUnicodeEx` call: its return value and the units it wrote.
    fn to_unicode(&self, vk: u32, scan: u32, state: &[u8; 256], flags: u32) -> (i32, Vec<u16>) {
        let mut buf = [0u16; 64];
        let n = unsafe {
            ToUnicodeEx(
                vk,
                scan,
                state.as_ptr(),
                buf.as_mut_ptr(),
                buf.len() as i32,
                flags,
                self.hkl,
            )
        };
        let len = usize::try_from(n.unsigned_abs())
            .unwrap_or(0)
            .min(buf.len());
        (n, buf[..len].to_vec())
    }

    /// Types `case`'s presses from a cleared dead-key state. A call that
    /// types nothing keeps the pending dead key; with none known, a space
    /// that does not change the state checks that none is pending.
    fn typed(&self, case: &Case) -> Outcome {
        let empty = [0u8; 256];
        for _ in 0..2 {
            self.to_unicode(VK_SPACE, 0x39, &empty, 0);
        }
        let mut text = Vec::new();
        let mut pending = None;
        for press in &case.path {
            let keys = press.keys();
            let mut state = [0u8; 256];
            for vk in keys.held() {
                state[usize::from(vk)] = 0x80;
            }
            if keys.shift {
                state[VK_LSHIFT] = 0x80;
            }
            if keys.ctrl {
                state[VK_LCONTROL] = 0x80;
            }
            if keys.alt {
                state[VK_RMENU] = 0x80;
            }
            if keys.caps {
                state[VK_CAPITAL] = 0x01;
            }
            let scan = u32::from(press.scan());
            let vk = unsafe { MapVirtualKeyExW(scan, MAPVK_VSC_TO_VK, self.hkl) };
            let (n, units) = self.to_unicode(vk, scan, &state, 0);
            if n < 0 {
                pending = Some(String::from_utf16_lossy(&units[..1]));
            } else if n > 0 {
                pending = None;
                text.extend(units);
            }
        }
        if pending.is_none() {
            let (_, probe) = self.to_unicode(VK_SPACE, 0x39, &empty, TOUNICODE_KEEP_STATE);
            if probe != [0x20] {
                pending = Some(format!("probe {:?}", String::from_utf16_lossy(&probe)));
            }
        }
        Outcome {
            text: String::from_utf16_lossy(&text),
            pending,
        }
    }
}

impl Drop for Installed {
    fn drop(&mut self) {
        unsafe { UnloadKeyboardLayout(self.hkl) };
        let key = format!(r"{LAYOUTS_KEY}\{}", self.klid);
        let _ = Command::new("reg").args(["delete", &key, "/f"]).status();
        for _ in 0..10 {
            if std::fs::remove_file(&self.file).is_ok() || !self.file.exists() {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
        eprintln!("could not remove {}", self.file.display());
    }
}

// [spec:kbdgen:req:tsf.test.differential+1/test]
// [spec:kbdgen:req:kbdl.dead-keys+2/test]
// [spec:kbdgen:req:kbdl.ligatures+1/test]
#[tokio::test]
#[ignore = "installs layouts; needs an elevated interactive Windows session"]
async fn real_dlls_type_what_the_simulation_types() {
    let mut index = 0;
    let mut failures = Vec::new();
    for fixture in fixture_bundles() {
        let out = tempfile::tempdir().unwrap();
        write_crates(&fixture, out.path()).await;
        BuildKbdl.build(&fixture.bundle, out.path()).await.unwrap();
        for subject in subjects(&fixture) {
            let dll = out.path().join("x64").join(format!("{}.dll", subject.name));
            let installed = Installed::new(&dll, index);
            index += 1;
            let mut report = Report::default();
            let mut simulated = 0;
            for case in cases(&subject) {
                let real = installed.typed(&case);
                if real == case.dll {
                    simulated += 1;
                } else {
                    failures.push(format!(
                        "{}: {}: simulated {}, ToUnicodeEx {real}",
                        subject.name,
                        case.name(),
                        case.dll
                    ));
                }
                report.judge(&subject, &case, &real);
            }
            eprintln!(
                "{} ({}): {simulated} cases as simulated; against the engine: {}",
                fixture.label,
                subject.name,
                report.summary()
            );
            failures.extend(report.unexplained);
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
