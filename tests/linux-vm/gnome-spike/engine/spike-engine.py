#!/usr/bin/env python3
"""Test IBus engine for the kbdgen GNOME spike: logs every ProcessKeyEvent.

Mode (~/spike-engine.mode): "commit" (default) commits a printable keyval
that arrives with key code 0 and returns true, as linux.ibus.virtual-keys's
Emit does; anything else returns false. "pass" returns false for everything.
"""
import os
import sys
import time

import gi
gi.require_version('IBus', '1.0')
from gi.repository import GLib, GObject, IBus

LOG = os.path.expanduser('~/spike-engine.log')
MODE_FILE = os.path.expanduser('~/spike-engine.mode')
BITS = [(0, 'SHIFT'), (1, 'LOCK'), (2, 'CONTROL'), (3, 'MOD1'), (4, 'MOD2'),
        (5, 'MOD3'), (6, 'MOD4'), (7, 'MOD5'), (25, 'FORWARD'), (26, 'SUPER'),
        (30, 'RELEASE')]


def log(msg):
    with open(LOG, 'a', encoding='utf-8') as f:
        f.write(f'{time.time():.3f} {msg}\n')


def mods(state):
    return '|'.join(n for b, n in BITS if state & (1 << b)) or '0'


def mode():
    try:
        with open(MODE_FILE, encoding='utf-8') as f:
            return f.read().strip() or 'commit'
    except OSError:
        return 'commit'


class SpikeEngine(IBus.Engine):
    __gtype_name__ = 'SpikeEngine'

    def __init__(self, **kw):
        super().__init__(**kw)
        log(f'engine created: {self.get_name()}')

    def do_process_key_event(self, keyval, keycode, state):
        uni = IBus.keyval_to_unicode(keyval)
        printable = bool(uni) and ord(uni) >= 0x20 and ord(uni) != 0x7f
        handled = False
        if keycode == 0 and printable and mode() == 'commit':
            if not state & IBus.ModifierType.RELEASE_MASK:
                self.commit_text(IBus.Text.new_from_string(uni))
            handled = True
        log(f'ProcessKeyEvent keyval=0x{keyval:x} name={IBus.keyval_name(keyval)} '
            f'unicode={uni!r} keycode={keycode} state=0x{state:x} [{mods(state)}] '
            f'-> {handled}')
        return handled

    def do_focus_in(self):
        log('FocusIn')

    def do_focus_out(self):
        log('FocusOut')

    def do_enable(self):
        log('Enable')

    def do_disable(self):
        log('Disable')

    def do_set_content_type(self, purpose, hints):
        log(f'SetContentType purpose={purpose} hints=0x{hints:x}')


def main():
    IBus.init()
    bus = IBus.Bus()
    if not bus.is_connected():
        log('not connected to ibus-daemon')
        sys.exit(1)
    bus.connect('disconnected', lambda *_: loop.quit())
    factory = IBus.Factory.new(bus.get_connection())
    for name in ('test-se', 'spike-other'):
        factory.add_engine(name, GObject.type_from_name('SpikeEngine'))
    if '--ibus' in sys.argv:
        bus.request_name('org.freedesktop.IBus.Spike', 0)
    log(f'engine process started, argv={sys.argv}')
    loop = GLib.MainLoop()
    loop.run()


if __name__ == '__main__':
    main()
