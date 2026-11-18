# Source inside a VM to talk to the spike shell.
export XDG_RUNTIME_DIR=/run/user/$(id -u)
export DBUS_SESSION_BUS_ADDRESS=$(cat ~/spike-bus)
export WAYLAND_DISPLAY=wayland-0 XDG_SESSION_TYPE=wayland
unset DISPLAY GTK_IM_MODULE
