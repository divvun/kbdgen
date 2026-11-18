#!/bin/sh
# presskeys.sh TAG LABEL... -- taps OSK keys one by one, logging each tap with
# what the engine, the app and the shell logged for it into ~/spike-run-TAG.log.
S=$(dirname "$(readlink -f "$0")")
tag=$1; shift
out=~/spike-run-$tag.log
: > $out
for k in "$@"; do
  e0=$(wc -l < ~/spike-engine.log 2>/dev/null || echo 0); a0=$(wc -l < ~/spike-app.log); s0=$(wc -l < ~/spike-shell.log)
  echo "### tap $k: $($S/eval.py $S/js/tapkey.js "$k")" >> $out
  sleep 0.7
  tail -n +$((e0+1)) ~/spike-engine.log 2>/dev/null | sed 's/^[0-9.]* /  engine: /' >> $out
  tail -n +$((a0+1)) ~/spike-app.log | sed 's/^[0-9.]* /  app:    /' >> $out
  tail -n +$((s0+1)) ~/spike-shell.log | grep -v '^$' | sed 's/^/  shell:  /' >> $out
done
cat $out
