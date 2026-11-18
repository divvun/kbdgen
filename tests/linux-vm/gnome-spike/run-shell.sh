#!/bin/sh
# Starts a headless Wayland GNOME Shell on a private session bus, in the
# background. Writes the bus address to ~/spike-bus; logs to ~/spike-shell.log.
export XDG_RUNTIME_DIR=/run/user/$(id -u)
export XDG_SESSION_TYPE=wayland XDG_CURRENT_DESKTOP=GNOME XDG_SESSION_DESKTOP=gnome
unset DISPLAY WAYLAND_DISPLAY
rm -f ~/spike-bus
setsid -f dbus-run-session -- sh -c '
  echo "$DBUS_SESSION_BUS_ADDRESS" > ~/spike-bus
  exec gnome-shell --headless --wayland --no-x11 --virtual-monitor 1280x800 '"$*"' > ~/spike-shell.log 2>&1
' < /dev/null > /dev/null 2>&1
for i in $(seq 1 60); do [ -s ~/spike-bus ] && [ -S "$XDG_RUNTIME_DIR/wayland-0" ] && break; sleep 0.5; done
cat ~/spike-bus; ls -l "$XDG_RUNTIME_DIR/wayland-0"
