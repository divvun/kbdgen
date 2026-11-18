#!/bin/sh
# lock-test.sh TAG SOURCE -- with SOURCE current, locks, raises the prompt and
# reports the OSK there; then types "start" + Enter on the OSK to unlock.
S=$(dirname "$(readlink -f "$0")")
. $S/env.sh
tag=$1; src=$2
$S/eval.py $S/js/source.js "$src"; sleep 1
: > ~/spike-engine.log
gdbus call --session --dest org.gnome.ScreenSaver --object-path /org/gnome/ScreenSaver --method org.gnome.ScreenSaver.Lock
sleep 3
$S/eval.py $S/js/tap.js 640,300; sleep 3
$S/eval.py $S/js/state.js | grep -E 'current|oskVisible|purpose|keyFocus|extState|locked|sessionMode|group'
$S/eval.py $S/js/keys.js
$S/eval.py $S/js/shot.js $S/out/$tag-lock-osk.png
$S/presskeys.sh $tag-lock s t a r t osk-enter-symbolic | grep -E '^###|engine:' | head -40
sleep 3
$S/eval.py $S/js/state.js | grep -E 'current|locked|sessionMode'
