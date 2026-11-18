import QtQuick

import org.kde.plasma.keyboard.virtualkeyboard
import org.kde.plasma.keyboard.virtualkeyboard.components
import org.kde.plasma.keyboard.layouts.defaultplugin as SharedLayouts

KeyboardLayoutDefinition {
    symbols: SharedLayouts.Symbols {}
    numbers: SharedLayouts.Numbers {}
    digits: SharedLayouts.Digits {}
    dialpad: SharedLayouts.Dialpad {}

    KeyboardLayoutLoader {
        readonly property real maxWidthToHeightRatio: -1
        sourceComponent: inputEngine && inputEngine.uppercase ? shiftLevel : defaultLevel

        Component {
            id: defaultLevel

            KeyboardLayout {
                KeyboardRow {
                    FillerKey { weight: 100 }
                    Key { text: "´"; displayText: "´" }
                    Key { text: "ŋg" }
                    FillerKey {}
                    FillerKey {}
                    FillerKey {}
                    FillerKey {}
                    FillerKey {}
                    FillerKey {}
                    FillerKey {}
                    FillerKey {}
                    FillerKey {}
                    BackspaceKey { weight: 150 }
                    FillerKey { weight: 100 }
                }

                KeyboardRow {
                    FillerKey { weight: 50 }
                    Key { text: "q"; displayText: "Q̄" }
                    Key { text: "w"; weight: 150 }
                    Key { text: "e"; alternativeKeys: ["é", "ë"] }
                    Key { text: "t"; displayText: "T!" }
                    Key { text: "h"; alternativeKeys: ["w"] }
                    Key { text: "f" }
                    FillerKey {}
                    FillerKey {}
                    FillerKey {}
                    FillerKey {}
                    FillerKey {}
                    FillerKey {}
                    FillerKey {}
                    FillerKey { weight: 50 }
                }

                KeyboardRow {
                    FillerKey { weight: 200 }
                    FillerKey { weight: 50 }
                    Key { text: "s" }
                    FillerKey {}
                    Key { text: "d" }
                    FillerKey {}
                    FillerKey {}
                    FillerKey {}
                    FillerKey {}
                    FillerKey {}
                    FillerKey {}
                    FillerKey {}
                    FillerKey { weight: 200 }
                }

                KeyboardRow {
                    FillerKey { weight: 150 }
                    ShiftKey { weight: 150 }
                    FillerKey {}
                    FillerKey {}
                    FillerKey {}
                    FillerKey {}
                    FillerKey {}
                    FillerKey {}
                    FillerKey {}
                    FillerKey {}
                    FillerKey {}
                    FillerKey {}
                    FillerKey { weight: 150 }
                }

                KeyboardRow {
                    SymbolModeKey { weight: 150 }
                    ChangeLanguageKey {}
                    Key { key: Qt.Key_Comma; text: ","; secondaryStyle: true }
                    SpaceKey { weight: 750; displayText: "gaskoe" }
                    Key { key: Qt.Key_Period; text: "."; alternativeKeys: "!.?"; smallText: "!?"; secondaryStyle: true }
                    HideKeyboardKey {}
                    EnterKey { weight: 150 }
                }
            }
        }

        Component {
            id: shiftLevel

            KeyboardLayout {
                KeyboardRow {
                    Key { text: "ß"; noModifier: true }
                    Key { text: "˝"; noModifier: true }
                    FillerKey {}
                    FillerKey {}
                    FillerKey {}
                    FillerKey {}
                    FillerKey {}
                    FillerKey {}
                    FillerKey {}
                    FillerKey {}
                    FillerKey {}
                    FillerKey {}
                    FillerKey {}
                    BackspaceKey { weight: 150 }
                }

                KeyboardRow {
                    FillerKey { weight: 75 }
                    Key { text: "Q"; noModifier: true }
                    Key { text: "W"; noModifier: true }
                    Key { text: "E"; noModifier: true }
                    FillerKey {}
                    FillerKey {}
                    FillerKey {}
                    FillerKey {}
                    FillerKey {}
                    FillerKey {}
                    FillerKey {}
                    FillerKey {}
                    FillerKey {}
                    FillerKey {}
                    FillerKey { weight: 75 }
                }

                KeyboardRow {
                    FillerKey { weight: 175 }
                    FillerKey {}
                    FillerKey {}
                    FillerKey {}
                    FillerKey {}
                    FillerKey {}
                    FillerKey {}
                    FillerKey {}
                    FillerKey {}
                    FillerKey {}
                    FillerKey {}
                    FillerKey {}
                    FillerKey { weight: 175 }
                }

                KeyboardRow {
                    FillerKey { weight: 150 }
                    ShiftKey { weight: 150 }
                    FillerKey {}
                    FillerKey {}
                    FillerKey {}
                    FillerKey {}
                    FillerKey {}
                    FillerKey {}
                    FillerKey {}
                    FillerKey {}
                    FillerKey {}
                    FillerKey {}
                    FillerKey { weight: 150 }
                }

                KeyboardRow {
                    SymbolModeKey { weight: 150 }
                    ChangeLanguageKey {}
                    Key { key: Qt.Key_Comma; text: ","; secondaryStyle: true }
                    SpaceKey { weight: 750; displayText: "gaskoe" }
                    Key { key: Qt.Key_Period; text: "."; alternativeKeys: "!.?"; smallText: "!?"; secondaryStyle: true }
                    HideKeyboardKey {}
                    EnterKey { weight: 150 }
                }
            }
        }
    }
}
