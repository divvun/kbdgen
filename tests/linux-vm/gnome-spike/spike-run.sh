#!/bin/sh
# spike-run.sh TAG -- the whole spike in one VM (after setup-vm.sh). Writes
# screenshots to out/TAG-*.png and logs to out/TAG-logs/; prints a transcript.
S=$(dirname "$(readlink -f "$0")")
tag=$1
E=$S/eval.py
X=~/.local/share/gnome-shell/extensions
step() { echo; echo "=== $*"; }
gs() { (. $S/env.sh; gsettings "$@"); }
ext() { (. $S/env.sh; gnome-extensions "$@"); }
keys() { $E $S/js/keys.js; }
shot() { $E $S/js/shot.js $S/out/$tag-$1.png > /dev/null; echo "screenshot out/$tag-$1.png"; }
src() { $E $S/js/source.js "$1"; sleep 1; }
mkdir -p $S/out/$tag-logs; rm -f ~/spike-run-*.log ~/spike-shell.*.log
gnome-shell --version; mutter --version | head -1; ibus version
(rpm -q libxkbcommon glib2 gtk4 gjs xkeyboard-config 2>/dev/null || dpkg-query -W -f '${Package} ${Version}\n' libxkbcommon0 libglib2.0-0t64 libgtk-4-1 gjs xkb-data) | tr '\n' ' '; echo

dbus-run-session -- gsettings set org.gnome.shell enabled-extensions "['spike-helper@no.divvun.spike']"
dbus-run-session -- gsettings set org.gnome.desktop.input-sources sources "[('xkb', 'test-se'), ('ibus', 'test-se'), ('ibus', 'spike-other'), ('xkb', 'us')]"
dbus-run-session -- gsettings set org.gnome.desktop.input-sources mru-sources "[('xkb', 'test-se')]"
$S/start-fake-gdm.sh > /dev/null

step "1a. shell started, extension installed but disabled, OSK open in the app, source xkb:test-se"
$S/restart-session.sh
$E $S/js/state.js | grep -E '"current"|oskVisible|extState'
shot 01-ext-disabled

step "1b. gnome-extensions enable (no restart)"
ext enable osk@no.divvun.spike; ext info osk@no.divvun.spike | grep State
$E $S/js/res.js | tr -d ' \n'; echo
echo "right after enabling:"; keys
step "1c. reload by a source switch: xkb:us, then xkb:test-se"
src xkb:us; keys; src xkb:test-se; keys; shot 02-xkb-test-se
step "1d. IBus source whose engine name and layout are test-se"
src ibus:test-se; keys; $E $S/js/state.js | grep '"current"'; shot 03-ibus-test-se
step "1e. IBus source spike-other, whose layout is test-se"
src ibus:spike-other; keys; shot 04-ibus-spike-other

step "3a. key arrival, IBus source test-se, default level (engine commits keycode-0 text)"
src ibus:test-se
$S/presskeys.sh $tag-ibus-default á ´ e ŋá3 Ⓧ ˛ ˇ '~'
step "3b. shift level"
$S/presskeys.sh $tag-ibus-shift osk-shift-symbolic '`' osk-shift-symbolic '^' osk-shift-symbolic Á osk-shift-symbolic ŊÁ3
step "3c. engine passes everything"
echo pass > ~/spike-engine.mode
$S/presskeys.sh $tag-ibus-pass á Ⓧ ŋá3 osk-enter-symbolic
rm -f ~/spike-engine.mode
step "3d. XKB source: on-screen dead key, then on-screen e, then on-screen dead key + hardware e (evdev 18), then hardware AE12 (13) + e"
src xkb:test-se
$S/presskeys.sh $tag-xkb-dead ´ e | grep -E '^###|app:' | cut -c1-120
$S/presskeys.sh $tag-xkb-dead2 ´ > /dev/null; $E $S/js/hwkey.js 18; sleep 0.5; tail -n 1 ~/spike-app.log | sed 's/scalars.*//'
$E $S/js/hwkey.js 13; $E $S/js/hwkey.js 18; sleep 0.5; tail -n 1 ~/spike-app.log | sed 's/scalars.*//'
shot 05-typed-text

step "2. gnome-extensions disable"
ext disable osk@no.divvun.spike; ext info osk@no.divvun.spike | grep State
$E $S/js/res.js | tr -d ' \n'; echo
echo "right after disabling:"; keys
src ibus:test-se; keys; shot 06-disabled-ibus
src xkb:test-se; keys; shot 07-disabled-xkb

step "reload triggers: enable, then change the source list; disable, change it back; enable, toggle the OSK setting"
ext enable osk@no.divvun.spike; sleep 1; keys
gs set org.gnome.desktop.input-sources sources "[('xkb', 'test-se'), ('ibus', 'test-se'), ('ibus', 'spike-other'), ('xkb', 'us'), ('xkb', 'de')]"; sleep 1.5; keys
ext disable osk@no.divvun.spike; sleep 1
gs set org.gnome.desktop.input-sources sources "[('xkb', 'test-se'), ('ibus', 'test-se'), ('ibus', 'spike-other'), ('xkb', 'us')]"; sleep 1.5; keys
ext enable osk@no.divvun.spike; sleep 1
gs set org.gnome.desktop.a11y.applications screen-keyboard-enabled false; sleep 1
gs set org.gnome.desktop.a11y.applications screen-keyboard-enabled true; sleep 1.5; keys

step "start-up with the extension enabled"
$S/stop-shell.sh; $S/run-shell.sh > /dev/null
for t in 3 6 12 20; do sleep $((t - ${p:-0})); p=$t; echo "t=${t}s ext state $($E -e 'String(Main.extensionManager.lookup("osk@no.divvun.spike")?.state)') $(keys)"; done
echo "InputSourceManager.reload():"
$E -e '(async () => { const m = await import("resource:///org/gnome/shell/ui/status/keyboard.js"); m.getInputSourceManager().reload(); })()' > /dev/null; sleep 1; keys

step "lock screen A: sources xkb:test-se, ibus:test-se, ...; ibus:test-se current"
$S/restart-session.sh > /dev/null
$S/lock-test.sh $tag-A ibus:test-se
$E -e 'Main.screenShield.deactivate(false)' > /dev/null; sleep 2
step "lock screen B: only ibus:test-se"
gs set org.gnome.desktop.input-sources sources "[('ibus', 'test-se')]"; sleep 2
$S/lock-test.sh $tag-B ibus:test-se
$E -e 'Main.screenShield.deactivate(false)' > /dev/null; sleep 2
gs set org.gnome.desktop.input-sources sources "[('xkb', 'test-se'), ('ibus', 'test-se'), ('ibus', 'spike-other'), ('xkb', 'us')]"

step "install into a running shell, then gnome-extensions enable"
rm -rf $X/osk2@no.divvun.spike; cp -r $S/ext/osk@no.divvun.spike $X/osk2@no.divvun.spike
sed -i 's/"osk@no.divvun.spike"/"osk2@no.divvun.spike"/' $X/osk2@no.divvun.spike/metadata.json
ext enable osk2@no.divvun.spike; echo "exit=$?"; gs get org.gnome.shell enabled-extensions
rm -rf $X/osk2@no.divvun.spike

cp ~/spike-run-*.log $S/out/$tag-logs/
cat ~/spike-shell.*.log ~/spike-shell.log > $S/out/$tag-logs/gnome-shell.log 2>/dev/null
sudo journalctl -u fake-gdm --no-pager -o cat > $S/out/$tag-logs/fake-gdm.log
echo; echo "=== done"
