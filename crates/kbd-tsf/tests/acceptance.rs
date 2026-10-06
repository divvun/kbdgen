//! The Windows 11 acceptance test (`tsf.test.acceptance`), run explicitly
//! on a machine with an interactive session, Inno Setup 6, and the
//! toolchains of `tsf.arch.builds` and of kbdi:
//!
//! ```text
//! set KBD_TSF_ACCEPTANCE_ISS=<dir with divvun-tip.iss and install.all.iss>
//! set KBD_TSF_ACCEPTANCE_KBDI=<kbdi checkout>
//! cargo test -p kbd-tsf --test acceptance -- --ignored --nocapture
//! ```
//!
//! `install.all.iss` is divvun-actions' `generateKbdInnoFromBundle` output
//! for `tests/acceptance/fixture.kbdgen`, and `divvun-tip.iss` is
//! divvun-actions' `actions/kbd-tsf/divvun-tip.iss`. `tests/acceptance/
//! acceptance.ps1` builds and installs the keyboard as a user would get it,
//! types each case of `tests/acceptance/cases.rs` through the installed text
//! service, uninstalls it, and reports the machine's state before and after.
//! The text each control must hold is what `kbd-engine` makes of the case
//! on the model embedded in the built layout DLL, the model the text service
//! reads. All tests share one run.

#![cfg(windows)]

#[path = "acceptance/cases.rs"]
mod cases;
/// The text service's key handling, which the cases follow.
#[allow(dead_code)]
#[path = "../src/keys.rs"]
mod keys;

use std::collections::{BTreeMap, BTreeSet};
use std::process::Command;
use std::sync::OnceLock;

use cases::{CASES, Dll, End, engine_text, hex};
use kbd_engine::Model;
use windows::Win32::Foundation::FreeLibrary;
use windows::Win32::System::LibraryLoader::{
    FindResourceW, LOAD_LIBRARY_AS_DATAFILE, LOAD_LIBRARY_AS_IMAGE_RESOURCE, LoadLibraryExW,
    LoadResource, LockResource, SizeofResource,
};
use windows_core::{HSTRING, PCWSTR};

/// The controls typed into through the text service, with the bitness of
/// the process.
const CONTROLS: &[(&str, &str)] = &[
    ("edit", "64"),
    ("rich", "64"),
    ("wpf", "64"),
    ("edit", "32"),
    ("wpf", "32"),
];

/// The suffix of the case that puts a native case's text into the control
/// without typing and then presses Backspace.
const PUT: &str = " put";

/// What the controls held: (kind, bits, case name) → UTF-16 units.
type Results = BTreeMap<(String, String, String), String>;

/// The script's output and the results it reports.
struct Run {
    stdout: String,
    results: Results,
}

/// The cases file: every case, and for each native case its twin, which
/// puts the case's text into the control and erases with Backspace.
fn cases_file() -> String {
    let mut lines = Vec::new();
    for (name, chords, text, end, _) in CASES {
        lines.push(format!("{name}={chords}"));
        if *end == End::Native {
            lines.push(format!(
                "{name}{PUT}=set:{} 0e",
                hex(text).replace(' ', ",")
            ));
        }
    }
    lines.join("\n")
}

fn parse_results(stdout: &str) -> Results {
    let mut results = Results::new();
    for line in stdout.lines() {
        let Some(rest) = line.trim().strip_prefix("RESULT ") else {
            continue;
        };
        let (key, units) = rest.split_once("=>").unwrap_or((rest, ""));
        let mut words = key.trim().splitn(3, ' ');
        let mut word = || words.next().unwrap_or_default().to_owned();
        let key = (word(), word(), word());
        results.insert(key, units.trim().to_owned());
    }
    results
}

/// Runs `tests/acceptance/acceptance.ps1` once for every test of this file.
// [spec:kbdgen:req:tsf.test.acceptance]
fn run() -> Result<&'static Run, &'static str> {
    static RUN: OnceLock<Result<Run, String>> = OnceLock::new();
    let run = RUN.get_or_init(|| {
        let iss = std::env::var("KBD_TSF_ACCEPTANCE_ISS")
            .map_err(|_| "KBD_TSF_ACCEPTANCE_ISS must name the directory of the .iss scripts")?;
        let kbdi = std::env::var("KBD_TSF_ACCEPTANCE_KBDI")
            .map_err(|_| "KBD_TSF_ACCEPTANCE_KBDI must name a kbdi checkout")?;
        let cases = std::env::temp_dir().join("kbd-tsf-acceptance-cases.txt");
        std::fs::write(&cases, cases_file()).map_err(|e| e.to_string())?;
        let script = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "\\tests\\acceptance\\acceptance.ps1"
        );
        let output = Command::new("powershell.exe")
            .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", script])
            .args(["-IssDir", &iss, "-Kbdi", &kbdi])
            .arg("-CasesFile")
            .arg(&cases)
            .output()
            .map_err(|e| e.to_string())?;
        let _ = std::fs::remove_file(&cases);
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        println!("{stdout}");
        eprintln!("{}", String::from_utf8_lossy(&output.stderr));
        let results = parse_results(&stdout);
        Ok(Run { stdout, results })
    });
    run.as_ref().map_err(String::as_str)
}

/// The rests of the output lines that start with `prefix`.
fn lines<'a>(stdout: &'a str, prefix: &str) -> Vec<&'a str> {
    stdout
        .lines()
        .filter_map(|line| line.trim().strip_prefix(prefix))
        .map(str::trim)
        .collect()
}

fn result<'a>(results: &'a Results, kind: &str, bits: &str, name: &str) -> &'a str {
    let key = (kind.to_owned(), bits.to_owned(), name.to_owned());
    results.get(&key).map_or("missing", String::as_str)
}

/// The model resource (`tsf.data.resource`) of the layout DLL at `path`.
fn dll_model(path: &str) -> Result<Model, String> {
    let module = unsafe {
        LoadLibraryExW(
            &HSTRING::from(path),
            None,
            LOAD_LIBRARY_AS_DATAFILE | LOAD_LIBRARY_AS_IMAGE_RESOURCE,
        )
    }
    .map_err(|e| format!("{path}: {e}"))?;
    let info = unsafe { FindResourceW(Some(module), PCWSTR(1 as _), PCWSTR(10 as _)) };
    let bytes = if info.is_invalid() {
        None
    } else {
        let size = unsafe { SizeofResource(Some(module), info) } as usize;
        let data = unsafe { LoadResource(Some(module), info) }.ok();
        let pointer = data.map(|data| unsafe { LockResource(data) });
        pointer.filter(|pointer| !pointer.is_null()).map(|pointer| {
            unsafe { std::slice::from_raw_parts(pointer.cast::<u8>(), size) }.to_vec()
        })
    };
    let _ = unsafe { FreeLibrary(module) };
    let bytes = bytes.ok_or_else(|| format!("{path} has no model resource"))?;
    Model::from_bytes(&bytes).map_err(|e| format!("{path}: {e:?}"))
}

/// The model of the layout DLL the run built and installed.
fn model(stdout: &str) -> Result<Model, String> {
    let path = lines(stdout, "LAYOUTDLL ")
        .first()
        .copied()
        .ok_or("the run built no layout DLL")?;
    dll_model(path)
}

/// Whether the run happened; a run skipped for want of a session or disk
/// space fails the test.
fn ran(run: &Run) -> Result<(), String> {
    match lines(&run.stdout, "SKIPPED ").first() {
        Some(why) => Err(format!("skipped: {why}")),
        None => Ok(()),
    }
}

// [spec:kbdgen:req:tsf.test.acceptance/test]
#[test]
#[ignore = "needs the Windows 11 VM with a signed-in session, Inno Setup, the .iss scripts and kbdi"]
fn fixture_texts_are_the_engines() {
    let run = run().unwrap();
    ran(run).unwrap();
    let model = model(&run.stdout).unwrap();
    let mut failures = Vec::new();
    for (name, chords, text, end, _) in CASES {
        match engine_text(&model, chords, *end) {
            Ok(got) if got == *text => {}
            got => failures.push(format!(
                "{name}: the engine gives {got:?}, the case says {text:?}"
            )),
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

// [spec:kbdgen:req:tsf.test.acceptance/test]
#[test]
#[ignore = "needs the Windows 11 VM with a signed-in session, Inno Setup, the .iss scripts and kbdi"]
fn installed_text_service_types_as_engine() {
    let run = run().unwrap();
    ran(run).unwrap();
    let model = model(&run.stdout).unwrap();
    let mut failures = Vec::new();
    // The layout DLL's run loads a keyboard layout; the others activate
    // the profile and have no HKL.
    let setups = lines(&run.stdout, "SETUP ");
    for setup in setups.iter().filter(|setup| setup.contains(" hkl=0x0 ")) {
        if !setup.contains(" ours=True ") || !setup.contains(" foreground=True") {
            failures.push(format!("setup: {setup}"));
        }
    }
    if setups.len() != CONTROLS.len() + 1 {
        failures.push(format!("setups: {setups:?}"));
    }
    // A console switched to the keyboard with Win+Space types each case
    // whose Backspace, if any, the text service consumes.
    for (name, chords, _, end, _) in CASES.iter().filter(|case| case.3 == End::Engine) {
        let want = engine_text(&model, chords, *end)
            .map(|text| hex(&text))
            .unwrap();
        let got = result(&run.results, "console", "64", name);
        if got != want {
            failures.push(format!("console {name}: got [{got}], want [{want}]"));
        }
    }
    for (kind, bits) in CONTROLS {
        for (name, chords, _, end, _) in CASES {
            let want = engine_text(&model, chords, *end)
                .map(|text| hex(&text))
                .unwrap();
            let got = result(&run.results, kind, bits, name);
            let ok = match end {
                End::Engine => got == want,
                End::Native => {
                    let native = result(&run.results, kind, bits, &format!("{name}{PUT}"));
                    got == native && got.len() < want.len() && want.starts_with(got)
                }
            };
            if !ok {
                failures.push(format!(
                    "{kind} {bits}-bit {name}: got [{got}], want [{want}]"
                ));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

// [spec:kbdgen:req:tsf.test.acceptance/test]
#[test]
#[ignore = "needs the Windows 11 VM with a signed-in session, Inno Setup, the .iss scripts and kbdi"]
fn layout_dll_alone_types_table_cases() {
    let run = run().unwrap();
    ran(run).unwrap();
    let model = model(&run.stdout).unwrap();
    let mut failures = Vec::new();
    for (name, chords, _, end, dll) in CASES {
        let engine = engine_text(&model, chords, *end)
            .map(|text| hex(&text))
            .unwrap();
        let got = result(&run.results, "edit-dll", "64", name);
        let ok = match dll {
            Dll::Same => got == engine,
            Dll::Differs => got != engine && got != "missing",
            Dll::Unchecked => got != "missing",
        };
        if !ok {
            failures.push(format!(
                "{name} ({dll:?}): the DLL alone typed [{got}], the engine [{engine}]"
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

// [spec:kbdgen:req:tsf.test.acceptance/test]
// [spec:kbdgen:req:tsf.register.upgrade+2/test]
// [spec:kbdgen:req:tsf.register.uninstall+1/test]
#[test]
#[ignore = "needs the Windows 11 VM with a signed-in session, Inno Setup, the .iss scripts and kbdi"]
fn installers_install_then_restore_the_machine() {
    let run = run().unwrap();
    ran(run).unwrap();
    let stdout = &run.stdout;
    let mut failures = Vec::new();
    // Install, uninstall with the DLL loaded, reinstall, uninstall.
    let exits = lines(stdout, "EXIT ");
    if exits.len() != 4 || exits.iter().any(|exit| !exit.ends_with(" 0 finished=True")) {
        failures.push(format!("installer exits: {exits:?}"));
    }
    if lines(stdout, "HOLDER ") != ["loaded=True"] {
        failures.push("no process held the text service through the uninstall".to_owned());
    }
    // The uninstall moved the held DLL aside instead of leaving its path to
    // be deleted at the next restart, so the reinstall kept its own.
    let discarded = lines(stdout, "DISCARDED ");
    if discarded.is_empty()
        || discarded
            .iter()
            .any(|line| !line.ends_with(" removed=True"))
    {
        failures.push(format!("moved-aside DLLs: {discarded:?}"));
    }
    let version_dir = format!("\\Divvun\\Text Service\\{}\\", env!("CARGO_PKG_VERSION"));
    let installed = lines(stdout, "INSTALLED ");
    for label in ["INSTALLED ", "REINSTALLED "] {
        let reported = lines(stdout, label);
        for (prefix, file) in [
            ("server64 ", "divvun_tip_x64.dll"),
            ("server32 ", "divvun_tip_x86.dll"),
        ] {
            let server = reported.iter().find_map(|line| line.strip_prefix(prefix));
            if !server.is_some_and(|server| server.ends_with(&format!("{version_dir}{file}"))) {
                failures.push(format!("{label}{prefix}registration: {server:?}"));
            }
        }
        for pending in reported
            .iter()
            .filter_map(|line| line.strip_prefix("pending "))
        {
            failures.push(format!(
                "{label}: the registered {pending} is deleted at the next restart"
            ));
        }
    }
    let profiles: Vec<&&str> = installed
        .iter()
        .filter(|line| line.starts_with("profile "))
        .collect();
    if profiles.len() != 1 || !profiles.iter().all(|line| line.ends_with(" enable=1")) {
        failures.push(format!("profiles: {profiles:?}"));
    }
    if lines(stdout, "INPUT ")
        .first()
        .is_none_or(|input| input.is_empty())
    {
        failures.push("kbdi enabled no text service input".to_owned());
    }
    let before: BTreeSet<&str> = lines(stdout, "BEFORE ").into_iter().collect();
    let after: BTreeSet<&str> = lines(stdout, "AFTER ").into_iter().collect();
    if before.is_empty() {
        failures.push("no state was recorded".to_owned());
    }
    for gone in before.difference(&after) {
        failures.push(format!("removed by the round trip: {gone}"));
    }
    for added in after.difference(&before) {
        failures.push(format!("left by the round trip: {added}"));
    }
    for pending in lines(stdout, "PENDING ") {
        failures.push(format!(
            "{pending} is left to be deleted at the next restart"
        ));
    }
    for left in lines(stdout, "LEFT ") {
        failures.push(format!("cleanup found {left}"));
    }
    if !stdout.contains("CLEANUP done") {
        failures.push("the run did not finish".to_owned());
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
