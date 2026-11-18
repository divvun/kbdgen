# GNOME on-screen keyboard spike

The experiment behind "GNOME spike results" in `docs/spec/linux.md`. It
checks, on GNOME Shell 50 (Fedora 44) and 51 (Ubuntu 26.10) under headless
Wayland, that a GNOME Shell extension can add on-screen layouts through a
GResource bundle, and how on-screen keys reach an IBus engine.

These scripts are for VMs, not for `cargo test`. With Lima on the host:

```sh
limactl create -y --name=kbdgen-gnome50 --cpus=4 --memory=8 --disk=30 \
  --mount-only "$PWD/tests/linux-vm:w" template:fedora-44
limactl create -y --name=kbdgen-gnome51 --cpus=4 --memory=8 --disk=30 \
  --mount-only "$PWD/tests/linux-vm:w" template:experimental/ubuntu-26.10
limactl start kbdgen-gnome50
limactl shell kbdgen-gnome50 -- "$PWD/tests/linux-vm/gnome-spike/install-fedora.sh"
limactl shell kbdgen-gnome50 -- "$PWD/tests/linux-vm/gnome-spike/setup-vm.sh"
limactl shell kbdgen-gnome50 -- "$PWD/tests/linux-vm/gnome-spike/spike-run.sh" g50
```

Use `install-ubuntu.sh` and `g51` for the GNOME 51 VM. `spike-run.sh`
writes a transcript, screenshots and logs to `out/`.

- `run-shell.sh` starts `gnome-shell --headless --wayland --no-x11
  --virtual-monitor 1280x800` under `dbus-run-session`. A helper extension
  turns on unsafe mode, so `eval.py` can call `org.gnome.Shell.Eval`, and
  the scripts in `js/` drive mutter's virtual touchscreen, pointer and
  keyboard.
- `engine/spike-engine.py` is a logging IBus engine and
  `engine/spike-app.py` a GTK 4 text field.
- `fake-gdm.py` stands in for GDM on the system bus, so the screen shield
  can lock and unlock (password `start`).
- `ext-reload/` is the extension shape `linux.osk.gnome` specifies,
  reloading the input sources in `enable()` and `disable()`;
  `ext-fallback/` the untested `Gio.File.new_for_uri` alternative.
