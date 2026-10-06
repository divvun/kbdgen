//! The Windows 11 VM integration test (`tsf.test.vm`), run explicitly on a
//! machine with an interactive session:
//!
//! ```text
//! kbdgen target -b crates/kbd-tsf/tests/vm/fixture.kbdgen -o <out> windows
//! set KBD_TSF_VM_LAYOUT=<out>
//! cargo test -p kbd-tsf --test vm -- --ignored --nocapture
//! ```
//!
//! `tests/vm/run-vm.ps1` registers a test build and profile, types each
//! case below with `SendInput` scan codes in the signed-in session, and
//! removes everything again; this test checks what each control holds.

#![cfg(windows)]

use std::collections::BTreeMap;
use std::process::Command;

/// Name, scan-code chords (`e0` prefixes extended keys, `+` holds keys
/// together), and the UTF-16 units the control must then hold.
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
];

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

// [spec:kbdgen:req:tsf.test.vm/test]
// [spec:kbdgen:req:tsf.component.crate/test]
// [spec:kbdgen:req:tsf.edit.apply/test]
// [spec:kbdgen:req:tsf.edit.inject/test]
// [spec:kbdgen:req:tsf.edit.preedit/test]
// [spec:kbdgen:req:tsf.edit.reset/test]
// [spec:kbdgen:req:tsf.keys.altgr/test]
// [spec:kbdgen:req:tsf.data.locate/test]
// [spec:kbdgen:req:tsf.pairing.self-sufficient/test]
// [spec:kbdgen:req:tsf.test.vm]
// [spec:kbdgen:req:tsf.test.vm/test]
#[test]
#[ignore = "needs the Windows 11 VM with a signed-in session and KBD_TSF_VM_LAYOUT"]
fn types_through_text_service_in_apps() {
    let layout = std::env::var("KBD_TSF_VM_LAYOUT")
        .expect("KBD_TSF_VM_LAYOUT names the kbdgen output of tests/vm/fixture.kbdgen");
    let cases_file = std::env::temp_dir().join("kbd-tsf-vm-cases.txt");
    let cases: Vec<String> = CASES
        .iter()
        .map(|(name, chords, _)| format!("{name}={chords}"))
        .collect();
    std::fs::write(&cases_file, cases.join("\n")).unwrap();
    let script = concat!(env!("CARGO_MANIFEST_DIR"), "\\tests\\vm\\run-vm.ps1");
    let output = Command::new("powershell.exe")
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", script])
        .arg("-LayoutDir")
        .arg(&layout)
        .arg("-CasesFile")
        .arg(&cases_file)
        .output()
        .unwrap();
    let _ = std::fs::remove_file(&cases_file);
    let stdout = String::from_utf8_lossy(&output.stdout);
    println!("{stdout}");
    eprintln!("{}", String::from_utf8_lossy(&output.stderr));
    assert!(!stdout.contains("SKIPPED"), "nobody is signed in");

    let mut results = BTreeMap::new();
    for line in stdout.lines() {
        let Some(rest) = line.trim().strip_prefix("RESULT ") else {
            continue;
        };
        let (key, units) = rest.split_once(" => ").unwrap_or((rest, ""));
        let mut words = key.splitn(3, ' ');
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

    let mut failures = Vec::new();
    for (kind, bits) in CONTROLS {
        for (name, _, expected) in CASES {
            let key = ((*kind).to_owned(), (*bits).to_owned(), (*name).to_owned());
            let got = results.get(&key).map_or("missing", String::as_str);
            if got != *expected {
                failures.push(format!(
                    "{kind} {bits}-bit {name}: got [{got}], want [{expected}]"
                ));
            }
        }
    }
    let dll_only = results
        .get(&("edit-dll".to_owned(), "64".to_owned(), "acute b".to_owned()))
        .map_or("missing", String::as_str);
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
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
