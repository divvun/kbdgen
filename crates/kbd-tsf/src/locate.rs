//! Finding and decoding a profile's keyboard data (`tsf.data.locate`):
//! the layout whose `Layout Product Code` is the profile GUID, its layout
//! DLL's model resource, and a per-process cache of the results.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use kbd_engine::Model;

use crate::guid;
use crate::keys::altgr_layout;

/// A profile's keyboard: the decoded model and whether Right Alt is AltGr.
#[derive(Debug)]
pub struct Keyboard {
    pub model: Model,
    pub altgr: bool,
}

/// Whether a `Layout Product Code` value names `profile`, ignoring case and
/// surrounding braces.
// [spec:kbdgen:req:tsf.data.locate]
pub fn names_profile(value: &str, profile: u128) -> bool {
    guid::parse(value.trim()) == Some(profile)
}

/// Whether a `Layout File` value is a bare file name, which is loaded from
/// the system directory and nowhere else.
// [spec:kbdgen:req:tsf.data.locate]
// [spec:kbdgen:req:tsf.component.self-contained]
pub fn is_layout_file(name: &str) -> bool {
    !name.is_empty() && name != "." && name != ".." && !name.contains(['\\', '/', ':', '\0'])
}

/// Decodes a layout DLL's model resource with `Model::from_bytes`, which
/// refuses an unknown major version. A refused model leaves the profile
/// inert.
// [spec:kbdgen:req:tsf.data.locate]
// [spec:kbdgen:def:tsf.engine.api]
// [spec:kbdgen:req:ldml.crate.tsf+1]
pub fn decode(bytes: &[u8]) -> Option<Keyboard> {
    let model = Model::from_bytes(bytes).ok()?;
    let altgr = altgr_layout(&model);
    Some(Keyboard { model, altgr })
}

/// The keyboards of the profiles this process has activated, including
/// those found inert, so each is located and decoded once per process.
#[derive(Debug, Default)]
pub struct Keyboards {
    loaded: Mutex<BTreeMap<u128, Option<Arc<Keyboard>>>>,
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
        load: impl FnOnce() -> Option<Vec<u8>>,
    ) -> Option<Arc<Keyboard>> {
        let mut loaded = self
            .loaded
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        loaded
            .entry(profile)
            .or_insert_with(|| load().and_then(|bytes| decode(&bytes)).map(Arc::new))
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

    // [spec:kbdgen:req:tsf.data.locate/test]
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

    // [spec:kbdgen:req:tsf.data.locate/test]
    // [spec:kbdgen:req:tsf.component.self-contained/test]
    #[test]
    fn layout_file_must_be_bare_name() {
        assert!(is_layout_file("kbdvro.dll"));
        for name in ["", "..", "C:kbd.dll", "..\\kbd.dll", "x/kbd.dll", "a\0b"] {
            assert!(!is_layout_file(name), "{name:?}");
        }
    }

    // [spec:kbdgen:req:tsf.data.locate/test]
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

    // [spec:kbdgen:req:tsf.data.locate/test]
    #[test]
    fn refused_model_leaves_profile_inert() {
        let mut bytes = fixture::keyboard().to_bytes().unwrap();
        bytes[4..6].copy_from_slice(&(MAJOR_VERSION + 1).to_le_bytes());
        assert!(decode(&bytes).is_none());
        assert!(decode(b"not a model").is_none());
    }

    // [spec:kbdgen:req:tsf.data.locate/test]
    #[test]
    fn keyboards_load_each_profile_once() {
        let keyboards = Keyboards::new();
        let loads = Cell::new(0);
        let bytes = fixture::keyboard().to_bytes().unwrap();
        for _ in 0..2 {
            let found = keyboards.get(PROFILE, || {
                loads.set(loads.get() + 1);
                Some(bytes.clone())
            });
            assert!(found.is_some());
        }
        for _ in 0..2 {
            assert!(
                keyboards
                    .get(PROFILE + 1, || {
                        loads.set(loads.get() + 1);
                        None
                    })
                    .is_none()
            );
        }
        assert_eq!(loads.get(), 2);
    }
}
