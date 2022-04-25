use crate::bundle::layout::macos::MacOsKbdLayer;

pub fn layer_attributes(layer: MacOsKbdLayer) -> String {
    match layer {
        MacOsKbdLayer::Default => "command?",
        MacOsKbdLayer::Shift => "anyShift caps? command?",
        MacOsKbdLayer::Caps => "caps",
        MacOsKbdLayer::Alt => "anyOption command?",
        MacOsKbdLayer::AltAndShift => "anyOption anyShift caps? command?",
        MacOsKbdLayer::CapsAndAlt => "caps anyOption command?",
        MacOsKbdLayer::Ctrl => "anyShift? caps? anyOption? anyControl",
        MacOsKbdLayer::Cmd => "command",
        MacOsKbdLayer::CmdAndShift => "command anyShift",
        MacOsKbdLayer::CmdAndAlt => todo!(),
        MacOsKbdLayer::CmdAndAltAndShift => todo!(),
    }
    .to_string()
}
