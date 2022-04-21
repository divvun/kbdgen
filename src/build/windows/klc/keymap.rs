use indexmap::IndexMap;
use once_cell::sync::Lazy;

use crate::iso_key::IsoKey;

pub struct KlcKeyCodes {
    pub scancode: String,    // simply `sc` in .klc format
    pub virtual_key: String, // simply `vk` in .klc format
}

// Order is important as it corresponds to the keymap order in .kbdgen files
pub static MSKLC_KEYS: Lazy<IndexMap<IsoKey, KlcKeyCodes>> = Lazy::new(|| {
    let mut map = IndexMap::new();

    {
        let arr = [
            (IsoKey::E00, ("29", "OEM_3")),
            (IsoKey::E01, ("02", "1")),
            (IsoKey::E02, ("03", "2")),
            (IsoKey::E03, ("04", "3")),
            (IsoKey::E04, ("05", "4")),
            (IsoKey::E05, ("06", "5")),
            (IsoKey::E06, ("07", "6")),
            (IsoKey::E07, ("08", "7")),
            (IsoKey::E08, ("09", "8")),
            (IsoKey::E09, ("0a", "9")),
            (IsoKey::E10, ("0b", "0")),
            (IsoKey::E11, ("0c", "OEM_MINUS")),
            (IsoKey::E12, ("0d", "OEM_PLUS")),
            (IsoKey::D01, ("10", "Q")),
            (IsoKey::D02, ("11", "W")),
            (IsoKey::D03, ("12", "E")),
            (IsoKey::D04, ("13", "R")),
            (IsoKey::D05, ("14", "T")),
            (IsoKey::D06, ("15", "Y")),
            (IsoKey::D07, ("16", "U")),
            (IsoKey::D08, ("17", "I")),
            (IsoKey::D09, ("18", "O")),
            (IsoKey::D10, ("19", "P")),
            (IsoKey::D11, ("1a", "OEM_4")),
            (IsoKey::D12, ("1b", "OEM_6")),
            (IsoKey::C01, ("1e", "A")),
            (IsoKey::C02, ("1f", "S")),
            (IsoKey::C03, ("20", "D")),
            (IsoKey::C04, ("21", "F")),
            (IsoKey::C05, ("22", "G")),
            (IsoKey::C06, ("23", "H")),
            (IsoKey::C07, ("24", "J")),
            (IsoKey::C08, ("25", "K")),
            (IsoKey::C09, ("26", "L")),
            (IsoKey::C10, ("27", "OEM_1")),
            (IsoKey::C11, ("28", "OEM_7")),
            (IsoKey::C12, ("2b", "OEM_5")),
            (IsoKey::B00, ("56", "OEM_102")),
            (IsoKey::B01, ("2c", "Z")),
            (IsoKey::B02, ("2d", "X")),
            (IsoKey::B03, ("2e", "C")),
            (IsoKey::B04, ("2f", "V")),
            (IsoKey::B05, ("30", "B")),
            (IsoKey::B06, ("31", "N")),
            (IsoKey::B07, ("32", "M")),
            (IsoKey::B08, ("33", "OEM_COMMA")),
            (IsoKey::B09, ("34", "OEM_PERIOD")),
            (IsoKey::B10, ("35", "OEM_2")),
        ];

        for (key, value) in arr {
            map.insert(
                key,
                KlcKeyCodes {
                    scancode: value.0.to_owned(),
                    virtual_key: value.1.to_owned(),
                },
            );
        }
    }

    map
});
