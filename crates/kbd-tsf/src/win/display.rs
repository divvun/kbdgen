//! The display attribute provider: one attribute, the dotted underline of
//! the preedit (`tsf.edit.preedit`).

use std::cell::Cell;

use windows::Win32::Foundation::{E_INVALIDARG, E_NOTIMPL, S_FALSE};
use windows::Win32::UI::TextServices::{
    IEnumTfDisplayAttributeInfo, IEnumTfDisplayAttributeInfo_Impl, ITfDisplayAttributeInfo,
    ITfDisplayAttributeInfo_Impl, ITfDisplayAttributeProvider_Impl, TF_ATTR_INPUT, TF_CT_NONE,
    TF_DA_COLOR, TF_DISPLAYATTRIBUTE, TF_LS_DOT,
};
use windows_core::{BSTR, GUID, Result, implement};

use crate::guard::{Entry, POISON, contain};
use crate::guid::PREEDIT_ATTRIBUTE;
use crate::server::Live;

use super::tip::Tip_Impl;

#[implement(ITfDisplayAttributeInfo)]
struct PreeditInfo {
    _live: Live,
}

impl PreeditInfo {
    fn interface() -> ITfDisplayAttributeInfo {
        PreeditInfo {
            _live: Live::default(),
        }
        .into()
    }
}

impl ITfDisplayAttributeInfo_Impl for PreeditInfo_Impl {
    fn GetGUID(&self) -> Result<GUID> {
        contain(&POISON, Entry::InfoGetGuid, || {
            Ok(GUID::from_u128(PREEDIT_ATTRIBUTE))
        })
    }

    fn GetDescription(&self) -> Result<BSTR> {
        contain(&POISON, Entry::InfoGetDescription, || {
            Ok(BSTR::from("Divvun keyboard pending input"))
        })
    }

    // [spec:kbdgen:req:tsf.edit.preedit+2]
    fn GetAttributeInfo(&self, pda: *mut TF_DISPLAYATTRIBUTE) -> Result<()> {
        contain(&POISON, Entry::InfoGetAttributeInfo, || {
            let none = TF_DA_COLOR {
                r#type: TF_CT_NONE,
                ..TF_DA_COLOR::default()
            };
            let attribute = TF_DISPLAYATTRIBUTE {
                crText: none,
                crBk: none,
                lsStyle: TF_LS_DOT,
                fBoldLine: false.into(),
                crLine: none,
                bAttr: TF_ATTR_INPUT,
            };
            let out = unsafe { pda.as_mut() }.ok_or(E_INVALIDARG)?;
            *out = attribute;
            Ok(())
        })
    }

    fn SetAttributeInfo(&self, _pda: *const TF_DISPLAYATTRIBUTE) -> Result<()> {
        contain(&POISON, Entry::InfoSetAttributeInfo, || {
            Err(E_NOTIMPL.into())
        })
    }

    fn Reset(&self) -> Result<()> {
        contain(&POISON, Entry::InfoReset, || Ok(()))
    }
}

/// An enumerator over the one display attribute.
#[implement(IEnumTfDisplayAttributeInfo)]
struct PreeditEnum {
    _live: Live,
    next: Cell<u32>,
}

impl PreeditEnum {
    fn interface(next: u32) -> IEnumTfDisplayAttributeInfo {
        PreeditEnum {
            _live: Live::default(),
            next: Cell::new(next),
        }
        .into()
    }
}

impl IEnumTfDisplayAttributeInfo_Impl for PreeditEnum_Impl {
    fn Clone(&self) -> Result<IEnumTfDisplayAttributeInfo> {
        contain(&POISON, Entry::EnumClone, || {
            Ok(PreeditEnum::interface(self.next.get()))
        })
    }

    fn Next(
        &self,
        ulcount: u32,
        rginfo: *mut Option<ITfDisplayAttributeInfo>,
        pcfetched: *mut u32,
    ) -> Result<()> {
        contain(&POISON, Entry::EnumNext, || {
            let give = ulcount > 0 && self.next.get() == 0 && !rginfo.is_null();
            if give {
                unsafe { rginfo.write(Some(PreeditInfo::interface())) };
                self.next.set(1);
            }
            if let Some(fetched) = unsafe { pcfetched.as_mut() } {
                *fetched = u32::from(give);
            }
            if give || ulcount == 0 {
                Ok(())
            } else {
                Err(S_FALSE.into())
            }
        })
    }

    fn Reset(&self) -> Result<()> {
        contain(&POISON, Entry::EnumReset, || {
            self.next.set(0);
            Ok(())
        })
    }

    fn Skip(&self, ulcount: u32) -> Result<()> {
        contain(&POISON, Entry::EnumSkip, || {
            let skipped = self.next.get().saturating_add(ulcount);
            self.next.set(skipped.min(1));
            if skipped <= 1 {
                Ok(())
            } else {
                Err(S_FALSE.into())
            }
        })
    }
}

impl ITfDisplayAttributeProvider_Impl for Tip_Impl {
    fn EnumDisplayAttributeInfo(&self) -> Result<IEnumTfDisplayAttributeInfo> {
        contain(&POISON, Entry::EnumDisplayAttributeInfo, || {
            Ok(PreeditEnum::interface(0))
        })
    }

    fn GetDisplayAttributeInfo(&self, guid: *const GUID) -> Result<ITfDisplayAttributeInfo> {
        contain(&POISON, Entry::GetDisplayAttributeInfo, || {
            match unsafe { guid.as_ref() } {
                Some(guid) if guid.to_u128() == PREEDIT_ATTRIBUTE => Ok(PreeditInfo::interface()),
                _ => Err(E_INVALIDARG.into()),
            }
        })
    }
}
