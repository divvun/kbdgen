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
                    FillerKey {}
                    FillerKey {}
                    BackspaceKey { weight: 150 }
                }

                KeyboardRow {
                    FillerKey { weight: 125 }
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
                    FillerKey {}
                    FillerKey { weight: 125 }
                }

                KeyboardRow {
                    FillerKey { weight: 125 }
                    Key { text: "a" }
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
                    FillerKey { weight: 125 }
                }

                KeyboardRow {
                    FillerKey { weight: 100 }
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
                    FillerKey {}
                    FillerKey { weight: 100 }
                }

                KeyboardRow {
                    SymbolModeKey { weight: 150 }
                    ChangeLanguageKey {}
                    Key { key: Qt.Key_Comma; text: ","; secondaryStyle: true }
                    SpaceKey { weight: 750 }
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
                    FillerKey {}
                    FillerKey {}
                    BackspaceKey { weight: 150 }
                }

                KeyboardRow {
                    FillerKey { weight: 125 }
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
                    FillerKey {}
                    FillerKey { weight: 125 }
                }

                KeyboardRow {
                    FillerKey { weight: 125 }
                    Key { text: "A"; noModifier: true }
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
                    FillerKey { weight: 125 }
                }

                KeyboardRow {
                    FillerKey { weight: 100 }
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
                    FillerKey {}
                    FillerKey { weight: 100 }
                }

                KeyboardRow {
                    SymbolModeKey { weight: 150 }
                    ChangeLanguageKey {}
                    Key { key: Qt.Key_Comma; text: ","; secondaryStyle: true }
                    SpaceKey { weight: 750 }
                    Key { key: Qt.Key_Period; text: "."; alternativeKeys: "!.?"; smallText: "!?"; secondaryStyle: true }
                    HideKeyboardKey {}
                    EnterKey { weight: 150 }
                }
            }
        }
    }
}
