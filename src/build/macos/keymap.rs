
// Order is important as it corresponds to the keymap order in .kbdgen files
// The value is MacOS internal expected key ids
pub static MACOS_KEYS: Lazy<IndexMap<IsoKey, String>> = Lazy::new(|| {
    let mut map = IndexMap::new();

    {
        let arr = [

            (IsoKey::C01, "0"),
            (IsoKey::C02, "1"),
            (IsoKey::C03, "2"),
            (IsoKey::C04, "3"),

            (IsoKey::C05, "5"),
            (IsoKey::C06, "4"),

            (IsoKey::C07, "38"),
            (IsoKey::C08, "40"),
            (IsoKey::C09, "37"),
            (IsoKey::C10, "41"),
            (IsoKey::C11, "39"),


            (IsoKey::B00, "50"),
            (IsoKey::B01, "6"),
            (IsoKey::B02, "7"),
            (IsoKey::B03, "8"),
            (IsoKey::B04, "9"),
            (IsoKey::B05, "11"),
            (IsoKey::B06, "45"),
            (IsoKey::B07, "46"),
            (IsoKey::B08, "43"),
            (IsoKey::B09, "47"),
            (IsoKey::B10, "44"),


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

            

            
        ];

        for (key, value) in arr {
            map.insert(
                key,
                value,
            );
        }
    }

    map
});
