#!/bin/sh
# Run inside the VM after ../gnome-spike/setup-vm.sh: installs build/ as the
# keyboard's .deb or .rpm and the IBus engine's package would
# (linux.install), after removing the spike's own layout, on-screen
# extension and engine.
set -ex
A=$(dirname "$(readlink -f "$0")")
O=$A/build/out
P=no.divvun
rm -rf ~/.config/xkb ~/.local/share/gnome-shell/extensions/osk@no.divvun.spike ~/.cache/ibus
sudo rm -f /usr/share/ibus/component/spike.xml
sudo rm -rf /usr/share/xkeyboard-config.d/$P /usr/share/divvun/keyboards/$P /usr/share/gnome-shell/extensions/osk@$P
sudo mkdir -p /usr/share/xkeyboard-config.d /usr/share/divvun/keyboards /usr/share/gnome-shell/extensions
sudo cp -r $O/xkb /usr/share/xkeyboard-config.d/$P
sudo cp -r $O/ibus /usr/share/divvun/keyboards/$P
sudo cp -r $O/gnome-shell/osk@$P /usr/share/gnome-shell/extensions/
sudo install -m 755 $A/build/ibus-engine-divvun /usr/libexec/ibus-engine-divvun
sed -e 's|@LIBEXECDIR@|/usr/libexec|g' -e 's|@DATADIR@|/usr/share|g' -e 's|@VERSION@|0.1.0|g' \
  $A/build/divvun.xml.in | sudo tee /usr/share/ibus/component/divvun.xml > /dev/null
sudo restorecon -R /usr/share/xkeyboard-config.d /usr/share/divvun /usr/share/gnome-shell/extensions \
  /usr/libexec/ibus-engine-divvun /usr/share/ibus/component 2>/dev/null || true
xkbcli list | grep -A4 "layout: 'divvun-sme'"
xkbcli compile-keymap --layout divvun-sme > /dev/null
/usr/libexec/ibus-engine-divvun --xml --data-dir /usr/share/divvun/keyboards | grep '<name>'
