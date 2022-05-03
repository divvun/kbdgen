use indexmap::IndexMap;
use once_cell::sync::Lazy;

use crate::util::iso_key::IsoKey;

pub static CHROMEOS_KEYS: Lazy<IndexMap<IsoKey, String>> = Lazy::new(|| {
    let mut map = IndexMap::new();

    {
        let arr = [
            (IsoKey::E00, "Backquote"),
            (IsoKey::E01, "Digit1"),
            (IsoKey::E02, "Digit2"),
            (IsoKey::E03, "Digit3"),
            (IsoKey::E04, "Digit4"),
            (IsoKey::E05, "Digit5"),
            (IsoKey::E06, "Digit6"),
            (IsoKey::E07, "Digit7"),
            (IsoKey::E08, "Digit8"),
            (IsoKey::E09, "Digit9"),
            (IsoKey::E10, "Digit0"),
            (IsoKey::E11, "Minus"),
            (IsoKey::E12, "Equal"),
            (IsoKey::D01, "KeyQ"),
            (IsoKey::D02, "KeyW"),
            (IsoKey::D03, "KeyE"),
            (IsoKey::D04, "KeyR"),
            (IsoKey::D05, "KeyT"),
            (IsoKey::D06, "KeyY"),
            (IsoKey::D07, "KeyU"),
            (IsoKey::D08, "KeyI"),
            (IsoKey::D09, "KeyO"),
            (IsoKey::D10, "KeyP"),
            (IsoKey::D11, "BracketLeft"),
            (IsoKey::D12, "BracketRight"),
            (IsoKey::C01, "KeyA"),
            (IsoKey::C02, "KeyS"),
            (IsoKey::C03, "KeyD"),
            (IsoKey::C04, "KeyF"),
            (IsoKey::C05, "KeyG"),
            (IsoKey::C06, "KeyH"),
            (IsoKey::C07, "KeyJ"),
            (IsoKey::C08, "KeyK"),
            (IsoKey::C09, "KeyL"),
            (IsoKey::C10, "Semicolon"),
            (IsoKey::C11, "Quote"),
            (IsoKey::C12, "Backslash"),
            (IsoKey::B00, "IntlBackslash"),
            (IsoKey::B01, "KeyZ"),
            (IsoKey::B02, "KeyX"),
            (IsoKey::B03, "KeyC"),
            (IsoKey::B04, "KeyV"),
            (IsoKey::B05, "KeyB"),
            (IsoKey::B06, "KeyN"),
            (IsoKey::B07, "KeyM"),
            (IsoKey::B08, "Comma"),
            (IsoKey::B09, "Period"),
            (IsoKey::B10, "Slash"),
        ];

        for (key, value) in arr {
            map.insert(key, value.to_string());
        }
    }

    map
});
