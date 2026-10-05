use once_cell::sync::Lazy;
use regex::Regex;

pub mod iso_key;

// [spec:kbdgen:syn:keys.escape+1]
pub static UNICODE_ESCAPES: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\\u\{([0-9A-Fa-f]{1,6})\}").expect("valid regex"));

// [spec:kbdgen:req:layout.transforms.dead-key-entries]
pub const TRANSFORM_ESCAPE: &str = " ";

// [spec:kbdgen:req:keys.iso-order.desktop-layers]
pub fn split_keys(layer: &str) -> Vec<String> {
    layer.split_whitespace().map(|v| v.to_string()).collect()
}

// [spec:kbdgen:syn:keys.escape+1]
pub fn decode_unicode_escapes(input: &str) -> String {
    let new = UNICODE_ESCAPES.replace_all(input, |hex: &regex::Captures| {
        let number = u32::from_str_radix(hex.get(1).unwrap().as_str(), 16).unwrap_or(0xfeff);
        std::char::from_u32(number).unwrap().to_string()
    });

    new.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    // [spec:kbdgen:syn:keys.escape+1/test]
    #[test]
    fn escapes_decode_anywhere_with_either_case() {
        assert_eq!(decode_unicode_escapes("\\u{E1}"), "á");
        assert_eq!(decode_unicode_escapes("\\u{e1}\\u{20}x"), "á x");
        assert_eq!(decode_unicode_escapes("a\\u{1F600}b"), "a😀b");
        assert_eq!(decode_unicode_escapes("\\u{000041}"), "A");
        assert_eq!(decode_unicode_escapes("\\u{10FFFF}"), "\u{10FFFF}");
    }

    // [spec:kbdgen:syn:keys.escape+1/test]
    #[test]
    fn non_matching_escapes_stay_verbatim() {
        for verbatim in [
            "\\u{}",
            "\\u{1234567}",
            "\\u00E1",
            "\\u{G1}",
            "u{41}",
            "\\U{41}",
        ] {
            assert_eq!(decode_unicode_escapes(verbatim), verbatim);
        }
    }

    // [spec:kbdgen:syn:keys.escape+1/test]
    #[test]
    fn non_scalar_escapes_panic() {
        for escape in ["\\u{D800}", "\\u{DFFF}", "\\u{110000}", "\\u{FFFFFF}"] {
            assert!(
                std::panic::catch_unwind(|| decode_unicode_escapes(escape)).is_err(),
                "{escape}"
            );
        }
    }
}
