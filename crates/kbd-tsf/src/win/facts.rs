//! What registration reads from Windows before it decides
//! (`crate::registration`): this DLL's path, the machine's architecture,
//! the 64-bit `%ProgramFiles%`, registry strings and file DACLs.

use std::ffi::c_void;

use windows::Win32::Foundation::{
    ERROR_FILE_NOT_FOUND, ERROR_NO_MORE_ITEMS, ERROR_PATH_NOT_FOUND, ERROR_SUCCESS, HLOCAL,
    HMODULE, LocalFree, WIN32_ERROR,
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
        Error::from(WIN32_ERROR(refusal.code()).to_hresult())
    }
}

pub(super) fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain([0]).collect()
}

pub(super) fn check(status: WIN32_ERROR) -> Result<()> {
    if status == ERROR_SUCCESS {
        Ok(())
    } else {
        Err(Error::from(status.to_hresult()))
    }
}

/// A path as `GetModuleFileNameW` or `GetLongPathNameW` wrote it into
/// `buffer`, `len` units long, without a `\\?\` prefix.
fn path_from(buffer: &[u16], len: u32) -> Result<String> {
    let len = usize::try_from(len).unwrap_or_default();
    let units = buffer.get(..len).ok_or_else(Error::empty)?;
    let path = String::from_utf16(units).map_err(|_| Error::empty())?;
    Ok(path
        .strip_prefix(r"\\?\")
        .map(str::to_owned)
        .unwrap_or(path))
}

/// This DLL's path, with short names expanded.
pub(super) fn module_path() -> Result<String> {
    let mut module = HMODULE::default();
    let anchor: fn() -> Result<String> = module_path;
    unsafe {
        GetModuleHandleExW(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
            PCWSTR(anchor as *const u16),
            &mut module,
        )
    }?;
    let mut short = vec![0u16; 32_768];
    let len = unsafe { GetModuleFileNameW(Some(module), &mut short) };
    let short = short
        .get(..=usize::try_from(len).unwrap_or_default())
        .ok_or_else(Error::empty)?;
    let mut long = vec![0u16; 32_768];
    let len = unsafe { GetLongPathNameW(PCWSTR(short.as_ptr()), Some(&mut long)) };
    if len == 0 || usize::try_from(len).unwrap_or(usize::MAX) >= long.len() {
        return Err(Error::from_thread());
    }
    path_from(&long, len)
}

/// The machine's native architecture, also from an emulated process.
pub(super) fn native_machine() -> Result<Machine> {
    let mut process = IMAGE_FILE_MACHINE::default();
    let mut native = IMAGE_FILE_MACHINE::default();
    unsafe { IsWow64Process2(GetCurrentProcess(), &mut process, Some(&mut native)) }?;
    Machine::from_image(native.0).ok_or_else(|| Refusal::Unsupported.into())
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
    .ok_or_else(Error::empty)
}

fn sid_string(sid: PSID) -> Result<String> {
    let mut text = PWSTR::null();
    unsafe { ConvertSidToStringSidW(sid, &mut text) }?;
    let string = unsafe { text.to_string() }.map_err(|_| Error::empty());
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
        let header = unsafe { &*ace.cast::<ACE_HEADER>() };
        let allow = match header.AceType {
            ACCESS_ALLOWED_ACE_TYPE => true,
            ACCESS_DENIED_ACE_TYPE => false,
            _ => continue,
        };
        // Allowing and denying entries share one layout.
        let entry = unsafe { &*ace.cast::<ACCESS_ALLOWED_ACE>() };
        let sid = PSID(
            std::ptr::from_ref(&entry.SidStart)
                .cast_mut()
                .cast::<c_void>(),
        );
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
pub(super) fn dacl(path: &str) -> Result<Option<Vec<Ace>>> {
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
    let result = match unsafe { acl.as_ref() } {
        Some(acl) => aces(acl).map(Some),
        None => Ok(None),
    };
    unsafe { LocalFree(Some(HLOCAL(descriptor.0))) };
    result
}
