//! What `DllRegisterServer` and `DllUnregisterServer` do: the CLSID with
//! this DLL as its `InprocServer32`, and the text service's TSF categories.
//! Profiles are `kbdi`'s to register.

use windows::Win32::Foundation::{ERROR_SUCCESS, HMODULE, WIN32_ERROR};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
    CoUninitialize,
};
use windows::Win32::System::LibraryLoader::{
    GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS, GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
    GetModuleFileNameW, GetModuleHandleExW,
};
use windows::Win32::System::Registry::{
    HKEY, HKEY_LOCAL_MACHINE, KEY_WRITE, REG_OPTION_NON_VOLATILE, REG_SZ, RegCloseKey,
    RegCreateKeyExW, RegDeleteTreeW, RegSetValueExW,
};
use windows::Win32::UI::TextServices::{
    CLSID_TF_CategoryMgr, GUID_TFCAT_DISPLAYATTRIBUTEPROVIDER, GUID_TFCAT_TIP_KEYBOARD,
    GUID_TFCAT_TIPCAP_IMMERSIVESUPPORT, GUID_TFCAT_TIPCAP_SECUREMODE,
    GUID_TFCAT_TIPCAP_SYSTRAYSUPPORT, GUID_TFCAT_TIPCAP_UIELEMENTENABLED, ITfCategoryMgr,
};
use windows_core::{Error, GUID, PCWSTR, Result};

use crate::guid::{CLSID, braced};

/// The categories of `tsf.register.server`. Secure mode support lets the
/// text service run under `TF_TMF_SECUREMODE` (`tsf.security.secure-mode`).
// [spec:kbdgen:req:tsf.security.secure-mode]
pub(super) const CATEGORIES: [GUID; 6] = [
    GUID_TFCAT_TIP_KEYBOARD,
    GUID_TFCAT_TIPCAP_IMMERSIVESUPPORT,
    GUID_TFCAT_TIPCAP_SYSTRAYSUPPORT,
    GUID_TFCAT_TIPCAP_UIELEMENTENABLED,
    GUID_TFCAT_TIPCAP_SECUREMODE,
    GUID_TFCAT_DISPLAYATTRIBUTEPROVIDER,
];

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain([0]).collect()
}

fn check(status: WIN32_ERROR) -> Result<()> {
    if status == ERROR_SUCCESS {
        Ok(())
    } else {
        Err(Error::from(status.to_hresult()))
    }
}

fn class_key() -> String {
    format!("SOFTWARE\\Classes\\CLSID\\{}", braced(CLSID))
}

fn module_path() -> Result<String> {
    let mut module = HMODULE::default();
    let anchor: fn() -> Result<String> = module_path;
    unsafe {
        GetModuleHandleExW(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
            PCWSTR(anchor as *const u16),
            &mut module,
        )
    }?;
    let mut path = vec![0u16; 32_768];
    let len =
        usize::try_from(unsafe { GetModuleFileNameW(Some(module), &mut path) }).unwrap_or_default();
    path.truncate(len);
    String::from_utf16(&path).map_err(|_| Error::empty())
}

/// Creates `path` under `HKLM` and sets its string values; `None` names the
/// default value.
fn write_key(path: &str, values: &[(Option<&str>, &str)]) -> Result<()> {
    let mut key = HKEY::default();
    let path = wide(path);
    check(unsafe {
        RegCreateKeyExW(
            HKEY_LOCAL_MACHINE,
            PCWSTR(path.as_ptr()),
            None,
            PCWSTR::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_WRITE,
            None,
            &mut key,
            None,
        )
    })?;
    let result = values.iter().try_for_each(|(name, value)| {
        let name = name.map(wide);
        let data: Vec<u8> = wide(value).iter().flat_map(|u| u.to_le_bytes()).collect();
        let name = name.as_ref().map_or(PCWSTR::null(), |n| PCWSTR(n.as_ptr()));
        check(unsafe { RegSetValueExW(key, name, None, REG_SZ, Some(&data)) })
    });
    let _ = unsafe { RegCloseKey(key) };
    result
}

/// Runs `body` with COM initialised on this thread.
fn with_com<T>(body: impl FnOnce() -> Result<T>) -> Result<T> {
    let initialised = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.is_ok();
    let result = body();
    if initialised {
        unsafe { CoUninitialize() };
    }
    result
}

fn categories() -> Result<ITfCategoryMgr> {
    unsafe { CoCreateInstance(&CLSID_TF_CategoryMgr, None, CLSCTX_INPROC_SERVER) }
}

/// Registers the CLSID, as an apartment-threaded in-process server at this
/// DLL's path, and the text service's categories.
pub fn register() -> Result<()> {
    let key = class_key();
    write_key(&key, &[(None, "Divvun keyboard text service")])?;
    let path = module_path()?;
    write_key(
        &format!("{key}\\InprocServer32"),
        &[(None, &path), (Some("ThreadingModel"), "Apartment")],
    )?;
    let clsid = GUID::from_u128(CLSID);
    with_com(|| {
        let categories = categories()?;
        CATEGORIES.iter().try_for_each(|category| unsafe {
            categories.RegisterCategory(&clsid, category, &clsid)
        })
    })
}

/// Removes what [`register`] added.
pub fn unregister() -> Result<()> {
    let clsid = GUID::from_u128(CLSID);
    let categories = with_com(|| {
        let categories = categories()?;
        for category in &CATEGORIES {
            let _ = unsafe { categories.UnregisterCategory(&clsid, category, &clsid) };
        }
        Ok(())
    });
    let key = wide(&class_key());
    let status = unsafe { RegDeleteTreeW(HKEY_LOCAL_MACHINE, PCWSTR(key.as_ptr())) };
    categories?;
    if status == windows::Win32::Foundation::ERROR_FILE_NOT_FOUND {
        Ok(())
    } else {
        check(status)
    }
}
