pub mod iso_key;

pub const TRANSFORM_ESCAPE: &str = " ";

pub fn split_keys(layer: &str) -> Vec<String> {
    layer.split_whitespace().map(|v| v.to_string()).collect()
}
