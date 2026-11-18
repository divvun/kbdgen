#!/bin/sh
S=$(dirname "$(readlink -f "$0")")
sudo install -D -m 755 "$S/fake-gdm.py" /usr/local/libexec/fake-gdm.py
sudo systemctl stop fake-gdm 2>/dev/null; sudo systemctl reset-failed fake-gdm 2>/dev/null
sudo systemd-run --unit=fake-gdm /usr/bin/python3 /usr/local/libexec/fake-gdm.py
