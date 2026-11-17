//! Finding and decoding a profile's keyboard data (`tsf.data.locate`):
//! the layout whose `Layout Product Code` is the profile GUID, its layout
//! DLL's model resource, and a per-process cache of the results. A profile
//! without usable data is inert and keeps why (`tsf.data.inert`).

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use kbd_engine::{Error, Model};
use kbd_model::DecodeError;

use crate::guid;
use crate::keys::AltGrChords;

/// The step of `tsf.data.locate` at which a profile became inert.
// [spec:kbdgen:req:tsf.data.inert]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reason {
    /// No profile of this text service is active.
    NoProfile,
    /// The `Keyboard Layouts` key could not be opened.
    LayoutsUnreadable,
    /// No layout's `Layout Product Code` names the profile.
    NoLayout,
    NoLayoutFile,
    /// A `Layout File` with a directory, drive or `..` in it, which could
    /// load a DLL from outside the system directory.
    LayoutFileNotBare,
    NoSystemDirectory,
    /// `LoadLibraryExW` refused the layout DLL.
    LayoutDllUnloadable,
    /// The layout DLL has no model resource.
    NoModelResource,
    /// The model resource could not be loaded, or is empty.
    ModelResourceUnreadable,
    /// `Model::from_bytes` refused the model's major version.
    ModelVersion,
    /// The resource is not an encoded model.
    ModelMalformed,
    /// The model violates an invariant or needs an engine feature this
    /// build lacks.
    ModelInvalid,
}

impl Reason {
    /// The reason code that diagnostics record (`tsf.component.events`).
    pub const fn code(self) -> &'static str {
        match self {
            Reason::NoProfile => "no-profile",
            Reason::LayoutsUnreadable => "layouts-unreadable",
            Reason::NoLayout => "no-layout",
            Reason::NoLayoutFile => "no-layout-file",
            Reason::LayoutFileNotBare => "layout-file-not-bare",
            Reason::NoSystemDirectory => "no-system-directory",
            Reason::LayoutDllUnloadable => "layout-dll-unloadable",
            Reason::NoModelResource => "no-model-resource",
            Reason::ModelResourceUnreadable => "model-resource-unreadable",
            Reason::ModelVersion => "model-version",
            Reason::ModelMalformed => "model-malformed",
            Reason::ModelInvalid => "model-invalid",
        }
    }
}

/// Why a profile is inert: the step that failed, and the `HRESULT` of the
/// Windows call that failed there, where one did.
// [spec:kbdgen:req:tsf.data.inert]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Inert {
    pub reason: Reason,
    pub hresult: Option<i32>,
}

impl Inert {
    pub const fn failed(reason: Reason, hresult: i32) -> Inert {
        Inert {
            reason,
            hresult: Some(hresult),
        }
    }
}

impl From<Reason> for Inert {
    fn from(reason: Reason) -> Inert {
        Inert {
            reason,
            hresult: None,
        }
    }
}

/// A profile's keyboard: the decoded model, whether Right Alt is AltGr
/// (`tsf.keys.altgr`), and its AltGr chords.
#[derive(Debug)]
pub struct Keyboard {
    pub model: Model,
    pub altgr: bool,
    pub chords: AltGrChords,
}

/// Whether a `Layout Product Code` value names `profile`, ignoring case and
/// surrounding braces.
// [spec:kbdgen:req:tsf.data.locate+1]
pub fn names_profile(value: &str, profile: u128) -> bool {
    guid::parse(value.trim()) == Some(profile)
}

/// Whether a `Layout File` value is a bare file name, which is loaded from
/// the system directory and nowhere else.
// [spec:kbdgen:req:tsf.data.locate+1]
// [spec:kbdgen:req:tsf.component.self-contained]
pub fn is_layout_file(name: &str) -> bool {
    !name.is_empty() && name != "." && name != ".." && !name.contains(['\\', '/', ':', '\0'])
}

/// The `Layout File` of the layout that names the profile, from what the
/// registry gave: `None` when no layout names it, an `HRESULT` when its
/// value could not be read. Only a bare file name is accepted.
// [spec:kbdgen:req:tsf.data.locate+1]
// [spec:kbdgen:req:tsf.data.inert]
pub fn check_layout_file(found: Option<Result<String, i32>>) -> Result<String, Inert> {
    match found {
        None => Err(Reason::NoLayout.into()),
        Some(Err(hresult)) => Err(Inert::failed(Reason::NoLayoutFile, hresult)),
        Some(Ok(file)) if is_layout_file(&file) => Ok(file),
        Some(Ok(_)) => Err(Reason::LayoutFileNotBare.into()),
    }
}

/// Decodes a layout DLL's model resource with `Model::from_bytes`, which
/// refuses an unknown major version. A refused model leaves the profile
/// inert.
// [spec:kbdgen:req:tsf.data.locate+1]
// [spec:kbdgen:req:tsf.data.inert]
// [spec:kbdgen:def:tsf.engine.api]
// [spec:kbdgen:req:ldml.crate.tsf+1]
// [spec:kbdgen:req:tsf.keys.altgr+2]
pub fn decode(bytes: &[u8]) -> Result<Keyboard, Reason> {
    let model = Model::from_bytes(bytes).map_err(|error| match error {
        Error::Decode(DecodeError::UnsupportedVersion(_)) => Reason::ModelVersion,
        Error::Decode(_) => Reason::ModelMalformed,
        Error::Invalid(_) | Error::NormalizationUnsupported => Reason::ModelInvalid,
    })?;
    let chords = AltGrChords::of(&model);
    Ok(Keyboard {
        altgr: !chords.typed.is_empty(),
        model,
        chords,
    })
}

/// The keyboards of the profiles this process has activated, including
/// those found inert, so each is located and decoded once per process.
#[derive(Debug, Default)]
pub struct Keyboards {
    loaded: Mutex<BTreeMap<u128, Result<Arc<Keyboard>, Inert>>>,
}

impl Keyboards {
    pub const fn new() -> Keyboards {
        Keyboards {
            loaded: Mutex::new(BTreeMap::new()),
        }
    }

    /// The keyboard of `profile`, loading its model resource with `load`
    /// the first time.
    pub fn get(
        &self,
        profile: u128,
        load: impl FnOnce() -> Result<Vec<u8>, Inert>,
    ) -> Result<Arc<Keyboard>, Inert> {
        let mut loaded = self
            .loaded
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        loaded
            .entry(profile)
            .or_insert_with(|| {
                let bytes = load()?;
                decode(&bytes).map(Arc::new).map_err(Inert::from)
            })
            .clone()
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use kbd_model::MAJOR_VERSION;

    use super::*;
    use crate::tests::fixture;

    const PROFILE: u128 = 0x1234_5678_9ABC_4DEF_8123_4567_89AB_CDEF;

    // [spec:kbdgen:req:tsf.data.locate+1/test]
    #[test]
    fn product_code_ignores_case_and_braces() {
        for value in [
            "{12345678-9ABC-4DEF-8123-456789ABCDEF}",
            "{12345678-9abc-4def-8123-456789abcdef",
            "12345678-9abc-4def-8123-456789abcdef",
            " {12345678-9ABC-4DEF-8123-456789ABCDEF} ",
        ] {
            assert!(names_profile(value, PROFILE), "{value:?}");
        }
        assert!(!names_profile(
            "{12345678-9ABC-4DEF-8123-456789ABCDEE}",
            PROFILE
        ));
        assert!(!names_profile("", PROFILE));
    }

    // [spec:kbdgen:req:tsf.data.locate+1/test]
    // [spec:kbdgen:req:tsf.component.self-contained/test]
    #[test]
    fn layout_file_must_be_bare_name() {
        assert!(is_layout_file("kbdvro.dll"));
        for name in ["", "..", "C:kbd.dll", "..\\kbd.dll", "x/kbd.dll", "a\0b"] {
            assert!(!is_layout_file(name), "{name:?}");
        }
    }

    // [spec:kbdgen:req:tsf.data.locate+1/test]
    // [spec:kbdgen:req:ldml.crate.tsf+1/test]
    #[test]
    fn decodes_model_and_detects_altgr() {
        let bytes = fixture::keyboard().to_bytes().unwrap();
        let keyboard = decode(&bytes).unwrap();
        assert!(keyboard.altgr);
        let mut plain = fixture::keyboard();
        if let Some(hardware) = plain.hardware.as_mut() {
            hardware.layers.truncate(2);
        }
        let plain = decode(&plain.to_bytes().unwrap()).unwrap();
        assert!(!plain.altgr);
    }

    // [spec:kbdgen:req:tsf.data.locate+1/test]
    // [spec:kbdgen:req:tsf.data.inert/test]
    #[test]
    fn refused_model_leaves_profile_inert() {
        let mut bytes = fixture::keyboard().to_bytes().unwrap();
        bytes[4..6].copy_from_slice(&(MAJOR_VERSION + 1).to_le_bytes());
        assert_eq!(decode(&bytes).unwrap_err(), Reason::ModelVersion);
        assert_eq!(decode(b"not a model").unwrap_err(), Reason::ModelMalformed);
        assert_eq!(decode(b"DVKB").unwrap_err(), Reason::ModelMalformed);
        let mut invalid = fixture::keyboard();
        invalid.context_len = 65;
        assert_eq!(
            decode(&invalid.to_bytes().unwrap()).unwrap_err(),
            Reason::ModelInvalid
        );
    }

    // [spec:kbdgen:req:tsf.data.locate+1/test]
    // [spec:kbdgen:req:tsf.data.inert/test]
    #[test]
    fn layout_file_reasons() {
        assert_eq!(
            check_layout_file(Some(Ok("kbdvro.dll".to_owned()))),
            Ok("kbdvro.dll".to_owned())
        );
        assert_eq!(check_layout_file(None), Err(Reason::NoLayout.into()));
        let missing = 0x8007_0002_u32 as i32;
        assert_eq!(
            check_layout_file(Some(Err(missing))),
            Err(Inert::failed(Reason::NoLayoutFile, missing))
        );
        for file in ["C:\\Windows\\System32\\kbdvro.dll", "..\\kbdvro.dll", ""] {
            assert_eq!(
                check_layout_file(Some(Ok(file.to_owned()))),
                Err(Reason::LayoutFileNotBare.into()),
                "{file:?}"
            );
        }
    }

    // [spec:kbdgen:req:tsf.data.inert/test]
    #[test]
    fn reason_codes_are_distinct_kebab_case() {
        let reasons = [
            Reason::NoProfile,
            Reason::LayoutsUnreadable,
            Reason::NoLayout,
            Reason::NoLayoutFile,
            Reason::LayoutFileNotBare,
            Reason::NoSystemDirectory,
            Reason::LayoutDllUnloadable,
            Reason::NoModelResource,
            Reason::ModelResourceUnreadable,
            Reason::ModelVersion,
            Reason::ModelMalformed,
            Reason::ModelInvalid,
        ];
        for (i, reason) in reasons.iter().enumerate() {
            let code = reason.code();
            assert!(
                code.bytes().all(|b| b.is_ascii_lowercase() || b == b'-'),
                "{code}"
            );
            assert!(
                reasons[..i].iter().all(|other| other.code() != code),
                "{code} twice"
            );
        }
    }

    // [spec:kbdgen:req:tsf.data.locate+1/test]
    // [spec:kbdgen:req:tsf.data.inert/test]
    #[test]
    fn keyboards_load_each_profile_once() {
        let keyboards = Keyboards::new();
        let loads = Cell::new(0);
        let bytes = fixture::keyboard().to_bytes().unwrap();
        for _ in 0..2 {
            let found = keyboards.get(PROFILE, || {
                loads.set(loads.get() + 1);
                Ok(bytes.clone())
            });
            assert!(found.is_ok());
        }
        let unloadable = Inert::failed(Reason::LayoutDllUnloadable, 0x8007_007E_u32 as i32);
        for _ in 0..2 {
            let found = keyboards.get(PROFILE + 1, || {
                loads.set(loads.get() + 1);
                Err(unloadable)
            });
            assert_eq!(found.unwrap_err(), unloadable);
        }
        let found = keyboards.get(PROFILE + 2, || Ok(b"DVKB".to_vec()));
        assert_eq!(found.unwrap_err(), Reason::ModelMalformed.into());
        assert_eq!(loads.get(), 2);
    }
}
