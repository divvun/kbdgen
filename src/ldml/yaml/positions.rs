//! The ISO position names of `keys.iso-order`.

/// The 49 ISO positions of `keys.iso-order` and their scan codes.
const ISO_POSITIONS: [(&str, u8); 49] = [
    ("E00", 0x29),
    ("E01", 0x02),
    ("E02", 0x03),
    ("E03", 0x04),
    ("E04", 0x05),
    ("E05", 0x06),
    ("E06", 0x07),
    ("E07", 0x08),
    ("E08", 0x09),
    ("E09", 0x0A),
    ("E10", 0x0B),
    ("E11", 0x0C),
    ("E12", 0x0D),
    ("D01", 0x10),
    ("D02", 0x11),
    ("D03", 0x12),
    ("D04", 0x13),
    ("D05", 0x14),
    ("D06", 0x15),
    ("D07", 0x16),
    ("D08", 0x17),
    ("D09", 0x18),
    ("D10", 0x19),
    ("D11", 0x1A),
    ("D12", 0x1B),
    ("C01", 0x1E),
    ("C02", 0x1F),
    ("C03", 0x20),
    ("C04", 0x21),
    ("C05", 0x22),
    ("C06", 0x23),
    ("C07", 0x24),
    ("C08", 0x25),
    ("C09", 0x26),
    ("C10", 0x27),
    ("C11", 0x28),
    ("C12", 0x2B),
    ("B00", 0x56),
    ("B01", 0x2C),
    ("B02", 0x2D),
    ("B03", 0x2E),
    ("B04", 0x2F),
    ("B05", 0x30),
    ("B06", 0x31),
    ("B07", 0x32),
    ("B08", 0x33),
    ("B09", 0x34),
    ("B10", 0x35),
    ("B11", 0x73),
];

/// The scan code of an ISO position name of `keys.iso-order`.
pub fn position_scan_code(name: &str) -> Option<u8> {
    ISO_POSITIONS
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, c)| *c)
}
