//! The binary form of a keyboard: the engine model of `tsf.data.resource`.

use alloc::vec::Vec;
use core::fmt;

use crate::keyboard::Keyboard;
use crate::validate::{InvariantError, NfdCheck};

/// The ASCII magic that starts every encoded keyboard.
pub const MAGIC: [u8; 4] = *b"DVKB";
/// The major format version this crate reads and writes.
pub const MAJOR_VERSION: u16 = 1;
/// The minor format version this crate writes.
pub const MINOR_VERSION: u16 = 0;
/// The length of the magic and version header.
pub const HEADER_LEN: usize = 8;

/// The format version of an encoded keyboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Version {
    pub major: u16,
    pub minor: u16,
}

/// Why bytes are not a keyboard.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    /// Fewer bytes than the header.
    Truncated,
    /// The bytes do not start with `DVKB`.
    Magic,
    /// A major version this crate does not implement.
    UnsupportedVersion(Version),
    /// The body is not the postcard encoding of a keyboard: truncated,
    /// invalid, or declaring a length beyond the remaining input.
    Postcard(postcard::Error),
    /// The keyboard violates an invariant.
    Invariant(InvariantError),
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DecodeError::Truncated => f.write_str("keyboard model is shorter than its header"),
            DecodeError::Magic => f.write_str("keyboard model does not start with DVKB"),
            DecodeError::UnsupportedVersion(v) => write!(
                f,
                "keyboard model version {}.{} is not supported (major {MAJOR_VERSION} is)",
                v.major, v.minor
            ),
            DecodeError::Postcard(e) => write!(f, "keyboard model body is malformed: {e}"),
            DecodeError::Invariant(e) => write!(f, "keyboard model is invalid: {e}"),
        }
    }
}

impl core::error::Error for DecodeError {}

impl From<InvariantError> for DecodeError {
    fn from(e: InvariantError) -> Self {
        DecodeError::Invariant(e)
    }
}

/// Why a keyboard could not be encoded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodeError(pub postcard::Error);

impl fmt::Display for EncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "keyboard model could not be encoded: {}", self.0)
    }
}

impl core::error::Error for EncodeError {}

/// Reads the header of an encoded keyboard: the magic, then the major and
/// minor versions as little-endian `u16`s.
pub fn read_header(bytes: &[u8]) -> Result<(Version, &[u8]), DecodeError> {
    let Some((header, body)) = bytes.split_first_chunk::<HEADER_LEN>() else {
        return Err(DecodeError::Truncated);
    };
    let [m0, m1, m2, m3, a0, a1, i0, i1] = *header;
    if [m0, m1, m2, m3] != MAGIC {
        return Err(DecodeError::Magic);
    }
    let version = Version {
        major: u16::from_le_bytes([a0, a1]),
        minor: u16::from_le_bytes([i0, i1]),
    };
    Ok((version, body))
}

impl Keyboard {
    // [spec:kbdgen:syn:ldml.model.encoding]
    // [spec:kbdgen:req:ldml.model.deterministic]
    /// The binary form: `DVKB`, major version 1 and minor version 0 as
    /// little-endian `u16`s, then the postcard encoding of the keyboard.
    ///
    /// The model holds only `Vec`, `BTreeMap`, fixed-width integers,
    /// `bool`, `char`, `String` and enums, and postcard writes each in one
    /// platform-independent way, so equal keyboards encode to identical
    /// bytes everywhere. Encoding does not validate; build keyboards
    /// through a validating producer.
    pub fn to_bytes(&self) -> Result<Vec<u8>, EncodeError> {
        let mut out = Vec::new();
        out.extend_from_slice(&MAGIC);
        out.extend_from_slice(&MAJOR_VERSION.to_le_bytes());
        out.extend_from_slice(&MINOR_VERSION.to_le_bytes());
        postcard::to_extend(self, out).map_err(EncodeError)
    }

    // [spec:kbdgen:req:ldml.model.decode]
    /// Decodes and validates a keyboard ([`Keyboard::validate`]).
    ///
    /// Fails, without panicking, on a short header, a wrong magic, a major
    /// version other than 1, malformed postcard data, or a violated
    /// invariant. A higher minor version is accepted, and bytes after the
    /// keyboard, which a later minor version appends, are ignored.
    ///
    /// Allocation is bounded by the input: postcard rejects a string
    /// longer than the remaining input, and hints a sequence's length only
    /// when the remaining input could hold it, so serde preallocates no
    /// more than that (and never past 1 MiB per sequence). No encoded type
    /// is recursive, so decoding uses bounded stack.
    pub fn from_bytes(bytes: &[u8], nfd: Option<&dyn NfdCheck>) -> Result<Keyboard, DecodeError> {
        let (version, body) = read_header(bytes)?;
        if version.major != MAJOR_VERSION {
            return Err(DecodeError::UnsupportedVersion(version));
        }
        let (keyboard, _later_fields) =
            postcard::take_from_bytes::<Keyboard>(body).map_err(DecodeError::Postcard)?;
        keyboard.validate(nfd)?;
        Ok(keyboard)
    }
}
