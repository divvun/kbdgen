#!/bin/sh
# mru-test.sh VARIANT -- start-up with mru-sources [ibus:test-se] and the given
# extension.js (ext or ext-reload); reports the current source and the MRU.
S=$(dirname "$(readlink -f "$0")")
X=~/.local/share/gnome-shell/extensions/osk@no.divvun.spike
cp $S/$1/osk@no.divvun.spike/extension.js $X/extension.js
dbus-run-session -- gsettings set org.gnome.desktop.input-sources mru-sources "[('ibus', 'test-se'), ('xkb', 'test-se')]" 2>/dev/null
$S/stop-shell.sh; $S/run-shell.sh > /dev/null; sleep 12
echo "$1: $($S/eval.py $S/js/state.js | grep '"current"') $($S/eval.py $S/js/keys.js)"
(. $S/env.sh; echo "  mru-sources now: $(gsettings get org.gnome.desktop.input-sources mru-sources)")
cp $S/ext/osk@no.divvun.spike/extension.js $X/extension.js
