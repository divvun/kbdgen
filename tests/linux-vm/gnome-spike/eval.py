#!/usr/bin/env python3
"""eval.py FILE.js|-e CODE [ARG]: org.gnome.Shell.Eval in the spike shell.
ARG is bound as the JS constant ARG. Prints the JSON result."""
import json, os, sys
from gi.repository import Gio, GLib

with open(os.path.expanduser('~/spike-bus')) as f:
    addr = f.read().strip()
code = sys.argv[2] if sys.argv[1] == '-e' else open(sys.argv[1]).read()
arg = sys.argv[3] if sys.argv[1] == '-e' and len(sys.argv) > 3 else (sys.argv[2] if sys.argv[1] != '-e' and len(sys.argv) > 2 else None)
code = f'const ARG = {json.dumps(arg)};\n{code}'
conn = Gio.DBusConnection.new_for_address_sync(
    addr, Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT | Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION, None, None)
ok, val = conn.call_sync('org.gnome.Shell', '/org/gnome/Shell', 'org.gnome.Shell', 'Eval',
                         GLib.Variant('(s)', (code,)), GLib.VariantType('(bs)'), 0, 60000, None).unpack()
try:
    val = json.dumps(json.loads(val), ensure_ascii=False, indent=1)
except Exception:
    pass
print(('' if ok else 'EVAL FAILED: ') + val)
sys.exit(0 if ok else 1)
