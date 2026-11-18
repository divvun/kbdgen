#!/bin/sh
# ev.sh 'JS expression' -- org.gnome.Shell.Eval in the spike shell.
. "$(dirname "$(readlink -f "$0")")/env.sh"
gdbus call --session --dest org.gnome.Shell --object-path /org/gnome/Shell \
  --method org.gnome.Shell.Eval "$1"
