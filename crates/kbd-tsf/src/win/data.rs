//! Reading a profile's keyboard data (`tsf.data.locate`): the layout under
//! `HKLM\SYSTEM\CurrentControlSet\Control\Keyboard Layouts` whose
//! `Layout Product Code` is the profile GUID, and the model resource of its
//! `Layout File`, loaded from the system directory as a resource-only
//! image. Nothing is written, and results are cached per process.

use std::sync::Arc;

use windows::Win32::Foundation::{ERROR_SUCCESS, FreeLibrary, HMODULE};
use windows::Win32::System::LibraryLoader::{
    FindResourceW, LOAD_LIBRARY_AS_DATAFILE, LOAD_LIBRARY_AS_IMAGE_RESOURCE, LoadLibraryExW,
    LoadResource, LockResource, SizeofResource,
};
use windows::Win32::System::Registry::{
    HKEY, HKEY_LOCAL_MACHINE, KEY_READ, RRF_RT_REG_SZ, RegCloseKey, RegEnumKeyExW, RegGetValueW,
    RegOpenKeyExW,
};
use windows::Win32::System::SystemInformation::GetSystemDirectoryW;
use windows_core::{PCWSTR, PWSTR, w};

use crate::locate::{Keyboard, Keyboards, is_layout_file, names_profile};

static KEYBOARDS: Keyboards = Keyboards::new();

const RT_RCDATA: PCWSTR = PCWSTR(10 as _);
/// The model's resource name, `MAKEINTRESOURCE(1)` (`tsf.data.resource`).
const MODEL_NAME: PCWSTR = PCWSTR(1 as _);

/// The keyboard of `profile`, or `None` when the profile is inert.
// [spec:kbdgen:req:tsf.data.locate]
// [spec:kbdgen:req:tsf.component.self-contained]
// [spec:kbdgen:req:tsf.security.secure-mode+1]
pub fn keyboard(profile: u128) -> Option<Arc<Keyboard>> {
    KEYBOARDS.get(profile, || {
        let file = layout_file(profile)?;
        model_resource(&file)
    })
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain([0]).collect()
}

/// A `REG_SZ` value of subkey `subkey` (NUL-terminated) of `root`.
fn string_value(root: HKEY, subkey: &[u16], value: &str) -> Option<String> {
    let value = wide(value);
    let mut size = 0u32;
    let status = unsafe {
        RegGetValueW(
            root,
            PCWSTR(subkey.as_ptr()),
            PCWSTR(value.as_ptr()),
            RRF_RT_REG_SZ,
            None,
            None,
            Some(&mut size),
        )
    };
    if status != ERROR_SUCCESS {
        return None;
    }
    let mut buffer = vec![0u16; usize::try_from(size).ok()?.div_ceil(2)];
    let status = unsafe {
        RegGetValueW(
            root,
            PCWSTR(subkey.as_ptr()),
            PCWSTR(value.as_ptr()),
            RRF_RT_REG_SZ,
            None,
            Some(buffer.as_mut_ptr().cast()),
            Some(&mut size),
        )
    };
    if status != ERROR_SUCCESS {
        return None;
    }
    let end = buffer.iter().position(|&u| u == 0).unwrap_or(buffer.len());
    String::from_utf16(buffer.get(..end)?).ok()
}

fn subkey(root: HKEY, index: u32) -> Option<Vec<u16>> {
    let mut name = vec![0u16; 256];
    let mut len = 256u32;
    let status = unsafe {
        RegEnumKeyExW(
            root,
            index,
            Some(PWSTR(name.as_mut_ptr())),
            &mut len,
            None,
            None,
            None,
            None,
        )
    };
    (status == ERROR_SUCCESS).then(|| {
        name.truncate(usize::try_from(len).unwrap_or_default());
        name.push(0);
        name
    })
}

/// The `Layout File` of the layout whose `Layout Product Code` names
/// `profile`.
fn layout_file(profile: u128) -> Option<String> {
    let mut root = HKEY::default();
    let status = unsafe {
        RegOpenKeyExW(
            HKEY_LOCAL_MACHINE,
            w!("SYSTEM\\CurrentControlSet\\Control\\Keyboard Layouts"),
            Some(0),
            KEY_READ,
            &mut root,
        )
    };
    if status != ERROR_SUCCESS {
        return None;
    }
    let file = (0..)
        .map_while(|index| subkey(root, index))
        .find(|name| {
            string_value(root, name, "Layout Product Code")
                .is_some_and(|code| names_profile(&code, profile))
        })
        .and_then(|name| string_value(root, &name, "Layout File"));
    let _ = unsafe { RegCloseKey(root) };
    file.filter(|file| is_layout_file(file))
}

fn resource_bytes(module: HMODULE) -> Option<Vec<u8>> {
    let info = unsafe { FindResourceW(Some(module), MODEL_NAME, RT_RCDATA) };
    if info.is_invalid() {
        return None;
    }
    let size = usize::try_from(unsafe { SizeofResource(Some(module), info) }).ok()?;
    let data = unsafe { LoadResource(Some(module), info) }.ok()?;
    let pointer = unsafe { LockResource(data) };
    if pointer.is_null() || size == 0 {
        return None;
    }
    let bytes = unsafe { std::slice::from_raw_parts(pointer.cast::<u8>(), size) };
    Some(bytes.to_vec())
}

/// The model resource of layout DLL `file`, loaded from the system
/// directory with `LOAD_LIBRARY_AS_DATAFILE | LOAD_LIBRARY_AS_IMAGE_RESOURCE`.
/// In a 32-bit process on 64-bit Windows the file system redirects this to
/// the `SysWOW64` variant, which carries identical data.
fn model_resource(file: &str) -> Option<Vec<u8>> {
    let mut directory = vec![0u16; 260];
    let len = usize::try_from(unsafe { GetSystemDirectoryW(Some(&mut directory)) }).ok()?;
    if len == 0 || len >= directory.len() {
        return None;
    }
    directory.truncate(len);
    let mut path = String::from_utf16(&directory).ok()?;
    path.push('\\');
    path.push_str(file);
    let path = wide(&path);
    let module = unsafe {
        LoadLibraryExW(
            PCWSTR(path.as_ptr()),
            None,
            LOAD_LIBRARY_AS_DATAFILE | LOAD_LIBRARY_AS_IMAGE_RESOURCE,
        )
    }
    .ok()?;
    let bytes = resource_bytes(module);
    let _ = unsafe { FreeLibrary(module) };
    bytes
}
