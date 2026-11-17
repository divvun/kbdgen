//! What registration reads from Windows before it decides
//! (`crate::registration`): this DLL's path, the machine's architecture,
//! the 64-bit `%ProgramFiles%`, registry strings and file DACLs.

use std::ffi::c_void;

use windows::Win32::Foundation::{
    ERROR_FILE_NOT_FOUND, ERROR_INSUFFICIENT_BUFFER, ERROR_INVALID_DATA, ERROR_NO_MORE_ITEMS,
    ERROR_PATH_NOT_FOUND, ERROR_SUCCESS, HLOCAL, HMODULE, LocalFree, WIN32_ERROR,
};
use windows::Win32::Security::Authorization::{
    ConvertSidToStringSidW, GetNamedSecurityInfoW, SE_FILE_OBJECT,
};
use windows::Win32::Security::{
    ACCESS_ALLOWED_ACE, ACE_HEADER, ACL, DACL_SECURITY_INFORMATION, GetAce, PSECURITY_DESCRIPTOR,
    PSID,
};
use windows::Win32::Storage::FileSystem::GetLongPathNameW;
use windows::Win32::System::LibraryLoader::{
    GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS, GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
    GetModuleFileNameW, GetModuleHandleExW,
};
use windows::Win32::System::Registry::{
    HKEY, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_64KEY, REG_SAM_FLAGS, RRF_RT_REG_SZ, RegCloseKey,
    RegEnumKeyExW, RegGetValueW, RegOpenKeyExW,
};
use windows::Win32::System::SystemInformation::IMAGE_FILE_MACHINE;
use windows::Win32::System::Threading::{GetCurrentProcess, IsWow64Process2};
use windows_core::{Error, PCWSTR, PWSTR, Result};

use crate::registration::{Ace, Machine, Refusal};

use super::wide;

const INHERIT_ONLY_ACE: u8 = 0x08;
const ACCESS_ALLOWED_ACE_TYPE: u8 = 0;
const ACCESS_DENIED_ACE_TYPE: u8 = 1;

/// The architecture this DLL is built for.
pub(super) const OWN: Machine = if cfg!(target_arch = "x86") {
    Machine::X86
} else if cfg!(target_arch = "aarch64") {
    Machine::Arm64
} else {
    Machine::X64
};

impl From<Refusal> for Error {
    fn from(refusal: Refusal) -> Error {
        Error::from(WIN32_ERROR(refusal.code()))
    }
}

/// Why registration failed. A refusal is kept apart from other errors
/// until the export returns, so diagnostics can name it
/// (`tsf.component.events`); its Win32 code alone is ambiguous.
pub(super) enum Failure {
    Refused(Refusal),
    Failed(Error),
}

impl From<Refusal> for Failure {
    fn from(refusal: Refusal) -> Failure {
        Failure::Refused(refusal)
    }
}

impl From<Error> for Failure {
    fn from(error: Error) -> Failure {
        Failure::Failed(error)
    }
}

impl From<Failure> for Error {
    fn from(failure: Failure) -> Error {
        match failure {
            Failure::Refused(refusal) => refusal.into(),
            Failure::Failed(error) => error,
        }
    }
}

/// Registration fails with real Win32 errors, never `Error::empty()`: its
/// code is `S_OK`, which `DllRegisterServer` would then return.
pub(super) fn check(status: WIN32_ERROR) -> Result<()> {
    if status == ERROR_SUCCESS {
        Ok(())
    } else {
        Err(Error::from(status))
    }
}

/// The first `len` units of `buffer`, a path, registry string or key name
/// that Windows wrote there, without a `\\?\` prefix.
fn path_from(buffer: &[u16], len: u32) -> Result<String> {
    let len = usize::try_from(len).unwrap_or(usize::MAX);
    let units = buffer
        .get(..len)
        .ok_or_else(|| Error::from(ERROR_INSUFFICIENT_BUFFER))?;
    let path = String::from_utf16(units).map_err(|_| Error::from(ERROR_INVALID_DATA))?;
    Ok(path
        .strip_prefix(r"\\?\")
        .map(str::to_owned)
        .unwrap_or(path))
}

/// This DLL's path, with short names expanded.
pub(super) fn module_path() -> Result<String> {
    let mut module = HMODULE::default();
    let anchor: fn() -> Result<String> = module_path;
    // SAFETY: with `GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS` the name is an
    // address inside this module and is never read as a string.
    unsafe {
        GetModuleHandleExW(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
            PCWSTR(anchor as *const u16),
            &mut module,
        )
    }?;
    let mut short = vec![0u16; 32_768];
    let len = unsafe { GetModuleFileNameW(Some(module), &mut short) };
    if len == 0 {
        return Err(Error::from_thread());
    }
    // Keeps the terminating NUL, which `GetLongPathNameW` needs. A
    // truncated path is reported as the buffer's whole length, which
    // leaves no room for it, and is refused.
    let short = short
        .get(..=usize::try_from(len).unwrap_or(usize::MAX))
        .ok_or_else(|| Error::from(ERROR_INSUFFICIENT_BUFFER))?;
    let mut long = vec![0u16; 32_768];
    let len = unsafe { GetLongPathNameW(PCWSTR(short.as_ptr()), Some(&mut long)) };
    if len == 0 {
        return Err(Error::from_thread());
    }
    if usize::try_from(len).unwrap_or(usize::MAX) >= long.len() {
        return Err(Error::from(ERROR_INSUFFICIENT_BUFFER));
    }
    path_from(&long, len)
}

/// The machine's native architecture, also from an emulated process.
pub(super) fn native_machine() -> std::result::Result<Machine, Failure> {
    let mut process = IMAGE_FILE_MACHINE::default();
    let mut native = IMAGE_FILE_MACHINE::default();
    unsafe { IsWow64Process2(GetCurrentProcess(), &mut process, Some(&mut native)) }?;
    Machine::from_image(native.0).ok_or(Failure::Refused(Refusal::Unsupported))
}

/// An open registry key, closed when dropped.
struct Key(HKEY);

impl Drop for Key {
    fn drop(&mut self) {
        let _ = unsafe { RegCloseKey(self.0) };
    }
}

impl Key {
    /// Opens `path` under `parent` for reading, in `view`; `None` if it
    /// does not exist.
    fn open(parent: HKEY, path: &str, view: REG_SAM_FLAGS) -> Result<Option<Key>> {
        let mut key = HKEY::default();
        let path = wide(path);
        let status = unsafe {
            RegOpenKeyExW(
                parent,
                PCWSTR(path.as_ptr()),
                None,
                KEY_READ | view,
                &mut key,
            )
        };
        if status == ERROR_FILE_NOT_FOUND {
            return Ok(None);
        }
        check(status)?;
        Ok(Some(Key(key)))
    }

    /// The names of this key's subkeys.
    fn subkeys(&self) -> Result<Vec<String>> {
        let mut names = Vec::new();
        for index in 0.. {
            let mut name = [0u16; 256];
            let mut len = 256u32;
            let status = unsafe {
                RegEnumKeyExW(
                    self.0,
                    index,
                    Some(PWSTR(name.as_mut_ptr())),
                    &mut len,
                    None,
                    None,
                    None,
                    None,
                )
            };
            if status == ERROR_NO_MORE_ITEMS {
                break;
            }
            check(status)?;
            names.push(path_from(&name, len)?);
        }
        Ok(names)
    }
}

/// The string value `name` (`None`: the default value) of the HKLM key
/// `path`, in `view`; `None` if the key or value does not exist.
pub(super) fn read_string(
    path: &str,
    name: Option<&str>,
    view: REG_SAM_FLAGS,
) -> Result<Option<String>> {
    let Some(key) = Key::open(HKEY_LOCAL_MACHINE, path, view)? else {
        return Ok(None);
    };
    let name = name.map(wide);
    let name = name.as_ref().map_or(PCWSTR::null(), |n| PCWSTR(n.as_ptr()));
    let mut value = vec![0u16; 32_768];
    let mut bytes = u32::try_from(value.len() * 2).unwrap_or(u32::MAX);
    let status = unsafe {
        RegGetValueW(
            key.0,
            PCWSTR::null(),
            name,
            RRF_RT_REG_SZ,
            None,
            Some(value.as_mut_ptr().cast::<c_void>()),
            Some(&mut bytes),
        )
    };
    if status == ERROR_FILE_NOT_FOUND {
        return Ok(None);
    }
    check(status)?;
    let units = (bytes / 2).saturating_sub(1);
    path_from(&value, units).map(Some)
}

/// Whether the HKLM key `path`, a text service's `LanguageProfile` key in
/// `CTF\TIP`, holds a profile: a LANGID key with a profile GUID key in it.
pub(super) fn profiles_remain(path: &str) -> Result<bool> {
    let Some(profiles) = Key::open(HKEY_LOCAL_MACHINE, path, REG_SAM_FLAGS(0))? else {
        return Ok(false);
    };
    for langid in profiles.subkeys()? {
        let key = Key::open(profiles.0, &langid, REG_SAM_FLAGS(0))?;
        if let Some(key) = key
            && !key.subkeys()?.is_empty()
        {
            return Ok(true);
        }
    }
    Ok(false)
}

/// The 64-bit `%ProgramFiles%`, also from a 32-bit process.
pub(super) fn program_files() -> Result<String> {
    read_string(
        r"SOFTWARE\Microsoft\Windows\CurrentVersion",
        Some("ProgramFilesDir"),
        KEY_WOW64_64KEY,
    )?
    .ok_or_else(|| Error::from(ERROR_FILE_NOT_FOUND))
}

fn sid_string(sid: PSID) -> Result<String> {
    let mut text = PWSTR::null();
    unsafe { ConvertSidToStringSidW(sid, &mut text) }?;
    // SAFETY: `ConvertSidToStringSidW` succeeded, so `text` is a
    // NUL-terminated string that it allocated and that is freed below.
    let string = unsafe { text.to_string() }.map_err(|_| Error::from(ERROR_INVALID_DATA));
    unsafe { LocalFree(Some(HLOCAL(text.0.cast::<c_void>()))) };
    string
}

/// The allowing and denying entries of `acl`. Other entry types are
/// skipped.
fn aces(acl: &ACL) -> Result<Vec<Ace>> {
    let mut found = Vec::new();
    for index in 0..u32::from(acl.AceCount) {
        let mut ace: *mut c_void = std::ptr::null_mut();
        unsafe { GetAce(acl, index, &mut ace) }?;
        // SAFETY: `GetAce` succeeded, so `ace` points at a DWORD-aligned
        // entry of `acl`, and every entry starts with a header.
        let header = unsafe { &*ace.cast::<ACE_HEADER>() };
        let allow = match header.AceType {
            ACCESS_ALLOWED_ACE_TYPE => true,
            ACCESS_DENIED_ACE_TYPE => false,
            _ => continue,
        };
        // SAFETY: allowing and denying entries share the layout of
        // `ACCESS_ALLOWED_ACE`.
        let entry = unsafe { &*ace.cast::<ACCESS_ALLOWED_ACE>() };
        let sid_start = std::mem::offset_of!(ACCESS_ALLOWED_ACE, SidStart);
        // SAFETY: `SidStart` lies inside the entry. The SID runs on past
        // that field, so its pointer is derived from `ace`, which covers
        // the whole entry, rather than from a reference to the field.
        let sid = PSID(unsafe { ace.byte_add(sid_start) });
        found.push(Ace {
            allow,
            inherit_only: header.AceFlags & INHERIT_ONLY_ACE != 0,
            mask: entry.Mask,
            sid: sid_string(sid)?,
        });
    }
    Ok(found)
}

/// The DACL of the file at `path`; `None` for a null DACL. A missing file
/// is [`Refusal::Missing`].
pub(super) fn dacl(path: &str) -> std::result::Result<Option<Vec<Ace>>, Failure> {
    let name = wide(path);
    let mut acl: *mut ACL = std::ptr::null_mut();
    let mut descriptor = PSECURITY_DESCRIPTOR::default();
    let status = unsafe {
        GetNamedSecurityInfoW(
            PCWSTR(name.as_ptr()),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            None,
            None,
            Some(&mut acl),
            None,
            &mut descriptor,
        )
    };
    if status == ERROR_FILE_NOT_FOUND || status == ERROR_PATH_NOT_FOUND {
        return Err(Refusal::Missing(path.to_owned()).into());
    }
    check(status)?;
    // SAFETY: `acl` is null or points into `descriptor`, which is freed
    // only once `aces` has copied out what it needs.
    let result = match unsafe { acl.as_ref() } {
        Some(acl) => aces(acl).map(Some),
        None => Ok(None),
    };
    unsafe { LocalFree(Some(HLOCAL(descriptor.0))) };
    Ok(result?)
}
