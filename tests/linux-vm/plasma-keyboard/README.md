# plasma-keyboard load check

The check behind "Verified" in `ldml.linux.touch.plasma`
(`docs/spec/ldml/linux.md`). It builds plasma-keyboard from source and
runs the real program on kbdgen's golden packages
(`src/build/linux/osk/plasma/golden/`) under plasma-keyboard's own
mock-compositor test, which speaks `input-method-unstable-v1` to it like
KWin, taps the panel and drives keyboard navigation. So plasma-keyboard
reads each package's `metadata.json` from `XDG_DATA_DIRS`, loads its
`desktop.qml` and commits what its keys type.

It needs no VM, only a container. With Docker or OrbStack on the host and
a plasma-keyboard checkout in `$PK`:

```sh
docker run --name pk-check --platform linux/arm64 \
  -v "$PK:/src:ro" -v "$PWD/tests/linux-vm/plasma-keyboard:/check:ro" \
  fedora:rawhide bash /check/build.sh
docker commit pk-check pk-check:built && docker rm pk-check
docker run --rm --platform linux/arm64 \
  -v "$PWD/tests/linux-vm/plasma-keyboard:/check:ro" \
  -v "$PWD/src/build/linux/osk/plasma/golden:/golden:ro" \
  pk-check:built bash /check/run.sh
```

`build.sh` installs the build dependencies, adapts the test
(`mock-test.py`) and builds and installs plasma-keyboard; `run.sh` installs
the packages and runs the test on each.

Run on 2026-11-18 with plasma-keyboard `1e20db4` on Fedora rawhide (Qt
6.12.0, KF 6.32, libplasma 6.8.3, aarch64), it gave:

| Package | First key tapped | Two navigation steps |
|---|---|---|
| `en` (control) | `q` | `w` |
| `vro` | `ˇ` (its dead key's standalone) | `1` |
| `single` | nothing: a filler pads the row | `ŋg` |
| `mods` | nothing: the row is all gaps | `a` (after backspace) |
| `features` | `´` | `1` |
| `se-NO` | `\|` | `1` |

and for `vro` with its shift key pressed, `~`, then `ˇ` once the shift had
released. No QML error concerned the packages. Before the loader declared
`maxWidthToHeightRatio`, taps on `vro` committed nothing while navigation
worked (`ldml.linux.touch.plasma`).

Pressing the shift key logs `ShiftKey.qml:96: TypeError: Property
'syncDirectShift' … is not a function`: the loader replaces the level, and
with it the shift key, inside the key's own click handler. plasma-keyboard's
Korean layout switches levels the same way, so it should log the same
(not run: the check builds no Korean text composer). The shift still
latches and releases.
