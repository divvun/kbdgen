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
                    Key { text: "|" }
                    Key { text: "1" }
                    Key { text: "2" }
                    Key { text: "3" }
                    Key { text: "4" }
                    Key { text: "5" }
                    Key { text: "6" }
                    Key { text: "7" }
                    Key { text: "8" }
                    Key { text: "9" }
                    Key { text: "0" }
                    Key { text: "+" }
                    Key { text: "\\" }
                    BackspaceKey { weight: 150 }
                }

                KeyboardRow {
                    FillerKey { weight: 125 }
                    Key { text: "á" }
                    Key { text: "š" }
                    Key { text: "e" }
                    Key { text: "r" }
                    Key { text: "t" }
                    Key { text: "y" }
                    Key { text: "u" }
                    Key { text: "i" }
                    Key { text: "o" }
                    Key { text: "p" }
                    Key { text: "å" }
                    Key { text: "ŋ" }
                    FillerKey { weight: 125 }
                }

                KeyboardRow {
                    FillerKey { weight: 125 }
                    Key { text: "a" }
                    Key { text: "s" }
                    Key { text: "d" }
                    Key { text: "f" }
                    Key { text: "g" }
                    Key { text: "h" }
                    Key { text: "j" }
                    Key { text: "k" }
                    Key { text: "l" }
                    Key { text: "ø" }
                    Key { text: "æ" }
                    Key { text: "đ" }
                    FillerKey { weight: 125 }
                }

                KeyboardRow {
                    FillerKey { weight: 100 }
                    ShiftKey { weight: 150 }
                    Key { text: "ž" }
                    Key { text: "z" }
                    Key { text: "č" }
                    Key { text: "c" }
                    Key { text: "v" }
                    Key { text: "b" }
                    Key { text: "n" }
                    Key { text: "m" }
                    Key { text: "," }
                    Key { text: "." }
                    Key { text: "-" }
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
                    Key { text: "§"; noModifier: true }
                    Key { text: "!"; noModifier: true }
                    Key { text: "\""; noModifier: true }
                    Key { text: "#"; noModifier: true }
                    Key { text: "¤"; noModifier: true }
                    Key { text: "%"; noModifier: true }
                    Key { text: "&"; noModifier: true }
                    Key { text: "/"; noModifier: true }
                    Key { text: "("; noModifier: true }
                    Key { text: ")"; noModifier: true }
                    Key { text: "="; noModifier: true }
                    Key { text: "?"; noModifier: true }
                    Key { text: "`"; displayText: "`"; noModifier: true }
                    BackspaceKey { weight: 150 }
                }

                KeyboardRow {
                    FillerKey { weight: 125 }
                    Key { text: "Á"; noModifier: true }
                    Key { text: "Š"; noModifier: true }
                    Key { text: "E"; noModifier: true }
                    Key { text: "R"; noModifier: true }
                    Key { text: "T"; noModifier: true }
                    Key { text: "Y"; noModifier: true }
                    Key { text: "U"; noModifier: true }
                    Key { text: "I"; noModifier: true }
                    Key { text: "O"; noModifier: true }
                    Key { text: "P"; noModifier: true }
                    Key { text: "Å"; noModifier: true }
                    Key { text: "Ŋ"; noModifier: true }
                    FillerKey { weight: 125 }
                }

                KeyboardRow {
                    FillerKey { weight: 125 }
                    Key { text: "A"; noModifier: true }
                    Key { text: "S"; noModifier: true }
                    Key { text: "D"; noModifier: true }
                    Key { text: "F"; noModifier: true }
                    Key { text: "G"; noModifier: true }
                    Key { text: "H"; noModifier: true }
                    Key { text: "J"; noModifier: true }
                    Key { text: "K"; noModifier: true }
                    Key { text: "L"; noModifier: true }
                    Key { text: "Ø"; noModifier: true }
                    Key { text: "Æ"; noModifier: true }
                    Key { text: "Đ"; noModifier: true }
                    FillerKey { weight: 125 }
                }

                KeyboardRow {
                    FillerKey { weight: 100 }
                    ShiftKey { weight: 150 }
                    Key { text: "Ž"; noModifier: true }
                    Key { text: "Z"; noModifier: true }
                    Key { text: "Č"; noModifier: true }
                    Key { text: "C"; noModifier: true }
                    Key { text: "V"; noModifier: true }
                    Key { text: "B"; noModifier: true }
                    Key { text: "N"; noModifier: true }
                    Key { text: "M"; noModifier: true }
                    Key { text: ";"; noModifier: true }
                    Key { text: ":"; noModifier: true }
                    Key { text: "_"; noModifier: true }
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
