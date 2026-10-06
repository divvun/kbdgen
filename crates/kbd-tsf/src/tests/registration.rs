//! Registration's decisions: the `InprocServer32` row per architecture,
//! the versioned directory under `%ProgramFiles%`, and the package grants.

use crate::registration::{
    Ace, FORWARDER, Machine, PACKAGES, Plan, Refusal, VERSION, place, plan, ungranted,
};

const DIR: &str = r"C:\Program Files\Divvun\Text Service\0.1.0";
const PROGRAM_FILES: &str = r"C:\Program Files";

fn at(name: &str) -> String {
    format!(r"{DIR}\{name}")
}

/// The plan of the DLL built for `machine`, named as `kbdgen tsf` names it.
fn own(machine: Machine, native: Machine) -> Result<Plan, Refusal> {
    let module = at(machine.dll());
    plan(&module, machine, native)
}

fn ace(allow: bool, mask: u32, sid: &str) -> Ace {
    Ace {
        allow,
        inherit_only: false,
        mask,
        sid: sid.to_owned(),
    }
}

/// The ACEs `%ProgramFiles%` passes to the files in it on Windows 11.
fn program_files_dacl() -> Vec<Ace> {
    vec![
        ace(
            true,
            0x001F_01FF,
            "S-1-5-80-956008885-3418522649-1831038044-1853292631-2271478464",
        ),
        ace(true, 0x001F_01FF, "S-1-5-18"),
        ace(true, 0x001F_01FF, "S-1-5-32-544"),
        ace(true, 0x0012_00A9, "S-1-5-32-545"),
        ace(true, 0x0012_00A9, PACKAGES[0]),
        ace(true, 0x0012_00A9, PACKAGES[1]),
    ]
}

// [spec:kbdgen:req:tsf.arch.registration+1/test]
#[test]
fn x64_windows_names_x64_and_x86_dlls() {
    let x64 = own(Machine::X64, Machine::X64).unwrap();
    assert_eq!(x64.server, at("divvun_tip_x64.dll"));
    assert_eq!(x64.files, [at("divvun_tip_x64.dll")]);
    assert!(x64.categories);
    let x86 = own(Machine::X86, Machine::X64).unwrap();
    assert_eq!(x86.server, at("divvun_tip_x86.dll"));
    assert!(
        !x86.categories,
        "the 64-bit DLL registers the shared categories"
    );
}

// [spec:kbdgen:req:tsf.arch.registration+1/test]
#[test]
fn arm64_windows_names_forwarder_from_both() {
    for machine in [Machine::Arm64, Machine::X64] {
        let plan = own(machine, Machine::Arm64).unwrap();
        assert_eq!(plan.server, at(FORWARDER));
        assert_eq!(
            plan.files,
            [
                at(FORWARDER),
                at("divvun_tip_arm64.dll"),
                at("divvun_tip_x64.dll")
            ]
        );
        assert!(plan.categories);
    }
    let x86 = own(Machine::X86, Machine::Arm64).unwrap();
    assert_eq!(x86.server, at("divvun_tip_x86.dll"));
    assert!(!x86.categories);
}

// [spec:kbdgen:req:tsf.arch.registration+1/test]
#[test]
fn x86_windows_names_x86_dll_with_categories() {
    let x86 = own(Machine::X86, Machine::X86).unwrap();
    assert_eq!(x86.server, at("divvun_tip_x86.dll"));
    assert!(x86.categories);
}

// [spec:kbdgen:req:tsf.arch.registration+1/test]
#[test]
fn refuses_misnamed_or_foreign_dlls() {
    assert_eq!(
        plan(&at("kbd_tsf.dll"), Machine::X64, Machine::X64),
        Err(Refusal::Name)
    );
    assert_eq!(
        plan(&at("divvun_tip_x86.dll"), Machine::X64, Machine::X64),
        Err(Refusal::Name)
    );
    assert!(plan(&at("DIVVUN_TIP_X64.DLL"), Machine::X64, Machine::X64).is_ok());
    assert_eq!(own(Machine::Arm64, Machine::X64), Err(Refusal::Unsupported));
    assert_eq!(own(Machine::X64, Machine::X86), Err(Refusal::Unsupported));
}

// [spec:kbdgen:req:tsf.register.upgrade+1/test]
#[test]
fn accepts_only_versioned_directory() {
    let module = at("divvun_tip_x64.dll");
    assert_eq!(place(&module, PROGRAM_FILES, "0.1.0"), Ok(()));
    assert_eq!(place(&module, r"c:\program files\", "0.1.0"), Ok(()));
    assert_eq!(
        place(&module, PROGRAM_FILES, "0.2.0"),
        Err(Refusal::Version)
    );
    let flat = r"C:\Program Files\Divvun\Text Service\divvun_tip_x64.dll";
    assert_eq!(place(flat, PROGRAM_FILES, "0.1.0"), Err(Refusal::Version));
    let versioned = format!(r"C:\Program Files\Divvun\Text Service\{VERSION}\divvun_tip_x64.dll");
    assert_eq!(place(&versioned, PROGRAM_FILES, VERSION), Ok(()));
}

// [spec:kbdgen:req:tsf.security.appcontainer+2/test]
#[test]
fn refuses_files_outside_program_files() {
    for module in [
        r"C:\Users\admin\0.1.0\divvun_tip_x64.dll",
        r"C:\Program Files (x86)\Divvun\0.1.0\divvun_tip_x64.dll",
        r"C:\Program FilesX\0.1.0\divvun_tip_x64.dll",
        r"C:\Program Files\..\Users\0.1.0\divvun_tip_x64.dll",
        r"C:\Program Files\.\0.1.0\divvun_tip_x64.dll",
        r"D:\Program Files\0.1.0\divvun_tip_x64.dll",
        r"0.1.0\divvun_tip_x64.dll",
    ] {
        assert_eq!(
            place(module, PROGRAM_FILES, "0.1.0"),
            Err(Refusal::Location),
            "{module}"
        );
    }
    let module = at("divvun_tip_x64.dll");
    assert_eq!(place(&module, "", "0.1.0"), Err(Refusal::Location));
}

// [spec:kbdgen:req:tsf.security.appcontainer+2/test]
#[test]
fn program_files_inheritance_grants_both_packages() {
    assert!(ungranted(Some(&program_files_dacl())).is_empty());
    assert!(ungranted(None).is_empty(), "a null DACL grants everything");
    let generic = [
        ace(true, 0xA000_0000, PACKAGES[0]),
        ace(true, 0x1000_0000, PACKAGES[1]),
    ];
    assert!(ungranted(Some(&generic)).is_empty());
}

// [spec:kbdgen:req:tsf.security.appcontainer+2/test]
#[test]
fn reports_each_package_lacking_read_execute() {
    let mut dacl = program_files_dacl();
    dacl.retain(|ace| ace.sid != PACKAGES[1]);
    assert_eq!(ungranted(Some(&dacl)), [PACKAGES[1]]);

    let read_only = [
        ace(true, 0x0012_0089, PACKAGES[0]),
        ace(true, 0x0012_00A9, PACKAGES[1]),
    ];
    assert_eq!(ungranted(Some(&read_only)), [PACKAGES[0]]);

    let mut inherit_only = program_files_dacl();
    for ace in &mut inherit_only {
        ace.inherit_only = true;
    }
    assert_eq!(ungranted(Some(&inherit_only)), PACKAGES);
    assert_eq!(ungranted(Some(&[])), PACKAGES);
}

// [spec:kbdgen:req:tsf.security.appcontainer+2/test]
#[test]
fn denials_take_package_rights_away() {
    let mut dacl = program_files_dacl();
    dacl.push(ace(false, 0x0000_0020, PACKAGES[0]));
    assert_eq!(ungranted(Some(&dacl)), [PACKAGES[0]]);
    let mut everyone = program_files_dacl();
    everyone.insert(0, ace(false, 0x0000_0001, "S-1-1-0"));
    assert_eq!(ungranted(Some(&everyone)), PACKAGES);
}

#[test]
fn refusals_return_distinct_win32_errors() {
    let refusals = [
        Refusal::Name,
        Refusal::Unsupported,
        Refusal::Version,
        Refusal::Location,
        Refusal::Missing(String::new()),
        Refusal::Access(String::new()),
    ];
    let mut codes: Vec<u32> = refusals.iter().map(Refusal::code).collect();
    codes.sort_unstable();
    codes.dedup();
    assert_eq!(codes.len(), refusals.len());
}

#[test]
fn image_machines_map_to_architectures() {
    assert_eq!(Machine::from_image(0x014C), Some(Machine::X86));
    assert_eq!(Machine::from_image(0x8664), Some(Machine::X64));
    assert_eq!(Machine::from_image(0xAA64), Some(Machine::Arm64));
    assert_eq!(Machine::from_image(0xA641), None);
}
