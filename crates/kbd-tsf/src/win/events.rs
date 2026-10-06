//! The event sinks that reset the document (`tsf.edit.reset`): focus and
//! context changes, edits the text service did not make, the application
//! ending the composition, compartment changes, and profile switches.

use windows::Win32::UI::TextServices::{
    GUID_COMPARTMENT_EMPTYCONTEXT, GUID_COMPARTMENT_KEYBOARD_DISABLED, GUID_PROP_ATTRIBUTE,
    ITfActiveLanguageProfileNotifySink_Impl, ITfCompartmentEventSink, ITfCompartmentEventSink_Impl,
    ITfCompartmentMgr, ITfComposition, ITfCompositionSink_Impl, ITfContext, ITfDocumentMgr,
    ITfEditRecord, ITfRange, ITfSource, ITfTextEditSink, ITfTextEditSink_Impl,
    ITfThreadMgrEventSink_Impl, TF_GTP_INCL_TEXT,
};
use windows_core::{BOOL, GUID, IUnknown, IUnknownImpl, Interface, Ref, Result};

use crate::guard::{Entry, POISON, contain};
use crate::guid::CLSID;

use super::tip::{Advice, Tip_Impl, same};

/// Whether an edit record reports a text or selection change.
fn changed(record: &ITfEditRecord) -> bool {
    if unsafe { record.GetSelectionStatus() }.is_ok_and(|s| s.as_bool()) {
        return true;
    }
    let Ok(ranges) = (unsafe { record.GetTextAndPropertyUpdates(TF_GTP_INCL_TEXT, &[]) }) else {
        return false;
    };
    let mut range: [Option<ITfRange>; 1] = [None];
    let mut fetched = 0;
    unsafe { ranges.Next(&mut range, &mut fetched) }.is_ok() && fetched > 0
}

fn advise(source: ITfSource, iid: &GUID, sink: &IUnknown) -> Option<Advice> {
    let cookie = unsafe { source.AdviseSink(iid, sink) }.ok()?;
    Some(Advice { source, cookie })
}

impl Tip_Impl {
    /// Advises the text-edit sink on `context` and the compartment sink on
    /// its keyboard-disabled and empty-context compartments.
    // [spec:kbdgen:req:tsf.component.interfaces+2]
    pub(super) fn advise_context(&self, context: &ITfContext) -> Vec<Advice> {
        let edit: ITfTextEditSink = self.to_interface();
        let compartment: ITfCompartmentEventSink = self.to_interface();
        let mut sinks = Vec::new();
        if let (Ok(source), Ok(sink)) = (context.cast::<ITfSource>(), edit.cast::<IUnknown>()) {
            sinks.extend(advise(source, &ITfTextEditSink::IID, &sink));
        }
        let (Ok(mgr), Ok(sink)) = (
            context.cast::<ITfCompartmentMgr>(),
            compartment.cast::<IUnknown>(),
        ) else {
            return sinks;
        };
        for guid in [
            GUID_COMPARTMENT_KEYBOARD_DISABLED,
            GUID_COMPARTMENT_EMPTYCONTEXT,
        ] {
            let source = unsafe { mgr.GetCompartment(&guid) }.and_then(|c| c.cast::<ITfSource>());
            if let Ok(source) = source {
                sinks.extend(advise(source, &ITfCompartmentEventSink::IID, &sink));
            }
        }
        sinks
    }

    fn refocus(&self) {
        self.reset_document();
        self.follow_focus();
    }
}

impl ITfThreadMgrEventSink_Impl for Tip_Impl {
    fn OnInitDocumentMgr(&self, _pdim: Ref<ITfDocumentMgr>) -> Result<()> {
        contain(&POISON, Entry::OnInitDocumentMgr, || Ok(()))
    }

    fn OnUninitDocumentMgr(&self, _pdim: Ref<ITfDocumentMgr>) -> Result<()> {
        contain(&POISON, Entry::OnUninitDocumentMgr, || Ok(()))
    }

    // [spec:kbdgen:req:tsf.edit.reset+1]
    fn OnSetFocus(
        &self,
        _focus: Ref<ITfDocumentMgr>,
        _previous: Ref<ITfDocumentMgr>,
    ) -> Result<()> {
        contain(&POISON, Entry::OnSetFocus, || {
            self.refocus();
            Ok(())
        })
    }

    // [spec:kbdgen:req:tsf.edit.reset+1]
    fn OnPushContext(&self, _pic: Ref<ITfContext>) -> Result<()> {
        contain(&POISON, Entry::OnPushContext, || {
            self.refocus();
            Ok(())
        })
    }

    // [spec:kbdgen:req:tsf.edit.reset+1]
    fn OnPopContext(&self, _pic: Ref<ITfContext>) -> Result<()> {
        contain(&POISON, Entry::OnPopContext, || {
            self.refocus();
            Ok(())
        })
    }
}

impl ITfTextEditSink_Impl for Tip_Impl {
    /// An edit the text service did not make, such as a mouse click or
    /// another input method, resets. Its own edit sessions, and what its
    /// injected input makes the application do, do not.
    // [spec:kbdgen:req:tsf.edit.reset+1]
    // [spec:kbdgen:req:tsf.edit.own]
    fn OnEndEdit(
        &self,
        _pic: Ref<ITfContext>,
        _ecreadonly: u32,
        peditrecord: Ref<ITfEditRecord>,
    ) -> Result<()> {
        contain(&POISON, Entry::OnEndEdit, || {
            let own = match self.shared.try_borrow_mut() {
                Ok(mut inner) if !inner.own_edit && !inner.injected && inner.echoes > 0 => {
                    inner.echoes -= 1;
                    true
                }
                Ok(inner) => inner.own_edit || inner.injected,
                Err(_) => true,
            };
            if !own && peditrecord.as_ref().is_some_and(changed) {
                self.reset_document();
            }
            Ok(())
        })
    }
}

impl ITfCompositionSink_Impl for Tip_Impl {
    /// The application ended the composition: its text stays committed,
    /// undecorated, and the engine resets.
    // [spec:kbdgen:req:tsf.edit.preedit+2]
    // [spec:kbdgen:req:tsf.edit.reset+1]
    fn OnCompositionTerminated(
        &self,
        ecwrite: u32,
        pcomposition: Ref<ITfComposition>,
    ) -> Result<()> {
        contain(&POISON, Entry::OnCompositionTerminated, || {
            let Ok(mut inner) = self.shared.try_borrow_mut() else {
                return Ok(());
            };
            let ours = match (inner.composition.as_ref(), pcomposition.as_ref()) {
                (Some(ours), Some(ended)) => same(ours, ended),
                _ => false,
            };
            if !ours {
                return Ok(());
            }
            if let Some(range) = inner
                .composition
                .take()
                .and_then(|c| unsafe { c.GetRange() }.ok())
                && let Ok(property) = unsafe { range.GetContext() }
                    .and_then(|context| unsafe { context.GetProperty(&GUID_PROP_ATTRIBUTE) })
            {
                let _ = unsafe { property.Clear(ecwrite, &range) };
            }
            inner.document.terminated();
            Ok(())
        })
    }
}

impl ITfCompartmentEventSink_Impl for Tip_Impl {
    // [spec:kbdgen:req:tsf.security.disabled+1]
    fn OnChange(&self, _rguid: *const GUID) -> Result<()> {
        contain(&POISON, Entry::OnCompartmentChange, || {
            self.reset_document();
            Ok(())
        })
    }
}

impl ITfActiveLanguageProfileNotifySink_Impl for Tip_Impl {
    /// Switching between this text service's profiles loads the new
    /// profile's keyboard (`tsf.data.locate`).
    // [spec:kbdgen:req:tsf.data.locate+1]
    fn OnActivated(
        &self,
        clsid: *const GUID,
        guidprofile: *const GUID,
        factivated: BOOL,
    ) -> Result<()> {
        contain(&POISON, Entry::OnActivated, || {
            let ours = unsafe { clsid.as_ref() }.is_some_and(|c| c.to_u128() == CLSID);
            if ours {
                let profile = unsafe { guidprofile.as_ref() }.map(GUID::to_u128);
                self.select_profile(profile.filter(|_| factivated.as_bool()));
            }
            Ok(())
        })
    }
}
