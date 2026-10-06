//! GUIDs as `u128` values: the text service's CLSID and the parsing of the
//! GUID strings found in the registry and in the build environment.

use std::fmt::Write;

/// The CLSID every Divvun keyboard's profile names
/// (`{5E668C8A-2FB8-41D2-90B1-9C132653FA9D}`), or the GUID that
/// `KBD_TSF_CLSID` held when the crate was built. The override lets the
/// VM test register a build under a test CLSID (`tsf.test.vm`) beside an
/// installed text service.
// [spec:kbdgen:def:tsf.component]
pub const CLSID: u128 = match option_env!("KBD_TSF_CLSID") {
    Some(text) => match parse(text) {
        Some(clsid) => clsid,
        None => panic!("KBD_TSF_CLSID is not a GUID"),
    },
    None => 0x5E66_8C8A_2FB8_41D2_90B1_9C13_2653_FA9D,
};

/// The display attribute of the preedit (`tsf.edit.preedit`).
pub const PREEDIT_ATTRIBUTE: u128 = 0x9E3B_1F44_7C2D_4B8A_A6E1_57D0_C4F2_8B31;

const fn hex(byte: u8) -> Option<u128> {
    match byte {
        b'0'..=b'9' => Some((byte - b'0') as u128),
        b'a'..=b'f' => Some((byte - b'a' + 10) as u128),
        b'A'..=b'F' => Some((byte - b'A' + 10) as u128),
        _ => None,
    }
}

/// Parses `XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX` in either case, with or
/// without either surrounding brace. The legacy keyboard installer writes
/// `Layout Product Code` without its closing brace (`tsf.data.locate`).
pub const fn parse(text: &str) -> Option<u128> {
    let mut bytes = text.as_bytes();
    if let [b'{', rest @ ..] = bytes {
        bytes = rest;
    }
    if let [rest @ .., b'}'] = bytes {
        bytes = rest;
    }
    if bytes.len() != 36 {
        return None;
    }
    let mut value: u128 = 0;
    let mut position = 0;
    while let [byte, rest @ ..] = bytes {
        if matches!(position, 8 | 13 | 18 | 23) {
            if *byte != b'-' {
                return None;
            }
        } else {
            match hex(*byte) {
                Some(digit) => value = (value << 4) | digit,
                None => return None,
            }
        }
        position += 1;
        bytes = rest;
    }
    Some(value)
}

/// The registry form `{XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX}`, upper case.
pub fn braced(guid: u128) -> String {
    let hex = format!("{guid:032X}");
    let mut text = String::with_capacity(38);
    text.push('{');
    for (i, c) in hex.chars().enumerate() {
        if matches!(i, 8 | 12 | 16 | 20) {
            text.push('-');
        }
        let _ = text.write_char(c);
    }
    text.push('}');
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_braced_and_bare_guids() {
        let guid = 0x5E66_8C8A_2FB8_41D2_90B1_9C13_2653_FA9D;
        assert_eq!(parse("{5E668C8A-2FB8-41D2-90B1-9C132653FA9D}"), Some(guid));
        assert_eq!(parse("5e668c8a-2fb8-41d2-90b1-9c132653fa9d"), Some(guid));
        assert_eq!(parse("{5e668c8a-2fb8-41d2-90b1-9c132653fa9d"), Some(guid));
    }

    #[test]
    fn rejects_malformed_guids() {
        for text in [
            "",
            "{}",
            "5E668C8A2FB841D290B19C132653FA9D",
            "5E668C8A-2FB8-41D2-90B1-9C132653FA9",
            "5E668C8A-2FB8-41D2-90B1-9C132653FA9DX",
            "5E668C8A-2FB8+41D2-90B1-9C132653FA9D",
            "5E668C8G-2FB8-41D2-90B1-9C132653FA9D",
        ] {
            assert_eq!(parse(text), None, "{text:?}");
        }
    }

    #[test]
    fn braced_round_trips_through_parse() {
        let text = braced(CLSID);
        assert_eq!(text.len(), 38);
        assert_eq!(parse(&text), Some(CLSID));
        assert_eq!(
            braced(0x5E66_8C8A_2FB8_41D2_90B1_9C13_2653_FA9D),
            "{5E668C8A-2FB8-41D2-90B1-9C132653FA9D}"
        );
    }
}
