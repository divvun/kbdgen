//! The writer of `crate::diag`: the TraceLogging events of `crate::etw`,
//! written with the Win32 ETW functions (`tsf.component.logging`).
//!
//! The provider is registered only while one event is written, without an
//! enable callback, so ETW never holds a pointer into this DLL. The text
//! service has no `DllMain` to unregister from (it exports exactly the four
//! functions of `tsf.component.crate`), and COM can unload it at
//! `CoUninitialize` without asking `DllCanUnloadNow`, so a registration
//! kept between events would leak with every unload. Registering per event
//! costs a few system calls, and no event is written on the key path.
//!
//! Microsoft's `tracelogging` crate is not used: it links
//! `OneCore_apiset.lib`, which moves most of the DLL's `kernel32` imports,
//! the static C runtime's included, to `api-ms-win-core-*` API sets.

use std::cell::Cell;

use windows::Win32::System::Diagnostics::Etw::{
    EVENT_DATA_DESCRIPTOR, EVENT_DATA_DESCRIPTOR_0, EVENT_DATA_DESCRIPTOR_TYPE_EVENT_METADATA,
    EVENT_DATA_DESCRIPTOR_TYPE_NONE, EVENT_DATA_DESCRIPTOR_TYPE_PROVIDER_METADATA,
    EVENT_DESCRIPTOR, EventProviderEnabled, EventProviderSetTraits, EventRegister,
    EventSetInformation, EventUnregister, EventWriteTransfer, REGHANDLE,
};
use windows::Win32::System::LibraryLoader::GetModuleFileNameW;
use windows_core::GUID;

use crate::diag::Event;
use crate::etw::{self, CHANNEL, KEYWORD, PROVIDER_GUID, PROVIDER_METADATA};

thread_local! {
    /// Set while this thread writes. A panic in the writer reaches the
    /// panic hook, which must not write again: the same panic would
    /// follow, and a panic during a panic aborts the host.
    static WRITING: Cell<bool> = const { Cell::new(false) };
}

/// A registration of the provider, unregistered when dropped, also when a
/// write unwinds.
struct Registered(REGHANDLE);

impl Registered {
    fn new() -> Option<Registered> {
        let mut handle = REGHANDLE::default();
        let guid = GUID::from_u128(PROVIDER_GUID);
        let status = unsafe { EventRegister(&guid, None, None, &mut handle) };
        if status != 0 {
            return None;
        }
        let registered = Registered(handle);
        let traits = PROVIDER_METADATA;
        let _ = unsafe {
            EventSetInformation(
                handle,
                EventProviderSetTraits,
                traits.as_ptr().cast(),
                u32::try_from(traits.len()).unwrap_or(0),
            )
        };
        Some(registered)
    }
}

impl Drop for Registered {
    fn drop(&mut self) {
        let _ = unsafe { EventUnregister(self.0) };
    }
}

struct Writing;

impl Drop for Writing {
    fn drop(&mut self) {
        let _ = WRITING.try_with(|writing| writing.set(false));
    }
}

/// Writes `event` if a trace session enables the provider at the event's
/// level.
// [spec:kbdgen:req:tsf.component.logging+1]
// [spec:kbdgen:req:tsf.component.events]
pub fn write(event: Event) {
    if WRITING.try_with(|writing| writing.replace(true)) != Ok(false) {
        return;
    }
    let _writing = Writing;
    let Some(registered) = Registered::new() else {
        return;
    };
    let level = etw::level(&event);
    if !unsafe { EventProviderEnabled(registered.0, level, KEYWORD) } {
        return;
    }
    let encoded = etw::encode(event, process_name);
    let descriptor = EVENT_DESCRIPTOR {
        Channel: CHANNEL,
        Level: encoded.level,
        Keyword: KEYWORD,
        ..EVENT_DESCRIPTOR::default()
    };
    let data = [
        described(
            PROVIDER_METADATA,
            EVENT_DATA_DESCRIPTOR_TYPE_PROVIDER_METADATA,
        ),
        described(&encoded.metadata, EVENT_DATA_DESCRIPTOR_TYPE_EVENT_METADATA),
        described(&encoded.payload, EVENT_DATA_DESCRIPTOR_TYPE_NONE),
    ];
    let used = if encoded.payload.is_empty() { 2 } else { 3 };
    // SAFETY: every descriptor points into `PROVIDER_METADATA` or
    // `encoded`, which outlive the call.
    let _ = unsafe { EventWriteTransfer(registered.0, &descriptor, None, None, data.get(..used)) };
}

fn described(bytes: &[u8], kind: u32) -> EVENT_DATA_DESCRIPTOR {
    EVENT_DATA_DESCRIPTOR {
        Ptr: bytes.as_ptr() as u64,
        Size: u32::try_from(bytes.len()).unwrap_or(0),
        Anonymous: EVENT_DATA_DESCRIPTOR_0 { Reserved: kind },
    }
}

/// The file name of this process's image. The directory is left out: it
/// can name the user.
fn process_name() -> Vec<u16> {
    let mut path = vec![0u16; 32_768];
    let len = unsafe { GetModuleFileNameW(None, &mut path) };
    path.truncate(usize::try_from(len).unwrap_or(0));
    let start = path
        .iter()
        .rposition(|&unit| unit == u16::from(b'\\'))
        .map_or(0, |separator| separator + 1);
    path.split_off(start.min(path.len()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writing_without_a_session_returns() {
        write(Event::KeyboardLoaded);
        assert!(!WRITING.with(Cell::get));
    }

    #[test]
    fn process_name_has_no_directory() {
        let name = String::from_utf16(&process_name()).unwrap();
        assert!(name.to_ascii_lowercase().ends_with(".exe"), "{name}");
        assert!(!name.contains('\\'), "{name}");
    }
}
