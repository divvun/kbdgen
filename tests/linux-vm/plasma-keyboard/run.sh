#!/bin/bash
# Installs the golden plasma-keyboard packages mounted at /golden as system
# packages and runs plasma-keyboard on each under its mock compositor. The
# stock tests expect `en`'s keys, so they fail; the "Actual" lines say what
# a tap on the first key and two navigation steps committed.
set -o pipefail

packages=/pkgroot/plasma/keyboard/keyboardpackages
mkdir -p "$packages"
for pair in vro:no.divvun.vro single:no.divvun.sma mods:no.divvun.und_x_mods \
  features:no.divvun.sme se-NO:no.divvun.se_NO; do
  name="${pair%%:*}"
  id="${pair#*:}"
  mkdir -p "$packages/$id/contents/layouts"
  cp "/golden/$name.metadata.json" "$packages/$id/metadata.json"
  cp "/golden/$name.desktop.qml" "$packages/$id/contents/layouts/desktop.qml"
done

mkdir -p -m 700 /tmp/xdg
# The test process runs offscreen; plasma-keyboard, its child, inherits the
# environment and must run on the test's Wayland socket.
export XDG_DATA_DIRS=/pkgroot:/usr/share QT_QPA_PLATFORM=wayland \
  QT_FORCE_STDERR_LOGGING=1 LANG=C.UTF-8 XDG_RUNTIME_DIR=/tmp/xdg

run() {
  local layout="$1"
  shift
  echo "=== $layout"
  PK_LAYOUT="$layout" timeout 180 /build/out/bin/mockinputmethodcompositor -platform offscreen "$@" 2>&1 |
    grep -E "^(PASS|FAIL)|Actual|SHIFTED|qml|QML|arning|rror|Totals"
}

run org.kde.plasma.keyboard.en/mobile-qwerty testOnScreenKeyboardCommitsCharacter testKeyboardNavigationCommitsCharacter
for id in no.divvun.vro no.divvun.sma no.divvun.und_x_mods no.divvun.sme no.divvun.se_NO; do
  run "$id/desktop" testOnScreenKeyboardCommitsCharacter testKeyboardNavigationCommitsCharacter
done
# Võro's shift key is the 39th key keyboard navigation reaches.
PK_SHIFT_RIGHTS=39 run no.divvun.vro/desktop testShiftLevel
