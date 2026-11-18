#!/bin/sh
set -ex
sudo dnf -y --setopt=install_weak_deps=False install \
  gnome-shell mutter ibus ibus-libs ibus-gtk4 ibus-gtk3 python3-gobject \
  gnome-text-editor gtk4 gtk4-devel-tools glib2-devel dbus-daemon dbus-tools \
  xorg-x11-server-Xwayland adwaita-icon-theme adwaita-sans-fonts abattis-cantarell-vf-fonts \
  gnome-settings-daemon gsettings-desktop-schemas libxkbcommon libxkbcommon-utils \
  ImageMagick
