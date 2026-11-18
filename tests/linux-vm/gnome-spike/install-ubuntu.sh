#!/bin/sh
set -ex
export DEBIAN_FRONTEND=noninteractive
sudo -E apt-get update
sudo -E apt-get -y install --no-install-recommends \
  gnome-shell gnome-shell-common mutter ibus gir1.2-ibus-1.0 ibus-gtk4 ibus-gtk3 python3-gi \
  gnome-text-editor gtk-4-examples libglib2.0-bin libglib2.0-dev-bin dbus-daemon dbus-user-session \
  xwayland adwaita-icon-theme fonts-cantarell gnome-settings-daemon gsettings-desktop-schemas \
  libxkbcommon0 libxkbcommon-tools xkb-data imagemagick
