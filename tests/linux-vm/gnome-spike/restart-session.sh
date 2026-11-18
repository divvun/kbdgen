#!/bin/sh
# Restarts the spike shell with the probe app open and focused, the OSK up.
S=$(dirname "$(readlink -f "$0")")
$S/stop-shell.sh
cp ~/spike-shell.log ~/spike-shell.$(date +%s).log 2>/dev/null
rm -f ~/spike-engine.log ~/spike-app.log ~/spike-engine.mode
$S/run-shell.sh > /dev/null
sleep 10
$S/launch.sh python3 ~/spike-app.py
sleep 4
$S/eval.py -e 'Main.overview.hide()' > /dev/null
sleep 2
$S/eval.py $S/js/focus-app.js 'Spike app' > /dev/null
# GTK 4.22 asks for the input panel on the first tap, GTK 4.24 on a tap in
# the already focused text view.
for i in 1 2 3; do
  $S/eval.py $S/js/tap.js 600,400 > /dev/null
  sleep 1
  [ "$($S/eval.py -e 'String(Main.keyboard.visible)')" = '"true"' ] && break
done
$S/eval.py $S/js/keys.js
