pub mod iso_key;

pub fn split_keys(layer: &str) -> Vec<String> {
    layer.split_whitespace().map(|v| v.to_string()).collect()
}
