#!/bin/sh
# Run inside a spike VM as the default (lima) user: installs the test
# layout, the extensions (the OSK one disabled), the test engine and the
# lock-screen shim, and writes the GNOME settings the spike starts from.
set -ex
S=$(dirname "$(readlink -f "$0")")
# XKB layout, in the user configuration (~/.config/xkb)
mkdir -p ~/.config/xkb/symbols ~/.config/xkb/rules
cp "$S/layout/xkb/symbols/test-se" ~/.config/xkb/symbols/
cp "$S/layout/xkb/rules/evdev.xml" ~/.config/xkb/rules/
# GResource bundle
glib-compile-resources --sourcedir "$S/layout/osk" \
  --target "$S/ext/osk@no.divvun.spike/osk-layouts.gresource" "$S/layout/osk-layouts.gresource.xml"
cp "$S/layout/osk/test-se.json" "$S/ext-fallback/osk@no.divvun.spike/osk-layouts/"
# Extensions
X=~/.local/share/gnome-shell/extensions
mkdir -p $X
rm -rf "$X/osk@no.divvun.spike" "$X/spike-helper@no.divvun.spike"
cp -r "$S/helper/spike-helper@no.divvun.spike" $X/
cp -r "$S/ext/osk@no.divvun.spike" $X/
# IBus engine (system-wide: IBus has no per-user component directory)
sudo install -D -m 755 "$S/engine/spike-engine.py" /usr/local/libexec/spike-engine.py
sudo install -D -m 644 "$S/engine/spike.xml" /usr/share/ibus/component/spike.xml
install -m 755 "$S/engine/spike-app.py" ~/spike-app.py
rm -rf ~/.cache/ibus
# Lock-screen shim: a GDM stand-in on the system bus, and a password
sudo install -D -m 644 "$S/fake-gdm.conf" /etc/dbus-1/system.d/fake-gdm.conf
sudo install -D -m 755 "$S/fake-gdm.py" /usr/local/libexec/fake-gdm.py
sudo systemctl reload dbus 2>/dev/null || sudo systemctl reload dbus-broker 2>/dev/null || true
echo "$(id -un):spike" | sudo chpasswd
# Settings (dconf persists them in ~/.config/dconf/user)
dbus-run-session -- sh -c '
gsettings set org.gnome.shell disable-user-extensions false
gsettings set org.gnome.shell enabled-extensions "[\"spike-helper@no.divvun.spike\"]"
gsettings set org.gnome.shell welcome-dialog-last-shown-version "999"
gsettings set org.gnome.desktop.a11y.applications screen-keyboard-enabled true
gsettings set org.gnome.desktop.session idle-delay 0
gsettings set org.gnome.desktop.input-sources sources "[(\"xkb\", \"test-se\"), (\"ibus\", \"test-se\"), (\"ibus\", \"spike-other\"), (\"xkb\", \"us\")]"
gsettings set org.gnome.desktop.input-sources mru-sources "[(\"xkb\", \"test-se\")]"
gsettings get org.gnome.desktop.input-sources sources
'
