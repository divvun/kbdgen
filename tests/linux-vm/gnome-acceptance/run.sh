#!/bin/sh
# Run inside the VM after install-system.sh: types the installed layouts into
# the spike's GTK 4 field under a headless GNOME Shell, by hardware keys and
# on-screen keys, with the XKB source and the IBus source, and compares the
# field's text with what the spec predicts. Exits non-zero on a mismatch.
A=$(dirname "$(readlink -f "$0")")
S=$A/../gnome-spike
V=$S/eval.py
dbus-run-session -- gsettings set org.gnome.shell enabled-extensions "['spike-helper@no.divvun.spike', 'osk@no.divvun']"
dbus-run-session -- gsettings set org.gnome.desktop.input-sources sources "[('xkb', 'divvun-sme'), ('ibus', 'divvun-sme'), ('xkb', 'divvun-vro'), ('ibus', 'divvun-vro'), ('xkb', 'us')]"
dbus-run-session -- gsettings set org.gnome.desktop.input-sources mru-sources "[('xkb', 'divvun-sme')]"
$S/restart-session.sh > /dev/null 2>&1
failures=0
# The field's text, as the probe app last logged it.
text() { sleep 0.6; grep -o "text='[^']*'" ~/spike-app.log | tail -n 1 | sed "s/^text='//; s/'\$//"; }
clear() { $V $A/chord.js 29,30 > /dev/null; $V $S/js/hwkey.js 14 > /dev/null; sleep 0.3; }
src() { $V $S/js/source.js "$1" | tr -d '"'; sleep 1.5; }
keys() { for k in "$@"; do case $k in *,*) $V $A/chord.js $k ;; *) $V $S/js/hwkey.js $k ;; esac > /dev/null; done; }
tap() { for k in "$@"; do $V $S/js/tapkey.js "$k" > /dev/null; done; }
check() {
  got=$(text)
  if [ "$got" = "$2" ]; then r=ok; else r="FAIL, expected '$2'"; failures=$((failures + 1)); fi
  printf '  %-30s %-8s %s\n' "$1" "'$got'" "$r"
}
# case_ LABEL EXPECTED KEY... -- evdev codes; A,B is a chord.
case_() { label=$1; want=$2; shift 2; clear; keys "$@"; check "$label" "$want"; }
for source in xkb:divvun-sme ibus:divvun-sme; do
  echo "== $(src $source)"
  ibus=; [ "${source%%:*}" = ibus ] && ibus=1
  case_ "´ then a" á 41 30
  case_ "´ then 1" "${ibus:+´1}" 41 2
  case_ "AltGr+w" š 100,17
  case_ "AltGr+t (t U+0301)" "${ibus:+t́}" 100,20
  case_ "AltGr+= (˝) then o" ő 100,13 24
  case_ "AltGr+\\\\ (ˇ) then s" š 100,43 31
  case_ "Caps Lock, q, Caps Lock" Č 58 16 58
  case_ "Shift+a" A 42,30
  case_ "Left Alt+a (a shortcut)" "" 56,30
  clear; tap ´ a; check "OSK ´ then OSK a" "$([ -n "$ibus" ] && echo á || echo a)"
  clear; tap ´; keys 30; check "OSK ´ then hardware a" á
done
for source in xkb:divvun-vro ibus:divvun-vro; do
  echo "== $(src $source)"
  case_ "ˇ (TLDE) then s" š 41 31
  case_ "´ (AE12) then e" é 13 18
done
$V $S/js/shot.js $A/build/osk.png > /dev/null && echo "screenshot build/osk.png"
echo "$failures failure(s)"
[ "$failures" -eq 0 ]
