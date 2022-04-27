use indexmap::IndexMap;
use once_cell::sync::Lazy;

use crate::util::iso_key::IsoKey;

pub static MACOS_KEYS: Lazy<IndexMap<IsoKey, usize>> = Lazy::new(|| {
    let mut map = IndexMap::new();

    {
        let arr = [
            (IsoKey::E00, 10),
            (IsoKey::E01, 18),
            (IsoKey::E02, 19),
            (IsoKey::E03, 20),
            (IsoKey::E04, 21),
            (IsoKey::E05, 23),
            (IsoKey::E06, 22),
            (IsoKey::E07, 26),
            (IsoKey::E08, 28),
            (IsoKey::E09, 25),
            (IsoKey::E10, 29),
            (IsoKey::E11, 27),
            (IsoKey::E12, 24),
            (IsoKey::D01, 12),
            (IsoKey::D02, 13),
            (IsoKey::D03, 14),
            (IsoKey::D04, 15),
            (IsoKey::D05, 17),
            (IsoKey::D06, 16),
            (IsoKey::D07, 32),
            (IsoKey::D08, 34),
            (IsoKey::D09, 31),
            (IsoKey::D10, 35),
            (IsoKey::D11, 33),
            (IsoKey::D12, 30),
            (IsoKey::C01, 0),
            (IsoKey::C02, 1),
            (IsoKey::C03, 2),
            (IsoKey::C04, 3),
            (IsoKey::C05, 5),
            (IsoKey::C06, 4),
            (IsoKey::C07, 38),
            (IsoKey::C08, 40),
            (IsoKey::C09, 37),
            (IsoKey::C10, 41),
            (IsoKey::C11, 39),
            (IsoKey::B00, 50),
            (IsoKey::B01, 6),
            (IsoKey::B02, 7),
            (IsoKey::B03, 8),
            (IsoKey::B04, 9),
            (IsoKey::B05, 11),
            (IsoKey::B06, 45),
            (IsoKey::B07, 46),
            (IsoKey::B08, 43),
            (IsoKey::B09, 47),
            (IsoKey::B10, 44),
            (IsoKey::B11, 94),
        ];

        for (key, value) in arr {
            map.insert(key, value);
        }
    }

    map
});
