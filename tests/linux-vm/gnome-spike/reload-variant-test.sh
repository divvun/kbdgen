#!/bin/sh
# reload-variant-test.sh TAG -- installs ext-reload's extension.js and checks
# the OSK at start-up, on disable and on enable, without any source switch.
S=$(dirname "$(readlink -f "$0")")
X=~/.local/share/gnome-shell/extensions/osk@no.divvun.spike
cp $S/ext-reload/osk@no.divvun.spike/extension.js $X/extension.js
$S/stop-shell.sh
dbus-run-session -- gsettings set org.gnome.shell enabled-extensions "['spike-helper@no.divvun.spike', 'osk@no.divvun.spike']" 2>/dev/null
dbus-run-session -- gsettings set org.gnome.desktop.input-sources mru-sources "[('ibus', 'test-se'), ('xkb', 'test-se')]" 2>/dev/null
echo "== start-up, extension enabled, mru ibus:test-se"
$S/restart-session.sh
$S/eval.py $S/js/state.js | grep '"current"'
: > ~/spike-engine.log
. $S/env.sh
echo "== disable"; gnome-extensions disable osk@no.divvun.spike; sleep 1; $S/eval.py $S/js/keys.js; $S/eval.py $S/js/state.js | grep '"current"'
echo "== enable"; gnome-extensions enable osk@no.divvun.spike; sleep 1; $S/eval.py $S/js/keys.js; $S/eval.py $S/js/state.js | grep '"current"'
$S/eval.py $S/js/shot.js $S/out/$1-reload-variant-enabled.png
echo "== engine calls during disable+enable:"; cut -d' ' -f2- ~/spike-engine.log | tr '\n' ';'; echo
echo "== still types through the engine:"; $S/presskeys.sh $1-reload-variant á | grep engine:
cp ~/spike-run-$1-reload-variant.log $S/out/$1-logs/ 2>/dev/null
cp $S/ext/osk@no.divvun.spike/extension.js $X/extension.js
