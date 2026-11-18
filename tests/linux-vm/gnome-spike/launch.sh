#!/bin/sh
# launch.sh CMD... -- run a Wayland client of the spike shell in the background.
. "$(dirname "$(readlink -f "$0")")/env.sh"
setsid -f "$@" > ~/spike-launch-$(basename "$1").log 2>&1 < /dev/null
