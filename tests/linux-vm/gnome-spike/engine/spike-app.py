#!/usr/bin/env python3
"""GTK4 probe: a text view that logs key events it sees and its text."""
import os
import time

import gi
gi.require_version('Gtk', '4.0')
gi.require_version('Gdk', '4.0')
from gi.repository import Gdk, Gtk

LOG = os.path.expanduser('~/spike-app.log')


def log(msg):
    with open(LOG, 'a', encoding='utf-8') as f:
        f.write(f'{time.time():.3f} {msg}\n')


def on_activate(app):
    win = Gtk.ApplicationWindow(application=app, title='Spike app')
    win.set_default_size(1000, 300)
    view = Gtk.TextView()
    view.set_monospace(True)
    buf = view.get_buffer()

    def changed(b):
        text = b.get_text(b.get_start_iter(), b.get_end_iter(), True)
        log(f'text={text!r} scalars={[f"U+{ord(c):04X}" for c in text]}')
    buf.connect('changed', changed)

    keys = Gtk.EventControllerKey()
    keys.set_propagation_phase(Gtk.PropagationPhase.CAPTURE)

    def pressed(_c, keyval, keycode, state):
        log(f'key-pressed keyval=0x{keyval:x} {Gdk.keyval_name(keyval)} keycode={keycode} state={int(state):#x}')
        return False
    keys.connect('key-pressed', pressed)
    win.add_controller(keys)

    click = Gtk.GestureClick()
    click.set_propagation_phase(Gtk.PropagationPhase.CAPTURE)
    click.set_touch_only(False)

    def clicked(g, n, x, y):
        ev = g.get_current_event()
        src = ev.get_device().get_source().value_nick if ev and ev.get_device() else None
        log(f'pressed n={n} x={x:.0f} y={y:.0f} source={src} focus={win.get_focus()}')
    click.connect('pressed', clicked)
    win.add_controller(click)
    win.set_child(view)
    win.present()
    view.grab_focus()
    log('app started')


app = Gtk.Application(application_id='no.divvun.SpikeApp')
app.connect('activate', on_activate)
app.run(None)
