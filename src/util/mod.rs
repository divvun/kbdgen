use once_cell::sync::Lazy;
use regex::Regex;

pub mod iso_key;

pub static UNICODE_ESCAPES: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\\u\{([0-9A-Fa-f]{1,6})\}").expect("valid regex"));

pub const TRANSFORM_ESCAPE: &str = " ";

pub fn split_keys(layer: &str) -> Vec<String> {
    layer.split_whitespace().map(|v| v.to_string()).collect()
}

pub fn decode_unicode_escapes(input: &str) -> String {
    let new = UNICODE_ESCAPES.replace_all(input, |hex: &regex::Captures| {
        let number = u32::from_str_radix(hex.get(1).unwrap().as_str(), 16).unwrap_or(0xfeff);
        std::char::from_u32(number).unwrap().to_string()
    });

    new.to_string()
}
