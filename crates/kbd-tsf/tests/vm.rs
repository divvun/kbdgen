//! The Windows 11 VM integration test (`tsf.test.vm`), run explicitly on a
//! machine with an interactive session:
//!
//! ```text
//! kbdgen target -b crates/kbd-tsf/tests/vm/fixture.kbdgen -o <out> windows
//! set KBD_TSF_VM_LAYOUT=<out>
//! cargo test -p kbd-tsf --test vm -- --ignored --nocapture
//! ```
//!
//! `tests/vm/run-vm.ps1` checks where `DllRegisterServer` refuses to
//! register, registers a test build and a profile per fixture layout, types
//! each case below with `SendInput` scan codes in the signed-in session,
//! into the controls of `CONTROLS` and into a console switched to the
//! profile with Win+Space, and removes everything again; these tests check
//! what registration wrote and what each control holds. All tests share
//! one run.

#![cfg(windows)]

use std::collections::BTreeMap;
use std::process::Command;
use std::sync::OnceLock;

/// Name, scan-code chords (`e0` prefixes extended keys, `+` holds keys
/// together), and the UTF-16 units the control must then hold. A chord
/// `set:` puts UTF-16 units into the control without typing, caret at the
/// end, and `select:1,1` selects one unit from offset 1; the console skips
/// cases that put text.
const CASES: &[(&str, &str, &str)] = &[
    ("plain", "10 11 12", "0071 0077 0065"),
    ("acute a", "0d 1e", "00e1"),
    ("acute b", "0d 30", "0062 0301"),
    ("acute j", "0d 24", "00b4 006a"),
    ("caron c", "29 2e", "010d"),
    ("tilde o", "2a+29 18", "00f5"),
    ("altgr t", "e038+14", "0074 0301"),
    ("altgr acute a", "e038+0d 1e", "00e1"),
    ("decimal", "53", "002c"),
    ("acute end a", "0d e04f 1e", "00b4 0061"),
    ("acute backspace a", "0d 0e 1e", "0061"),
    ("q apostrophe", "10 2b", "02a0"),
    ("a q apostrophe", "1e 10 2b", "0061 02a0"),
    ("z apostrophe twice", "2c 2b 2b", "0290"),
    (
        "acute over selection",
        "set:0078,0079,007a select:1,1 0d 1e",
        "0078 00e1 007a",
    ),
];

const GRINNING: &str = "d83d de00";
const TECHNOLOGIST: &str = "d83d dc69 d83c dffd 200d d83d dcbb";
const FAMILY_RAINBOW: &str =
    "d83d dc68 200d d83d dc69 200d d83d dc67 200d d83d dc66 d83c dff3 fe0f 200d d83c df08";

/// The emoji fixture layout's keys (`und-Zsye.yaml`) and their outputs, as
/// UTF-16 units: a surrogate pair, a skin-toned ZWJ sequence, and one
/// output of 17 units, more than a layout DLL ligature delivers.
// [spec:kbdgen:req:tsf.test.emoji]
const EMOJI_KEYS: &[(&str, &str, &str)] = &[
    ("q", "10", GRINNING),
    ("w", "11", TECHNOLOGIST),
    ("e", "12", FAMILY_RAINBOW),
];

/// The emoji layout's cases: each key typed; then Backspace after it; and
/// Backspace after the same text put into the control without typing.
/// The text service then holds no pending state, so by
/// `ldml.engine.backspace.default` (`CancelOrPass`) Backspace passes and
/// both must leave what the control's own Backspace leaves.
fn emoji_cases() -> Vec<String> {
    let mut cases = Vec::new();
    for (key, scan, units) in EMOJI_KEYS {
        let set = units.replace(' ', ",");
        cases.push(format!("emoji {key}={scan}"));
        cases.push(format!("emoji {key} backspace={scan} 0e"));
        cases.push(format!("emoji {key} put backspace=set:{set} 0e"));
    }
    cases
}

/// The controls typed into through the text service, with the bitness of
/// the process.
const CONTROLS: &[(&str, &str)] = &[
    ("edit", "64"),
    ("rich", "64"),
    ("wpf", "64"),
    ("edit", "32"),
    ("wpf", "32"),
];

const EXPORTS: &str = "DllCanUnloadNow,DllGetClassObject,DllRegisterServer,DllUnregisterServer";

/// The categories of `tsf.register.server`, as `run-vm.ps1` lists them.
const CATEGORIES: &str = "{046B8C80-1647-40F7-9B21-B93B81AABC1B},\
                          {13A016DF-560B-46CD-947A-4C3AF1E0E35D},\
                          {25504FB4-7BAB-4BC1-9C69-CF81890F0EF5},\
                          {34745C63-B2F0-4784-8B67-5E12C8701A31},\
                          {49D2F9CE-1F5E-11D7-A6D3-00065B84435C},\
                          {49D2F9CF-1F5E-11D7-A6D3-00065B84435C}";

/// How `DllRegisterServer` refuses each misplaced copy of the x64 DLL:
/// `ERROR_INVALID_NAME`, `ERROR_DIRECTORY`, `ERROR_BAD_PATHNAME` and
/// `ERROR_INVALID_ACL` as `HRESULT`s.
const REFUSALS: &[(&str, &str)] = &[
    ("name", "0x8007007b"),
    ("version", "0x8007010b"),
    ("location", "0x800700a1"),
    ("acl", "0x80070538"),
];

/// C runtime DLLs, which a `+crt-static` build does not import.
const CRT: &[&str] = &["vcruntime", "msvcp", "ucrtbase", "api-ms-win-crt-"];

/// What the controls held: (kind, bits, case name) → UTF-16 units.
type Results = BTreeMap<(String, String, String), String>;

/// Runs `tests/vm/run-vm.ps1` once for every test of this file, giving its
/// output and the results it reports.
fn run() -> Result<&'static (String, Results), &'static str> {
    static RUN: OnceLock<Result<(String, Results), String>> = OnceLock::new();
    let run = RUN.get_or_init(|| {
        let layout = std::env::var("KBD_TSF_VM_LAYOUT").map_err(
            |_| "KBD_TSF_VM_LAYOUT must name the kbdgen output of tests/vm/fixture.kbdgen",
        )?;
        let temp = std::env::temp_dir();
        let cases_file = temp.join("kbd-tsf-vm-cases.txt");
        let emoji_file = temp.join("kbd-tsf-vm-emoji-cases.txt");
        let cases: Vec<String> = CASES
            .iter()
            .map(|(name, chords, _)| format!("{name}={chords}"))
            .collect();
        std::fs::write(&cases_file, cases.join("\n")).map_err(|e| e.to_string())?;
        std::fs::write(&emoji_file, emoji_cases().join("\n")).map_err(|e| e.to_string())?;
        let script = concat!(env!("CARGO_MANIFEST_DIR"), "\\tests\\vm\\run-vm.ps1");
        let output = Command::new("powershell.exe")
            .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", script])
            .arg("-LayoutDir")
            .arg(&layout)
            .arg("-CasesFile")
            .arg(&cases_file)
            .arg("-EmojiCasesFile")
            .arg(&emoji_file)
            .output()
            .map_err(|e| e.to_string())?;
        let _ = std::fs::remove_file(&cases_file);
        let _ = std::fs::remove_file(&emoji_file);
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        println!("{stdout}");
        eprintln!("{}", String::from_utf8_lossy(&output.stderr));

        let mut results = Results::new();
        for line in stdout.lines() {
            let Some(rest) = line.trim().strip_prefix("RESULT ") else {
                continue;
            };
            let (key, units) = rest.split_once("=>").unwrap_or((rest, ""));
            let mut words = key.trim().splitn(3, ' ');
            let (kind, bits, name) = (
                words.next().unwrap_or_default(),
                words.next().unwrap_or_default(),
                words.next().unwrap_or_default(),
            );
            results.insert(
                (kind.to_owned(), bits.to_owned(), name.to_owned()),
                units.trim().to_owned(),
            );
        }
        Ok((stdout, results))
    });
    run.as_ref().map_err(String::as_str)
}

/// The rest of the output line that starts with `prefix`.
fn line<'a>(stdout: &'a str, prefix: &str) -> &'a str {
    stdout
        .lines()
        .find_map(|line| line.trim().strip_prefix(prefix))
        .map_or("missing", str::trim)
}

fn result<'a>(results: &'a Results, kind: &str, bits: &str, name: &str) -> &'a str {
    let key = (kind.to_owned(), bits.to_owned(), name.to_owned());
    results.get(&key).map_or("missing", String::as_str)
}

/// What the console, switched to the profile with Win+Space, typed
/// differently from `cases` (name, chords, expected units), skipping the
/// cases that put text without typing.
fn console_failures<'a>(
    results: &Results,
    cases: impl IntoIterator<Item = (&'a str, &'a str, &'a str)>,
) -> Vec<String> {
    cases
        .into_iter()
        .filter(|(_, chords, _)| !chords.contains("set:"))
        .filter_map(|(name, _, expected)| {
            let got = result(results, "console", "64", name);
            (got != expected).then(|| format!("console {name}: got [{got}], want [{expected}]"))
        })
        .collect()
}

// [spec:kbdgen:req:tsf.component.crate/test]
// [spec:kbdgen:req:tsf.edit.apply+1/test]
// [spec:kbdgen:req:tsf.edit.inject+1/test]
// [spec:kbdgen:req:tsf.edit.preedit+2/test]
// [spec:kbdgen:req:tsf.edit.reset+1/test]
// [spec:kbdgen:req:tsf.keys.altgr+2/test]
// [spec:kbdgen:req:tsf.data.locate+1/test]
// [spec:kbdgen:req:tsf.pairing.self-sufficient/test]
// [spec:kbdgen:req:tsf.test.vm+2]
// [spec:kbdgen:req:tsf.test.vm+2/test]
// [spec:kbdgen:req:tsf.keys.recover/test]
// [spec:kbdgen:req:tsf.keys.phases/test]
// [spec:kbdgen:req:tsf.keys.preserved+1/test]
// [spec:kbdgen:req:tsf.edit.own/test]
#[test]
#[ignore = "needs the Windows 11 VM with a signed-in session and KBD_TSF_VM_LAYOUT"]
fn types_through_text_service_in_apps() {
    let (stdout, results) = run().unwrap();
    assert!(!stdout.contains("SKIPPED"), "nobody is signed in");

    let mut failures = Vec::new();
    for (kind, bits) in CONTROLS {
        for (name, _, expected) in CASES {
            let got = result(results, kind, bits, name);
            if got != *expected {
                failures.push(format!(
                    "{kind} {bits}-bit {name}: got [{got}], want [{expected}]"
                ));
            }
        }
    }
    failures.extend(console_failures(results, CASES.iter().copied()));
    let dll_only = result(results, "edit-dll", "64", "acute b");
    if dll_only == "missing" || dll_only == "0062 0301" {
        failures.push(format!(
            "the layout DLL alone typed acute b as [{dll_only}]"
        ));
    }
    for arch in ["x64", "x86"] {
        if !stdout.contains(&format!("EXPORTS {arch} {EXPORTS}")) {
            failures.push(format!("{arch} exports are not exactly {EXPORTS}"));
        }
    }
    if stdout.contains("LEFT ") || !stdout.contains("CLEANUP done") {
        failures.push("cleanup left registrations or files behind".to_owned());
    }
    let languages = line(stdout, "LANGUAGES before ");
    if languages == "missing" || line(stdout, "LANGUAGES after ") != languages {
        failures.push("the user's languages and inputs were not restored".to_owned());
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

// [spec:kbdgen:req:tsf.test.emoji/test]
#[test]
#[ignore = "needs the Windows 11 VM with a signed-in session and KBD_TSF_VM_LAYOUT"]
fn types_emoji_through_text_service() {
    let (stdout, results) = run().unwrap();
    assert!(!stdout.contains("SKIPPED"), "nobody is signed in");

    let mut failures = Vec::new();
    for (kind, bits) in CONTROLS {
        for (key, _, units) in EMOJI_KEYS {
            let typed = result(results, kind, bits, &format!("emoji {key}"));
            if typed != *units {
                failures.push(format!(
                    "{kind} {bits}-bit {key}: got [{typed}], want [{units}]"
                ));
            }
            let erased = result(results, kind, bits, &format!("emoji {key} backspace"));
            let native = result(results, kind, bits, &format!("emoji {key} put backspace"));
            let shorter = erased.len() < units.len() && units.starts_with(erased);
            if erased != native || !shorter {
                failures.push(format!(
                    "{kind} {bits}-bit {key} backspace: got [{erased}], the control's own \
                     backspace gives [{native}] of [{units}]"
                ));
            }
        }
    }
    let console: Vec<(String, &str, &str)> = EMOJI_KEYS
        .iter()
        .map(|(key, scan, units)| (format!("emoji {key}"), *scan, *units))
        .collect();
    failures.extend(console_failures(
        results,
        console
            .iter()
            .map(|(name, scan, units)| (name.as_str(), *scan, *units)),
    ));
    // The layout DLL alone delivers ligatures of up to 16 units and has
    // no key for a longer output (`kbdl.vk-chars.values`).
    for (key, _, units) in EMOJI_KEYS {
        let want = if units.split(' ').count() > 16 {
            ""
        } else {
            units
        };
        let got = result(results, "edit-dll", "64", &format!("emoji {key}"));
        if got != want {
            failures.push(format!(
                "the layout DLL alone typed {key} as [{got}], want [{want}]"
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

// [spec:kbdgen:req:tsf.arch.registration+1/test]
// [spec:kbdgen:req:tsf.arch.builds+1/test]
// [spec:kbdgen:req:tsf.register.server/test]
// [spec:kbdgen:req:tsf.register.upgrade+2/test]
// [spec:kbdgen:req:tsf.security.appcontainer+2/test]
#[test]
#[ignore = "needs the Windows 11 VM with a signed-in session and KBD_TSF_VM_LAYOUT"]
fn registers_and_refuses_text_service_dlls() {
    let (stdout, _) = run().unwrap();
    assert!(!stdout.contains("SKIPPED"), "nobody is signed in");

    let install = line(stdout, "INSTALL ");
    let x64 = format!("{install}\\divvun_tip_x64.dll Apartment");
    let x86 = format!("{install}\\divvun_tip_x86.dll Apartment");
    let mut expected: Vec<(String, String)> = REFUSALS
        .iter()
        .map(|(name, hresult)| {
            (
                format!("REFUSE {name} "),
                format!("{hresult} registered=False"),
            )
        })
        .collect();
    for (prefix, want) in [
        ("REGISTER x86 ", "0"),
        ("CATEGORIES x86 ", "none"),
        ("REGISTER x64 ", "0"),
        ("SERVER 64 ", &x64),
        ("SERVER 32 ", &x86),
        ("CATEGORIES both ", CATEGORIES),
        ("STALE ", &format!("0x00000000 {x64}")),
        ("CATEGORIES x86-removed ", CATEGORIES),
        ("UNREGISTERED ", "64=False 32=False tip=False"),
    ] {
        expected.push((prefix.to_owned(), want.to_owned()));
    }
    let mut failures = Vec::new();
    for (prefix, want) in &expected {
        let got = line(stdout, prefix);
        if got != want {
            failures.push(format!("{prefix}: got [{got}], want [{want}]"));
        }
    }
    for arch in ["x64", "x86"] {
        let depends = line(stdout, &format!("DEPENDS {arch} "));
        let crt = depends
            .split(',')
            .any(|dll| CRT.iter().any(|prefix| dll.starts_with(prefix)));
        if depends == "missing" || crt {
            failures.push(format!("{arch} imports [{depends}]"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
