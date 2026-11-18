# GNOME acceptance

Part of `linux.test.acceptance` in `docs/spec/linux.md`, for GNOME: a
keyboard installed as its packages would install it, typed under a headless
GNOME Shell with the XKB source and the IBus source, by hardware keys and by
the on-screen keyboard, into a GTK 4 field. It reuses the VM and the harness
of `../gnome-spike/` (`eval.py`, `js/`, the probe app). Not yet covered: Qt 6,
Chromium, a terminal, XWayland, the lock screen, Plasma and Phosh.

The VM must mount all of `tests/linux-vm`:

```sh
limactl create -y --name=kbdgen-gnome50 --cpus=4 --memory=8 --disk=30 \
  --mount-only "$PWD/tests/linux-vm:w" template:fedora-44
limactl start kbdgen-gnome50
limactl shell kbdgen-gnome50 -- "$PWD/tests/linux-vm/gnome-spike/install-fedora.sh"
limactl shell kbdgen-gnome50 -- "$PWD/tests/linux-vm/gnome-spike/setup-vm.sh"
```

Then, from the repository root:

```sh
tests/linux-vm/gnome-acceptance/prepare.sh
limactl shell kbdgen-gnome50 -- "$PWD/tests/linux-vm/gnome-acceptance/install-system.sh"
limactl shell kbdgen-gnome50 -- "$PWD/tests/linux-vm/gnome-acceptance/run.sh"
```

- `prepare.sh` (host) writes `build/`: the `target linux` output of a bundle
  of the `linux.test.golden` layouts (se-NO, sme, vro), and
  `ibus-engine-divvun` built in Docker. Docker must run aarch64 Linux, as the
  VM does.
- `install-system.sh` (VM) installs it at the paths of `linux.install`, with
  the engine's component file, and checks that libxkbregistry lists the
  layouts from `/usr/share/xkeyboard-config.d` and that the keymap compiles
  with the default include path.
- `run.sh` (VM) enables the extension, as a user would once, types each case
  and compares the field with what `ldml.linux.semantics` and the engine
  predict. With the XKB source a dead key followed by a key it does not
  compose with types nothing, AltGr+t (t with U+0301) types nothing, and an
  on-screen dead key composes only with a hardware key; the IBus source types
  all three as the engine does. It exits non-zero on a mismatch.

A run on 2026-11-18, GNOME Shell 50.6, IBus 1.5.34, libxkbcommon 1.13.1,
xkeyboard-config 2.47 (Fedora 44): 26 cases, 0 failures.
