//! What `DllRegisterServer` decides before it writes anything: the path
//! `InprocServer32` names and the files it loads (`tsf.arch.registration`),
//! where those files may lie (`tsf.register.upgrade`,
//! `tsf.security.appcontainer`), and whether their DACLs let AppContainers
//! read and execute them. The Windows layer gathers the facts; the
//! decisions are here, so every host tests them.

/// The directory each version of the text service is installed into is
/// named for this version (`tsf.register.upgrade`).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The Arm64X forwarder (`tsf.arch.arm64x`).
pub const FORWARDER: &str = "divvun_tip.dll";

/// The SIDs that must be granted read and execute: `ALL APPLICATION
/// PACKAGES` and `ALL RESTRICTED APPLICATION PACKAGES`.
pub const PACKAGES: [&str; 2] = ["S-1-15-2-1", "S-1-15-2-2"];

const EVERYONE: &str = "S-1-1-0";
const FILE_GENERIC_READ: u32 = 0x0012_0089;
const FILE_GENERIC_EXECUTE: u32 = 0x0012_00A0;
const FILE_ALL_ACCESS: u32 = 0x001F_01FF;
const GENERIC_READ: u32 = 0x8000_0000;
const GENERIC_EXECUTE: u32 = 0x2000_0000;
const GENERIC_ALL: u32 = 0x1000_0000;
const READ_EXECUTE: u32 = FILE_GENERIC_READ | FILE_GENERIC_EXECUTE;

/// A Windows architecture, of a DLL or of the machine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Machine {
    X86,
    X64,
    Arm64,
}

impl Machine {
    pub const fn from_image(machine: u16) -> Option<Machine> {
        match machine {
            0x014C => Some(Machine::X86),
            0x8664 => Some(Machine::X64),
            0xAA64 => Some(Machine::Arm64),
            _ => None,
        }
    }

    /// The name of this architecture's DLL (`tsf.arch.builds`).
    pub const fn dll(self) -> &'static str {
        match self {
            Machine::X86 => "divvun_tip_x86.dll",
            Machine::X64 => "divvun_tip_x64.dll",
            Machine::Arm64 => "divvun_tip_arm64.dll",
        }
    }
}

/// Why `DllRegisterServer` registers nothing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The DLL is not named for its architecture.
    Name,
    /// The DLL cannot serve this machine's 64-bit view.
    Unsupported,
    /// The DLL's directory is not named for its version.
    Version,
    /// The DLL does not lie under the 64-bit `%ProgramFiles%`.
    Location,
    /// A file the registration loads does not exist.
    Missing(String),
    /// A file the registration loads does not grant read and execute to
    /// both package SIDs.
    Access(String),
}

impl Refusal {
    /// The Win32 error code `DllRegisterServer` returns as an `HRESULT`.
    pub const fn code(&self) -> u32 {
        match self {
            Refusal::Name => 123,       // ERROR_INVALID_NAME
            Refusal::Unsupported => 50, // ERROR_NOT_SUPPORTED
            Refusal::Version => 267,    // ERROR_DIRECTORY
            Refusal::Location => 161,   // ERROR_BAD_PATHNAME
            Refusal::Missing(_) => 2,   // ERROR_FILE_NOT_FOUND
            Refusal::Access(_) => 1336, // ERROR_INVALID_ACL
        }
    }
}

/// What one DLL registers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plan {
    /// The `InprocServer32` default value.
    pub server: String,
    /// Whether this DLL registers the categories. They live in the shared
    /// `CTF\TIP` key, so only the DLL that writes the 64-bit view
    /// registers them, or the x86 DLL on x86 Windows.
    pub categories: bool,
    /// Every file a process loads through `server`.
    pub files: Vec<String>,
}

/// Splits a Windows path at its last separator.
fn split(path: &str) -> (&str, &str) {
    path.rsplit_once('\\').unwrap_or(("", path))
}

/// Plans the registration of the DLL at `module`, built for `own`, on a
/// machine whose native architecture is `native`: the row of
/// `tsf.arch.registration` that its process's registry view holds. A
/// 32-bit process writes the `WOW6432Node` view on 64-bit Windows; on
/// Arm64 both the arm64 and the x64 DLL write the shared 64-bit view,
/// which names the forwarder.
// [spec:kbdgen:req:tsf.arch.registration+1]
pub fn plan(module: &str, own: Machine, native: Machine) -> Result<Plan, Refusal> {
    let (dir, name) = split(module);
    if !name.eq_ignore_ascii_case(own.dll()) {
        return Err(Refusal::Name);
    }
    let sibling = |name: &str| format!("{dir}\\{name}");
    match (own, native) {
        (Machine::X86, _) | (Machine::X64, Machine::X64) => Ok(Plan {
            server: module.to_owned(),
            categories: own != Machine::X86 || native == Machine::X86,
            files: vec![module.to_owned()],
        }),
        (Machine::X64 | Machine::Arm64, Machine::Arm64) => Ok(Plan {
            server: sibling(FORWARDER),
            categories: true,
            files: [FORWARDER, Machine::Arm64.dll(), Machine::X64.dll()]
                .map(sibling)
                .to_vec(),
        }),
        _ => Err(Refusal::Unsupported),
    }
}

/// Checks that `module` lies in a directory named `version` under
/// `program_files`, the 64-bit `%ProgramFiles%`, with no `.` or `..`
/// component. Comparisons ignore ASCII case, as Windows paths do.
// [spec:kbdgen:req:tsf.register.upgrade+2]
// [spec:kbdgen:req:tsf.security.appcontainer+2]
pub fn place(module: &str, program_files: &str, version: &str) -> Result<(), Refusal> {
    let (dir, _) = split(module);
    if !split(dir).1.eq_ignore_ascii_case(version) {
        return Err(Refusal::Version);
    }
    let root = program_files.trim_end_matches('\\');
    let inside = dir
        .get(..root.len())
        .is_some_and(|head| head.eq_ignore_ascii_case(root))
        && dir.get(root.len()..).is_some_and(|tail| {
            tail.starts_with('\\')
                && tail
                    .split('\\')
                    .skip(1)
                    .all(|c| !matches!(c, "" | "." | ".."))
        });
    if root.is_empty() || !inside {
        return Err(Refusal::Location);
    }
    Ok(())
}

/// An access control entry of a file's DACL.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ace {
    pub allow: bool,
    pub inherit_only: bool,
    pub mask: u32,
    pub sid: String,
}

/// `mask` with its generic rights mapped to file rights.
const fn file_rights(mask: u32) -> u32 {
    let mut rights = mask;
    if mask & GENERIC_READ != 0 {
        rights |= FILE_GENERIC_READ;
    }
    if mask & GENERIC_EXECUTE != 0 {
        rights |= FILE_GENERIC_EXECUTE;
    }
    if mask & GENERIC_ALL != 0 {
        rights |= FILE_ALL_ACCESS;
    }
    rights
}

/// The package SIDs a DACL does not grant read and execute; `None` is a
/// null DACL, which grants everything. A right counts as granted when an
/// allowing entry for the SID grants it and no denying entry for the SID
/// or for Everyone takes it, in whatever order. Inherit-only entries do
/// not apply to the file.
// [spec:kbdgen:req:tsf.security.appcontainer+2]
pub fn ungranted(dacl: Option<&[Ace]>) -> Vec<&'static str> {
    let Some(dacl) = dacl else {
        return Vec::new();
    };
    let effective = || dacl.iter().filter(|ace| !ace.inherit_only);
    PACKAGES
        .into_iter()
        .filter(|sid| {
            let union = |allow: bool, applies: &dyn Fn(&str) -> bool| {
                effective()
                    .filter(|ace| ace.allow == allow && applies(&ace.sid))
                    .fold(0, |rights, ace| rights | file_rights(ace.mask))
            };
            let allowed = union(true, &|ace| ace == *sid);
            let denied = union(false, &|ace| ace == *sid || ace == EVERYONE);
            allowed & !denied & READ_EXECUTE != READ_EXECUTE
        })
        .collect()
}
