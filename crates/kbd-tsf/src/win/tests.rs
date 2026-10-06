//! COM-level tests, on Windows only: the exports, the interfaces the text
//! service object answers to, the display attribute, and a forced panic in
//! every entry point (`tsf.test.host`).

use std::ffi::c_void;
use std::sync::atomic::Ordering;

use windows::Win32::Foundation::{
    CLASS_E_CLASSNOTAVAILABLE, E_NOTIMPL, E_UNEXPECTED, LPARAM, S_FALSE, WPARAM,
};
use windows::Win32::System::Com::IClassFactory;
use windows::Win32::UI::TextServices::{
    IEnumTfContextViews, IEnumTfProperties, ITfActiveLanguageProfileNotifySink,
    ITfCompartmentEventSink, ITfCompositionSink, ITfContext, ITfContext_Impl, ITfContextView,
    ITfDisplayAttributeInfo, ITfDisplayAttributeProvider, ITfDocumentMgr, ITfEditSession,
    ITfKeyEventSink, ITfProperty, ITfRange, ITfRangeBackup, ITfReadOnlyProperty, ITfTextEditSink,
    ITfTextInputProcessor, ITfTextInputProcessorEx, ITfThreadMgrEventSink,
    TF_CONTEXT_EDIT_CONTEXT_FLAGS, TF_DISPLAYATTRIBUTE, TF_LS_DOT, TF_SELECTION, TS_STATUS,
};
use windows_core::{BOOL, GUID, HRESULT, IUnknown, Interface, Ref, Result, implement};

use crate::guard::{ENTRIES, Entry, POISON, faults, poisoned};
use crate::guid::{CLSID, PREEDIT_ATTRIBUTE};

use super::exports::{
    DllCanUnloadNow, DllGetClassObject, DllRegisterServer, DllUnregisterServer, factory,
};
use super::tip::Tip;

fn code(result: Result<()>) -> HRESULT {
    result.map_or_else(|e| e.code(), |()| HRESULT(0))
}

fn tip<I: Interface>() -> I {
    let unknown: IUnknown = Tip::new().into();
    unknown.cast().unwrap()
}

/// Calls entry point `entry` with arguments that are never read, because
/// an armed fault panics first.
fn call(entry: Entry) -> HRESULT {
    let guid = GUID::from_u128(PREEDIT_ATTRIBUTE);
    let null = std::ptr::null_mut();
    let w = WPARAM(0);
    let l = LPARAM(0);
    unsafe {
        match entry {
            Entry::DllGetClassObject => {
                let mut out: *mut c_void = null;
                DllGetClassObject(&guid, &IClassFactory::IID, &mut out)
            }
            Entry::DllCanUnloadNow => DllCanUnloadNow(),
            Entry::DllRegisterServer => DllRegisterServer(),
            Entry::DllUnregisterServer => DllUnregisterServer(),
            Entry::CreateInstance => code(factory().CreateInstance::<_, IUnknown>(None).map(drop)),
            Entry::LockServer => code(factory().LockServer(true)),
            Entry::Activate => code(tip::<ITfTextInputProcessor>().Activate(None, 0)),
            Entry::ActivateEx => code(tip::<ITfTextInputProcessorEx>().ActivateEx(None, 0, 0)),
            Entry::Deactivate => code(tip::<ITfTextInputProcessor>().Deactivate()),
            Entry::OnInitDocumentMgr => {
                code(tip::<ITfThreadMgrEventSink>().OnInitDocumentMgr(None))
            }
            Entry::OnUninitDocumentMgr => {
                code(tip::<ITfThreadMgrEventSink>().OnUninitDocumentMgr(None))
            }
            Entry::OnSetFocus => code(tip::<ITfThreadMgrEventSink>().OnSetFocus(None, None)),
            Entry::OnPushContext => code(tip::<ITfThreadMgrEventSink>().OnPushContext(None)),
            Entry::OnPopContext => code(tip::<ITfThreadMgrEventSink>().OnPopContext(None)),
            Entry::OnKeySetFocus => code(tip::<ITfKeyEventSink>().OnSetFocus(true)),
            Entry::OnTestKeyDown => {
                code(tip::<ITfKeyEventSink>().OnTestKeyDown(None, w, l).map(drop))
            }
            Entry::OnTestKeyUp => code(tip::<ITfKeyEventSink>().OnTestKeyUp(None, w, l).map(drop)),
            Entry::OnKeyDown => code(tip::<ITfKeyEventSink>().OnKeyDown(None, w, l).map(drop)),
            Entry::OnKeyUp => code(tip::<ITfKeyEventSink>().OnKeyUp(None, w, l).map(drop)),
            Entry::OnPreservedKey => code(
                tip::<ITfKeyEventSink>()
                    .OnPreservedKey(None, &guid)
                    .map(drop),
            ),
            Entry::OnEndEdit => code(tip::<ITfTextEditSink>().OnEndEdit(None, 0, None)),
            Entry::OnCompositionTerminated => {
                code(tip::<ITfCompositionSink>().OnCompositionTerminated(0, None))
            }
            Entry::OnCompartmentChange => code(tip::<ITfCompartmentEventSink>().OnChange(&guid)),
            Entry::OnActivated => {
                code(tip::<ITfActiveLanguageProfileNotifySink>().OnActivated(&guid, &guid, false))
            }
            Entry::EnumDisplayAttributeInfo => code(
                tip::<ITfDisplayAttributeProvider>()
                    .EnumDisplayAttributeInfo()
                    .map(drop),
            ),
            Entry::GetDisplayAttributeInfo => code(
                tip::<ITfDisplayAttributeProvider>()
                    .GetDisplayAttributeInfo(&guid)
                    .map(drop),
            ),
            Entry::DoEditSession => {
                code(super::session::test_session(NullContext.into()).DoEditSession(0))
            }
            Entry::InfoGetGuid => code(info().GetGUID().map(drop)),
            Entry::InfoGetDescription => code(info().GetDescription().map(drop)),
            Entry::InfoGetAttributeInfo => {
                let mut attribute = TF_DISPLAYATTRIBUTE::default();
                code(info().GetAttributeInfo(&mut attribute))
            }
            Entry::InfoSetAttributeInfo => {
                code(info().SetAttributeInfo(&TF_DISPLAYATTRIBUTE::default()))
            }
            Entry::InfoReset => code(info().Reset()),
            Entry::EnumClone => code(attributes().Clone().map(drop)),
            Entry::EnumNext => {
                let mut infos = [None];
                let mut fetched = 0;
                code(attributes().Next(&mut infos, &mut fetched))
            }
            Entry::EnumReset => code(attributes().Reset()),
            Entry::EnumSkip => code(attributes().Skip(1)),
        }
    }
}

fn info() -> ITfDisplayAttributeInfo {
    let provider = tip::<ITfDisplayAttributeProvider>();
    unsafe { provider.GetDisplayAttributeInfo(&GUID::from_u128(PREEDIT_ATTRIBUTE)) }.unwrap()
}

fn attributes() -> windows::Win32::UI::TextServices::IEnumTfDisplayAttributeInfo {
    unsafe { tip::<ITfDisplayAttributeProvider>().EnumDisplayAttributeInfo() }.unwrap()
}

/// A context that supports nothing, for edit sessions that never use it.
#[implement(ITfContext)]
struct NullContext;

impl ITfContext_Impl for NullContext_Impl {
    fn RequestEditSession(
        &self,
        _tid: u32,
        _pes: Ref<ITfEditSession>,
        _dwflags: TF_CONTEXT_EDIT_CONTEXT_FLAGS,
    ) -> Result<HRESULT> {
        Err(E_NOTIMPL.into())
    }

    fn InWriteSession(&self, _tid: u32) -> Result<BOOL> {
        Err(E_NOTIMPL.into())
    }

    fn GetSelection(
        &self,
        _ec: u32,
        _ulindex: u32,
        _ulcount: u32,
        _pselection: *mut TF_SELECTION,
        _pcfetched: *mut u32,
    ) -> Result<()> {
        Err(E_NOTIMPL.into())
    }

    fn SetSelection(
        &self,
        _ec: u32,
        _ulcount: u32,
        _pselection: *const TF_SELECTION,
    ) -> Result<()> {
        Err(E_NOTIMPL.into())
    }

    fn GetStart(&self, _ec: u32) -> Result<ITfRange> {
        Err(E_NOTIMPL.into())
    }

    fn GetEnd(&self, _ec: u32) -> Result<ITfRange> {
        Err(E_NOTIMPL.into())
    }

    fn GetActiveView(&self) -> Result<ITfContextView> {
        Err(E_NOTIMPL.into())
    }

    fn EnumViews(&self) -> Result<IEnumTfContextViews> {
        Err(E_NOTIMPL.into())
    }

    fn GetStatus(&self) -> Result<TS_STATUS> {
        Err(E_NOTIMPL.into())
    }

    fn GetProperty(&self, _guidprop: *const GUID) -> Result<ITfProperty> {
        Err(E_NOTIMPL.into())
    }

    fn GetAppProperty(&self, _guidprop: *const GUID) -> Result<ITfReadOnlyProperty> {
        Err(E_NOTIMPL.into())
    }

    fn TrackProperties(
        &self,
        _prgprop: *const *const GUID,
        _cprop: u32,
        _prgappprop: *const *const GUID,
        _cappprop: u32,
    ) -> Result<ITfReadOnlyProperty> {
        Err(E_NOTIMPL.into())
    }

    fn EnumProperties(&self) -> Result<IEnumTfProperties> {
        Err(E_NOTIMPL.into())
    }

    fn GetDocumentMgr(&self) -> Result<ITfDocumentMgr> {
        Err(E_NOTIMPL.into())
    }

    fn CreateRangeBackup(&self, _ec: u32, _prange: Ref<ITfRange>) -> Result<ITfRangeBackup> {
        Err(E_NOTIMPL.into())
    }
}

// [spec:kbdgen:req:tsf.test.host/test]
// [spec:kbdgen:req:tsf.component.panic/test]
#[test]
fn forced_panic_in_every_com_entry_point() {
    for entry in ENTRIES {
        POISON.store(false, Ordering::SeqCst);
        faults::arm(entry);
        assert_eq!(call(entry), E_UNEXPECTED, "{entry:?}");
        assert!(poisoned(), "{entry:?} poisons the service");
    }
    POISON.store(false, Ordering::SeqCst);
}

// [spec:kbdgen:req:tsf.component.crate/test]
#[test]
fn class_object_only_for_own_clsid() {
    let mut out: *mut c_void = std::ptr::null_mut();
    let other = GUID::from_u128(CLSID ^ 1);
    let hr = unsafe { DllGetClassObject(&other, &IClassFactory::IID, &mut out) };
    assert_eq!(hr, CLASS_E_CLASSNOTAVAILABLE);
    assert!(out.is_null());
    let clsid = GUID::from_u128(CLSID);
    let hr = unsafe { DllGetClassObject(&clsid, &IClassFactory::IID, &mut out) };
    assert!(hr.is_ok());
    let factory = unsafe { IClassFactory::from_raw(out) };
    assert_eq!(DllCanUnloadNow(), S_FALSE);
    let created: IUnknown = unsafe { factory.CreateInstance(None) }.unwrap();
    drop(factory);
    created.cast::<ITfTextInputProcessorEx>().unwrap();
}

// [spec:kbdgen:req:tsf.component.interfaces/test]
#[test]
fn text_service_answers_every_required_interface() {
    let unknown: IUnknown = Tip::new().into();
    unknown.cast::<ITfTextInputProcessor>().unwrap();
    unknown.cast::<ITfTextInputProcessorEx>().unwrap();
    unknown.cast::<ITfThreadMgrEventSink>().unwrap();
    unknown.cast::<ITfKeyEventSink>().unwrap();
    unknown.cast::<ITfTextEditSink>().unwrap();
    unknown.cast::<ITfCompositionSink>().unwrap();
    unknown.cast::<ITfCompartmentEventSink>().unwrap();
    unknown.cast::<ITfDisplayAttributeProvider>().unwrap();
}

// [spec:kbdgen:req:tsf.edit.preedit/test]
#[test]
fn one_dotted_preedit_attribute_is_enumerated() {
    let attributes = attributes();
    let mut infos = [None, None];
    let mut fetched = 0;
    unsafe { attributes.Next(&mut infos, &mut fetched) }.unwrap();
    assert_eq!(fetched, 1);
    let [Some(info), None] = infos else {
        panic!("one attribute");
    };
    assert_eq!(
        unsafe { info.GetGUID() }.unwrap(),
        GUID::from_u128(PREEDIT_ATTRIBUTE)
    );
    let mut attribute = TF_DISPLAYATTRIBUTE::default();
    unsafe { info.GetAttributeInfo(&mut attribute) }.unwrap();
    assert_eq!(attribute.lsStyle, TF_LS_DOT);
}

// [spec:kbdgen:req:tsf.security.secure-mode/test]
#[test]
fn registration_declares_secure_mode_support() {
    use windows::Win32::UI::TextServices::{
        GUID_TFCAT_TIPCAP_COMLESS, GUID_TFCAT_TIPCAP_SECUREMODE,
    };
    let categories = super::register::CATEGORIES;
    assert!(categories.contains(&GUID_TFCAT_TIPCAP_SECUREMODE));
    assert!(!categories.contains(&GUID_TFCAT_TIPCAP_COMLESS));
}
