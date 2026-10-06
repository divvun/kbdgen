//! What `DllRegisterServer` and `DllUnregisterServer` do: the CLSID with
//! the server path of `tsf.arch.registration`, and the text service's TSF
//! categories. Registration first checks where the DLL lies and what its
//! files grant (`crate::registration`), and writes nothing otherwise.
//! Profiles are `kbdi`'s to register.

use windows::Win32::Foundation::ERROR_FILE_NOT_FOUND;
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
    CoUninitialize,
};
use windows::Win32::System::Registry::{
    HKEY, HKEY_LOCAL_MACHINE, KEY_WRITE, REG_OPTION_NON_VOLATILE, REG_SAM_FLAGS, REG_SZ,
    RegCloseKey, RegCreateKeyExW, RegDeleteTreeW, RegSetValueExW,
};
use windows::Win32::UI::TextServices::{
    CLSID_TF_CategoryMgr, GUID_TFCAT_DISPLAYATTRIBUTEPROVIDER, GUID_TFCAT_TIP_KEYBOARD,
    GUID_TFCAT_TIPCAP_IMMERSIVESUPPORT, GUID_TFCAT_TIPCAP_SECUREMODE,
    GUID_TFCAT_TIPCAP_SYSTRAYSUPPORT, GUID_TFCAT_TIPCAP_UIELEMENTENABLED, ITfCategoryMgr,
};
use windows_core::{GUID, PCWSTR, Result};

use super::facts::{
    OWN, check, dacl, module_path, native_machine, profiles_remain, program_files, read_string,
    wide,
};
use crate::guid::{CLSID, braced};
use crate::registration::{Plan, Refusal, VERSION, place, plan, ungranted};

/// The categories of `tsf.register.server`. Secure mode support lets the
/// text service run under `TF_TMF_SECUREMODE` (`tsf.security.secure-mode`).
// [spec:kbdgen:req:tsf.security.secure-mode+1]
// [spec:kbdgen:req:tsf.security.appcontainer+2]
pub(super) const CATEGORIES: [GUID; 6] = [
    GUID_TFCAT_TIP_KEYBOARD,
    GUID_TFCAT_TIPCAP_IMMERSIVESUPPORT,
    GUID_TFCAT_TIPCAP_SYSTRAYSUPPORT,
    GUID_TFCAT_TIPCAP_UIELEMENTENABLED,
    GUID_TFCAT_TIPCAP_SECUREMODE,
    GUID_TFCAT_DISPLAYATTRIBUTEPROVIDER,
];

fn class_key() -> String {
    format!("SOFTWARE\\Classes\\CLSID\\{}", braced(CLSID))
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

/// Runs `body` on the category manager, with COM initialised.
fn with_categories(body: impl FnOnce(&ITfCategoryMgr) -> Result<()>) -> Result<()> {
    with_com(|| {
        let manager: ITfCategoryMgr =
            unsafe { CoCreateInstance(&CLSID_TF_CategoryMgr, None, CLSCTX_INPROC_SERVER) }?;
        body(&manager)
    })
}

/// Checks that this DLL lies in its version's directory under the 64-bit
/// `%ProgramFiles%`, and that every file its registration loads grants
/// read and execute to both package SIDs.
// [spec:kbdgen:req:tsf.register.upgrade+1]
// [spec:kbdgen:req:tsf.security.appcontainer+2]
fn verify(plan: &Plan) -> Result<()> {
    place(&module_path()?, &program_files()?, VERSION)?;
    for file in &plan.files {
        if !ungranted(dacl(file)?.as_deref()).is_empty() {
            return Err(Refusal::Access(file.clone()).into());
        }
    }
    Ok(())
}

/// Registers the CLSID, as an apartment-threaded in-process server at the
/// planned path, and, where this DLL owns them, the text service's
/// categories.
// [spec:kbdgen:req:tsf.register.server]
// [spec:kbdgen:req:tsf.arch.registration+1]
pub fn register() -> Result<()> {
    let plan = plan(&module_path()?, OWN, native_machine()?)?;
    verify(&plan)?;
    let key = class_key();
    write_key(&key, &[(None, "Divvun keyboard text service")])?;
    write_key(
        &format!("{key}\\InprocServer32"),
        &[(None, &plan.server), (Some("ThreadingModel"), "Apartment")],
    )?;
    if !plan.categories {
        return Ok(());
    }
    let clsid = GUID::from_u128(CLSID);
    with_categories(|manager| {
        CATEGORIES
            .iter()
            .try_for_each(|category| unsafe { manager.RegisterCategory(&clsid, category, &clsid) })
    })
}

/// Removes what [`register`] added, unless `InprocServer32` names another
/// server, as after an upgrade to a newer version. `UnregisterCategory`
/// leaves the text service's empty keys in `CTF\TIP`; the DLL that owns
/// the categories deletes them when no language profile remains there.
// [spec:kbdgen:req:tsf.register.server]
// [spec:kbdgen:req:tsf.register.upgrade+1]
pub fn unregister() -> Result<()> {
    let plan = plan(&module_path()?, OWN, native_machine()?)?;
    let key = class_key();
    let server = format!("{key}\\InprocServer32");
    let registered = read_string(&server, None, REG_SAM_FLAGS(0))?;
    if registered.is_some_and(|path| !path.eq_ignore_ascii_case(&plan.server)) {
        return Ok(());
    }
    let clsid = GUID::from_u128(CLSID);
    let categories = if plan.categories {
        with_categories(|manager| {
            for category in &CATEGORIES {
                let _ = unsafe { manager.UnregisterCategory(&clsid, category, &clsid) };
            }
            Ok(())
        })
    } else {
        Ok(())
    };
    delete_tree(&key)?;
    categories?;
    let tip = format!("SOFTWARE\\Microsoft\\CTF\\TIP\\{}", braced(CLSID));
    if plan.categories && !profiles_remain(&format!("{tip}\\LanguageProfile"))? {
        delete_tree(&tip)?;
    }
    Ok(())
}

/// Deletes the HKLM key `path` with its subkeys, if it exists.
fn delete_tree(path: &str) -> Result<()> {
    let path = wide(path);
    let status = unsafe { RegDeleteTreeW(HKEY_LOCAL_MACHINE, PCWSTR(path.as_ptr())) };
    if status == ERROR_FILE_NOT_FOUND {
        Ok(())
    } else {
        check(status)
    }
}
