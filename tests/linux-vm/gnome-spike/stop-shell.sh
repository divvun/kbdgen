#!/bin/sh
pkill -f 'gnome-shell --headless' ; sleep 2; pkill -f ibus-daemon; pkill -f '^python3 .*spike-app'; pkill -f gnome-text-editor; pkill -f 'dbus-daemon --config-file=/usr/share/dbus-1/session.conf' ; sleep 1; true
