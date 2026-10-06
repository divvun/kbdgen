//! The DLL's exports, exactly `DllGetClassObject`, `DllCanUnloadNow`,
//! `DllRegisterServer` and `DllUnregisterServer`, and the class factory
//! (`tsf.component.crate`).

use std::ffi::c_void;

use windows::Win32::Foundation::{
    CLASS_E_CLASSNOTAVAILABLE, CLASS_E_NOAGGREGATION, E_POINTER, S_FALSE, S_OK,
};
use windows::Win32::System::Com::{IClassFactory, IClassFactory_Impl};
use windows_core::{BOOL, GUID, HRESULT, IUnknown, Interface, Ref, Result, implement};

use crate::guard::{Entry, POISON, contain};
use crate::guid::CLSID;
use crate::server::{Live, can_unload, lock};

use super::register;
use super::tip::Tip;

#[implement(IClassFactory)]
struct Factory {
    _live: Live,
}

impl IClassFactory_Impl for Factory_Impl {
    fn CreateInstance(
        &self,
        outer: Ref<IUnknown>,
        riid: *const GUID,
        ppvobject: *mut *mut c_void,
    ) -> Result<()> {
        contain(&POISON, Entry::CreateInstance, || {
            if ppvobject.is_null() {
                return Err(E_POINTER.into());
            }
            unsafe { ppvobject.write(std::ptr::null_mut()) };
            if !outer.is_null() {
                return Err(CLASS_E_NOAGGREGATION.into());
            }
            let tip: IUnknown = Tip::new().into();
            unsafe { tip.query(riid, ppvobject) }.ok()
        })
    }

    fn LockServer(&self, flock: BOOL) -> Result<()> {
        contain(&POISON, Entry::LockServer, || {
            lock(flock.as_bool());
            Ok(())
        })
    }
}

fn hresult(result: Result<HRESULT>) -> HRESULT {
    result.unwrap_or_else(|error| error.code())
}

/// Returns the class factory for the text service's CLSID, and
/// `CLASS_E_CLASSNOTAVAILABLE` for any other.
// [spec:kbdgen:req:tsf.component.crate]
// [spec:kbdgen:req:tsf.component.panic]
#[unsafe(no_mangle)]
pub unsafe extern "system" fn DllGetClassObject(
    rclsid: *const GUID,
    riid: *const GUID,
    ppv: *mut *mut c_void,
) -> HRESULT {
    hresult(contain(&POISON, Entry::DllGetClassObject, || {
        if ppv.is_null() {
            return Ok(E_POINTER);
        }
        unsafe { ppv.write(std::ptr::null_mut()) };
        let Some(clsid) = (unsafe { rclsid.as_ref() }) else {
            return Ok(E_POINTER);
        };
        if clsid.to_u128() != CLSID {
            return Ok(CLASS_E_CLASSNOTAVAILABLE);
        }
        let factory: IUnknown = Factory {
            _live: Live::default(),
        }
        .into();
        Ok(unsafe { factory.query(riid, ppv) })
    }))
}

/// `S_OK` only when no object and no server lock is alive.
// [spec:kbdgen:req:tsf.component.crate]
#[unsafe(no_mangle)]
pub extern "system" fn DllCanUnloadNow() -> HRESULT {
    hresult(contain(&POISON, Entry::DllCanUnloadNow, || {
        Ok(if can_unload() { S_OK } else { S_FALSE })
    }))
}

#[unsafe(no_mangle)]
pub extern "system" fn DllRegisterServer() -> HRESULT {
    hresult(contain(&POISON, Entry::DllRegisterServer, || {
        register::register().map(|()| S_OK)
    }))
}

#[unsafe(no_mangle)]
pub extern "system" fn DllUnregisterServer() -> HRESULT {
    hresult(contain(&POISON, Entry::DllUnregisterServer, || {
        register::unregister().map(|()| S_OK)
    }))
}

#[cfg(test)]
pub(super) fn factory() -> IClassFactory {
    Factory {
        _live: Live::default(),
    }
    .into()
}
