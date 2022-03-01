use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub enum WindowsPlatform {
    #[serde(rename = "win2")]
    WindowsPlatformLayers(WindowsPlatformLayers),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WindowsPlatformLayers {
    default: String,
    shift: String,
}
