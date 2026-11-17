//! The TraceLogging encoding of diagnostic events (`tsf.component.events`),
//! as `TraceLoggingProvider.h` defines it: each event carries its provider's
//! name and its own name, field names and field types, so a decoder needs
//! no manifest. `win::trace` hands the result to `EventWriteTransfer`.

use crate::diag::{Event, Registration};

/// The provider's name (`tsf.component.logging`).
pub const PROVIDER_NAME: &str = "Divvun.Keyboard.TextService";

/// The provider's GUID: ETW's hash of the upper-cased name, as EventSource
/// and `tracelog -guid *Divvun.Keyboard.TextService` compute it.
pub const PROVIDER_GUID: u128 = 0xB773_2613_C499_504F_2AB7_FF5A_4E51_3A9B;

/// The provider's metadata: its size, little-endian, then its name.
pub const PROVIDER_METADATA: &[u8] = b"\x1e\x00Divvun.Keyboard.TextService\x00";

/// The channel that marks an event as TraceLogging.
pub const CHANNEL: u8 = 11;

/// The one keyword every event carries. A session that filters keywords
/// selects the text service's events with it.
pub const KEYWORD: u64 = 1;

pub const CRITICAL: u8 = 1;
pub const ERROR: u8 = 2;
pub const WARNING: u8 = 3;
pub const INFORMATIONAL: u8 = 4;

/// A field's value. Its TraceLogging in-type and out-type are fixed by the
/// variant.
#[derive(Clone, Copy, Debug)]
enum Value<'a> {
    /// `TlgInUINT32`.
    U32(u32),
    /// `TlgInHEXINT32`.
    Hex32(u32),
    /// `TlgInINT32` shown as `TlgOutHRESULT`.
    HResult(i32),
    /// `TlgInCOUNTEDANSISTRING` shown as `TlgOutUTF8`.
    Utf8(&'a str),
    /// `TlgInCOUNTEDSTRING`: UTF-16.
    Utf16(&'a [u16]),
}

impl Value<'_> {
    /// The field's type in event metadata: the in-type, with bit 7 set
    /// when an out-type follows.
    const fn types(self) -> &'static [u8] {
        match self {
            Value::U32(_) => &[8],
            Value::Hex32(_) => &[20],
            Value::HResult(_) => &[7 | 0x80, 15],
            Value::Utf8(_) => &[23 | 0x80, 35],
            Value::Utf16(_) => &[22],
        }
    }

    fn encode(self, payload: &mut Vec<u8>) {
        match self {
            Value::U32(value) | Value::Hex32(value) => payload.extend(value.to_le_bytes()),
            Value::HResult(value) => payload.extend(value.to_le_bytes()),
            Value::Utf8(text) => counted(payload, text.as_bytes()),
            Value::Utf16(text) => {
                let bytes: Vec<u8> = text.iter().flat_map(|unit| unit.to_le_bytes()).collect();
                counted(payload, &bytes);
            }
        }
    }
}

/// A counted string: its length in bytes as a 16-bit number, then the
/// bytes, cut to an even length that fits.
fn counted(payload: &mut Vec<u8>, bytes: &[u8]) {
    let len = bytes.len().min(usize::from(u16::MAX - 1));
    payload.extend(u16::try_from(len).unwrap_or(0).to_le_bytes());
    payload.extend(bytes.iter().take(len));
}

/// An event ready for `EventWriteTransfer`.
#[derive(Debug, PartialEq, Eq)]
pub struct Encoded {
    pub level: u8,
    /// The event's metadata: its size, a zero tag byte, its name, and each
    /// field's name and type.
    pub metadata: Vec<u8>,
    /// The fields' values, in order.
    pub payload: Vec<u8>,
}

/// The level `event` is written at, which decides whether a session takes
/// it before it is encoded.
pub const fn level(event: &Event) -> u8 {
    match event {
        Event::Activated { .. } | Event::KeyboardLoaded => INFORMATIONAL,
        Event::ActivationFailed { .. } => ERROR,
        Event::KeyboardInert(_) => WARNING,
        Event::Panicked { .. } => CRITICAL,
        Event::Register(outcome) | Event::Unregister(outcome) => {
            if outcome.hresult < 0 {
                ERROR
            } else {
                INFORMATIONAL
            }
        }
    }
}

/// Encodes `event`. `process` gives the image file name that `Activated`
/// records.
// [spec:kbdgen:req:tsf.component.events]
// [spec:kbdgen:req:tsf.component.logging+1]
pub fn encode(event: Event, process: impl FnOnce() -> Vec<u16>) -> Encoded {
    let level = level(&event);
    match event {
        Event::Activated { flags } => {
            let process = process();
            build(
                level,
                "Activated",
                &[
                    ("Flags", Value::Hex32(flags)),
                    ("Process", Value::Utf16(&process)),
                ],
            )
        }
        Event::ActivationFailed { interface, hresult } => build(
            level,
            "ActivationFailed",
            &[
                ("Interface", Value::Utf8(interface)),
                ("HResult", Value::HResult(hresult)),
            ],
        ),
        Event::KeyboardLoaded => build(level, "KeyboardLoaded", &[]),
        Event::KeyboardInert(inert) => build(
            level,
            "KeyboardInert",
            &[
                ("Reason", Value::Utf8(inert.reason.code())),
                ("HResult", Value::HResult(inert.hresult.unwrap_or(0))),
            ],
        ),
        Event::Panicked { entry, location } => build(
            level,
            "Panicked",
            &[
                ("Entry", Value::Utf8(entry.map_or("", |entry| entry.name()))),
                ("File", Value::Utf8(location.map_or("", |at| at.file()))),
                ("Line", Value::U32(location.map_or(0, |at| at.line()))),
                ("Column", Value::U32(location.map_or(0, |at| at.column()))),
            ],
        ),
        Event::Register(outcome) => registration(level, "DllRegisterServer", outcome),
        Event::Unregister(outcome) => registration(level, "DllUnregisterServer", outcome),
    }
}

fn registration(level: u8, name: &str, outcome: Registration) -> Encoded {
    build(
        level,
        name,
        &[
            ("HResult", Value::HResult(outcome.hresult)),
            ("Refusal", Value::Utf8(outcome.refusal.unwrap_or(""))),
            ("File", Value::Utf8(outcome.file.unwrap_or(""))),
        ],
    )
}

fn build(level: u8, name: &str, fields: &[(&str, Value<'_>)]) -> Encoded {
    let mut metadata = vec![0, 0, 0];
    metadata.extend(name.as_bytes());
    metadata.push(0);
    let mut payload = Vec::new();
    for (field, value) in fields {
        metadata.extend(field.as_bytes());
        metadata.push(0);
        metadata.extend(value.types());
        value.encode(&mut payload);
    }
    let size = u16::try_from(metadata.len()).unwrap_or(u16::MAX);
    if let Some(prefix) = metadata.get_mut(..2) {
        prefix.copy_from_slice(&size.to_le_bytes());
    }
    Encoded {
        level,
        metadata,
        payload,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::guard::Entry;
    use crate::locate::{Inert, Reason};
    use std::panic::Location;

    // [spec:kbdgen:req:tsf.component.logging+1/test]
    #[test]
    fn provider_metadata_is_sized_name() {
        let size = usize::from(u16::from_le_bytes([
            PROVIDER_METADATA[0],
            PROVIDER_METADATA[1],
        ]));
        assert_eq!(size, PROVIDER_METADATA.len());
        assert_eq!(&PROVIDER_METADATA[2..size - 1], PROVIDER_NAME.as_bytes());
        assert_eq!(PROVIDER_METADATA[size - 1], 0);
    }

    // [spec:kbdgen:req:tsf.component.events/test]
    #[test]
    fn inert_keyboard_encodes_reason_and_hresult() {
        let inert = Inert::failed(Reason::LayoutDllUnloadable, 0x8007_007E_u32 as i32);
        let encoded = encode(Event::KeyboardInert(inert), Vec::new);
        let mut metadata = vec![0x24, 0, 0];
        metadata.extend(b"KeyboardInert\0Reason\0\x97\x23HResult\0\x87\x0F");
        assert_eq!(metadata.len(), 0x24);
        let mut payload = vec![21, 0];
        payload.extend(b"layout-dll-unloadable");
        payload.extend([0x7E, 0, 0x07, 0x80]);
        assert_eq!(
            encoded,
            Encoded {
                level: WARNING,
                metadata,
                payload,
            }
        );
    }

    // [spec:kbdgen:req:tsf.component.events/test]
    #[test]
    fn activation_encodes_flags_and_process_name() {
        let process: Vec<u16> = "a.exe".encode_utf16().collect();
        let encoded = encode(Event::Activated { flags: 0x8000_0040 }, || process.clone());
        assert_eq!(encoded.level, INFORMATIONAL);
        assert!(
            encoded
                .metadata
                .ends_with(b"Activated\0Flags\0\x14Process\0\x16")
        );
        let mut payload = vec![0x40, 0, 0, 0x80, 10, 0];
        payload.extend(b"a\0.\0e\0x\0e\0");
        assert_eq!(encoded.payload, payload);
    }

    // [spec:kbdgen:req:tsf.component.events/test]
    // [spec:kbdgen:req:tsf.component.logging+1/test]
    #[test]
    fn panic_encodes_entry_and_location_only() {
        let location = Location::caller();
        let encoded = encode(
            Event::Panicked {
                entry: Some(Entry::OnKeyDown),
                location: Some(location),
            },
            || panic!("no process name for a panic"),
        );
        assert_eq!(encoded.level, CRITICAL);
        assert!(
            encoded
                .metadata
                .ends_with(b"Panicked\0Entry\0\x97\x23File\0\x97\x23Line\0\x08Column\0\x08")
        );
        let file = location.file().as_bytes();
        let mut payload = vec![9, 0];
        payload.extend(b"OnKeyDown");
        payload.extend(u16::try_from(file.len()).unwrap().to_le_bytes());
        payload.extend(file);
        payload.extend(location.line().to_le_bytes());
        payload.extend(location.column().to_le_bytes());
        assert_eq!(encoded.payload, payload);
    }

    // [spec:kbdgen:req:tsf.component.events/test]
    #[test]
    fn refusals_are_errors_with_reason_and_file() {
        let refused = Registration {
            hresult: 0x8007_0002_u32 as i32,
            refusal: Some("missing"),
            file: Some("divvun_tip.dll"),
        };
        let encoded = encode(Event::Register(refused), Vec::new);
        assert_eq!(encoded.level, ERROR);
        assert!(
            encoded
                .metadata
                .starts_with(b"\x30\x00\x00DllRegisterServer\0HResult\0\x87\x0f")
        );
        assert!(
            encoded
                .payload
                .ends_with(b"\x07\x00missing\x0e\x00divvun_tip.dll")
        );
        let registered = Registration {
            hresult: 0,
            refusal: None,
            file: None,
        };
        let encoded = encode(Event::Unregister(registered), Vec::new);
        assert_eq!(encoded.level, INFORMATIONAL);
        assert_eq!(encoded.payload, [0, 0, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn loaded_keyboard_has_no_fields() {
        let encoded = encode(Event::KeyboardLoaded, Vec::new);
        assert_eq!(encoded.metadata, b"\x12\x00\x00KeyboardLoaded\0");
        assert!(encoded.payload.is_empty());
    }

    #[test]
    fn long_strings_are_cut_to_fit_their_count() {
        let mut payload = Vec::new();
        counted(&mut payload, &[b'x'; 70_000]);
        assert_eq!(payload.len(), 2 + 65_534);
        assert_eq!(&payload[..2], &65_534_u16.to_le_bytes());
    }
}
