//! The text input processor object, its per-thread state, and activation.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyboardLayout, MAPVK_VSC_TO_VK_EX, MapVirtualKeyExW,
};
use windows::Win32::UI::TextServices::{
    CLSID_TF_CategoryMgr, CLSID_TF_InputProcessorProfiles, GUID_TFCAT_TIP_KEYBOARD,
    ITfActiveLanguageProfileNotifySink, ITfCategoryMgr, ITfComposition, ITfCompositionSink,
    ITfContext, ITfInputProcessorProfileMgr, ITfKeyEventSink, ITfKeystrokeMgr, ITfSource,
    ITfTextInputProcessor_Impl, ITfTextInputProcessorEx_Impl, ITfThreadMgr, ITfThreadMgrEventSink,
    ITfThreadMgrEx, TF_INPUTPROCESSORPROFILE, TF_MOD_RALT, TF_MOD_SHIFT, TF_PRESERVEDKEY,
};
use windows_core::{GUID, IUnknown, IUnknownImpl, Interface, Ref, Result, implement};

use crate::claim::Claims;
use crate::document::Document;
use crate::guard::{Entry, POISON, contain};
use crate::guid::{CLSID, PREEDIT_ATTRIBUTE};
use crate::keys::{Chord, Tracker, altgr_chords};
use crate::locate::Keyboard;
use crate::server::Live;

use super::data;
use super::session::{Pending, Task};

/// The text service object: one per thread on which TSF activates it.
#[implement(
    windows::Win32::UI::TextServices::ITfTextInputProcessorEx,
    windows::Win32::UI::TextServices::ITfThreadMgrEventSink,
    windows::Win32::UI::TextServices::ITfKeyEventSink,
    windows::Win32::UI::TextServices::ITfTextEditSink,
    windows::Win32::UI::TextServices::ITfCompositionSink,
    windows::Win32::UI::TextServices::ITfCompartmentEventSink,
    windows::Win32::UI::TextServices::ITfDisplayAttributeProvider,
    windows::Win32::UI::TextServices::ITfActiveLanguageProfileNotifySink
)]
// [spec:kbdgen:req:tsf.component.interfaces+2]
pub struct Tip {
    _live: Live,
    pub(super) shared: Rc<RefCell<Inner>>,
}

impl Tip {
    pub fn new() -> Tip {
        Tip {
            _live: Live::default(),
            shared: Rc::new(RefCell::new(Inner::default())),
        }
    }
}

/// An advised sink: the source and its cookie.
pub(super) struct Advice {
    pub source: ITfSource,
    pub cookie: u32,
}

impl Advice {
    pub fn unadvise(self) {
        let _ = unsafe { self.source.UnadviseSink(self.cookie) };
    }
}

/// The text service's state on one thread.
// [spec:kbdgen:req:tsf.edit.own]
#[derive(Default)]
pub(super) struct Inner {
    pub thread_mgr: Option<ITfThreadMgr>,
    pub client: u32,
    /// `ITfThreadMgrEx::GetActiveFlags` at activation: `TF_TMF_SECUREMODE`,
    /// `TF_TMF_IMMERSIVEMODE` and the rest.
    pub active_flags: u32,
    pub keyboard: Option<Arc<Keyboard>>,
    pub document: Document,
    /// The context `document` belongs to.
    pub context: Option<ITfContext>,
    pub composition: Option<ITfComposition>,
    /// The `GUID_PROP_ATTRIBUTE` atom of the preedit's display attribute.
    pub attribute: Option<u32>,
    pub tracker: Tracker,
    pub claims: Claims,
    pub pending: Option<Pending>,
    /// Inside a request for one of the text service's own edit sessions.
    pub own_edit: bool,
    /// Input the text service sent may not have reached the application
    /// yet; edits it causes are the text service's own.
    pub injected: bool,
    /// Edit notifications still due for `SetText` edits the text service
    /// made in a transitory context. TSF mirrors the application's own
    /// change back into such a context after the edit session ends, as an
    /// edit of the application's.
    pub echoes: u32,
    pub thread_sinks: Vec<Advice>,
    pub context_sinks: Vec<Advice>,
    /// The AltGr chords registered as preserved keys.
    pub preserved: Vec<(GUID, TF_PRESERVEDKEY)>,
}

impl Inner {
    /// Makes `document` the one of `context`, discarding the state of any
    /// other context (`tsf.edit.reset`).
    pub fn switch(&mut self, context: &ITfContext) {
        if !self.context.as_ref().is_some_and(|c| same(c, context)) {
            self.document = Document::default();
            self.composition = None;
            self.context = Some(context.clone());
        }
    }
}

/// COM identity: whether two interface pointers are one object.
pub(super) fn same<A: Interface, B: Interface>(a: &A, b: &B) -> bool {
    match (a.cast::<IUnknown>(), b.cast::<IUnknown>()) {
        (Ok(a), Ok(b)) => a.as_raw() == b.as_raw(),
        _ => false,
    }
}

/// The GUID of this text service's active keyboard profile, if one is
/// active.
fn active_profile() -> Option<u128> {
    let profiles: ITfInputProcessorProfileMgr =
        unsafe { CoCreateInstance(&CLSID_TF_InputProcessorProfiles, None, CLSCTX_INPROC_SERVER) }
            .ok()?;
    let mut profile = TF_INPUTPROCESSORPROFILE::default();
    unsafe { profiles.GetActiveProfile(&GUID_TFCAT_TIP_KEYBOARD, &mut profile) }.ok()?;
    (profile.clsid.to_u128() == CLSID).then(|| profile.guidProfile.to_u128())
}

impl Tip_Impl {
    /// Loads the keyboard of `profile`, or makes the service inert, and
    /// resets.
    // [spec:kbdgen:req:tsf.data.locate+1]
    pub(super) fn select_profile(&self, profile: Option<u128>) {
        let keyboard = profile.and_then(data::keyboard);
        self.unpreserve();
        if let Some(keyboard) = keyboard.as_ref().filter(|k| k.altgr) {
            self.preserve(&altgr_chords(&keyboard.model));
        }
        if let Ok(mut inner) = self.shared.try_borrow_mut() {
            inner.keyboard = keyboard;
        }
        self.reset_document();
    }

    /// Registers `chords` as preserved keys, each on the virtual key that
    /// the thread's dummy layout gives its scan code.
    // [spec:kbdgen:req:tsf.keys.altgr+1]
    // [spec:kbdgen:req:tsf.keys.preserved]
    fn preserve(&self, chords: &[Chord]) {
        let Ok(mut inner) = self.shared.try_borrow_mut() else {
            return;
        };
        let Some(keys) = inner
            .thread_mgr
            .as_ref()
            .and_then(|tm| tm.cast::<ITfKeystrokeMgr>().ok())
        else {
            return;
        };
        let layout = unsafe { GetKeyboardLayout(0) };
        for chord in chords {
            let vk = unsafe {
                MapVirtualKeyExW(u32::from(chord.scan), MAPVK_VSC_TO_VK_EX, Some(layout))
            };
            if vk == 0 {
                continue;
            }
            let modifiers = TF_MOD_RALT | if chord.shift { TF_MOD_SHIFT } else { 0 };
            let key = TF_PRESERVEDKEY {
                uVKey: vk,
                uModifiers: modifiers,
            };
            let guid = GUID::from_u128(chord.guid());
            if unsafe { keys.PreserveKey(inner.client, &guid, &key, &[]) }.is_ok() {
                inner.preserved.push((guid, key));
            }
        }
    }

    fn unpreserve(&self) {
        let Ok(mut inner) = self.shared.try_borrow_mut() else {
            return;
        };
        let preserved = std::mem::take(&mut inner.preserved);
        let keys = inner
            .thread_mgr
            .as_ref()
            .and_then(|tm| tm.cast::<ITfKeystrokeMgr>().ok());
        if let Some(keys) = keys {
            for (guid, key) in &preserved {
                let _ = unsafe { keys.UnpreserveKey(guid, key) };
            }
        }
    }

    /// Resets the document (`tsf.edit.reset`). A shown preedit is left
    /// committed by ending its composition in an edit session of its own.
    // [spec:kbdgen:req:tsf.edit.reset+1]
    pub(super) fn reset_document(&self) {
        let ending = {
            let Ok(mut inner) = self.shared.try_borrow_mut() else {
                return;
            };
            inner.document.reset(None);
            let composition = inner.composition.take();
            composition.zip(inner.context.clone())
        };
        if let Some((composition, context)) = ending {
            self.request(&context, Task::End(composition), false);
        }
    }

    /// Advises the text-edit and compartment sinks on the focused context,
    /// replacing those on the previous one.
    pub(super) fn follow_focus(&self) {
        let (thread_mgr, old) = {
            let Ok(mut inner) = self.shared.try_borrow_mut() else {
                return;
            };
            (
                inner.thread_mgr.clone(),
                std::mem::take(&mut inner.context_sinks),
            )
        };
        old.into_iter().for_each(Advice::unadvise);
        let Some(context) = thread_mgr
            .and_then(|tm| unsafe { tm.GetFocus() }.ok())
            .and_then(|dm| unsafe { dm.GetTop() }.ok())
        else {
            return;
        };
        let sinks = self.advise_context(&context);
        if let Ok(mut inner) = self.shared.try_borrow_mut() {
            inner.context_sinks = sinks;
        }
    }

    fn activate(&self, thread_mgr: &ITfThreadMgr, client: u32) -> Result<()> {
        let active_flags = thread_mgr
            .cast::<ITfThreadMgrEx>()
            .and_then(|tm| unsafe { tm.GetActiveFlags() })
            .unwrap_or(0);
        let categories: Option<ITfCategoryMgr> =
            unsafe { CoCreateInstance(&CLSID_TF_CategoryMgr, None, CLSCTX_INPROC_SERVER) }.ok();
        let attribute = categories
            .and_then(|c| unsafe { c.RegisterGUID(&GUID::from_u128(PREEDIT_ATTRIBUTE)) }.ok());
        {
            let mut inner = self.shared.borrow_mut();
            inner.thread_mgr = Some(thread_mgr.clone());
            inner.client = client;
            inner.active_flags = active_flags;
            inner.attribute = attribute;
        }
        let keys: ITfKeystrokeMgr = thread_mgr.cast()?;
        let key_sink: ITfKeyEventSink = self.to_interface();
        unsafe { keys.AdviseKeyEventSink(client, &key_sink, true) }?;
        let source: ITfSource = thread_mgr.cast()?;
        let thread_sink: ITfThreadMgrEventSink = self.to_interface();
        let profile_sink: ITfActiveLanguageProfileNotifySink = self.to_interface();
        let mut sinks = Vec::new();
        for (iid, sink) in [
            (ITfThreadMgrEventSink::IID, thread_sink.cast::<IUnknown>()?),
            (
                ITfActiveLanguageProfileNotifySink::IID,
                profile_sink.cast::<IUnknown>()?,
            ),
        ] {
            let cookie = unsafe { source.AdviseSink(&iid, &sink) }?;
            sinks.push(Advice {
                source: source.clone(),
                cookie,
            });
        }
        self.shared.borrow_mut().thread_sinks = sinks;
        self.select_profile(active_profile());
        self.follow_focus();
        Ok(())
    }

    fn deactivate(&self) {
        self.reset_document();
        self.unpreserve();
        let (thread_mgr, client, sinks) = {
            let Ok(mut inner) = self.shared.try_borrow_mut() else {
                return;
            };
            let mut sinks = std::mem::take(&mut inner.thread_sinks);
            sinks.append(&mut inner.context_sinks);
            let taken = (inner.thread_mgr.take(), inner.client, sinks);
            *inner = Inner::default();
            taken
        };
        sinks.into_iter().for_each(Advice::unadvise);
        if let Some(keys) = thread_mgr.and_then(|tm| tm.cast::<ITfKeystrokeMgr>().ok()) {
            let _ = unsafe { keys.UnadviseKeyEventSink(client) };
        }
    }

    pub(super) fn composition_sink(&self) -> ITfCompositionSink {
        self.to_interface()
    }
}

impl ITfTextInputProcessor_Impl for Tip_Impl {
    fn Activate(&self, ptim: Ref<ITfThreadMgr>, tid: u32) -> Result<()> {
        contain(&POISON, Entry::Activate, || {
            let thread_mgr = ptim.ok()?;
            self.activate(thread_mgr, tid)
                .inspect_err(|_| self.deactivate())
        })
    }

    // [spec:kbdgen:req:tsf.component.interfaces+2]
    fn Deactivate(&self) -> Result<()> {
        contain(&POISON, Entry::Deactivate, || {
            self.deactivate();
            Ok(())
        })
    }
}

impl ITfTextInputProcessorEx_Impl for Tip_Impl {
    // [spec:kbdgen:req:tsf.component.interfaces+2]
    // [spec:kbdgen:req:tsf.security.secure-mode+1]
    fn ActivateEx(&self, ptim: Ref<ITfThreadMgr>, tid: u32, _flags: u32) -> Result<()> {
        contain(&POISON, Entry::ActivateEx, || {
            let thread_mgr = ptim.ok()?;
            self.activate(thread_mgr, tid)
                .inspect_err(|_| self.deactivate())
        })
    }
}
