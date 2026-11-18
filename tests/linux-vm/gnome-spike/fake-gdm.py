#!/usr/bin/env python3
"""GDM stand-in for the lock-screen test (no GDM runs in the headless VM).

Owns org.gnome.DisplayManager on the system bus: Manager.Version makes
gnome-shell's LoginManager.canLock() true, so it builds its screen shield;
OpenReauthenticationChannel hands out a peer-to-peer address whose
/org/gnome/DisplayManager/Session UserVerifier asks "gdm-password" for a
secret and accepts the password "start" (typeable on the test-se OSK), so the unlock prompt's password
entry stays up and unlocking works. Other services report unavailable."""
import os
import sys
from gi.repository import Gio, GLib

PASSWORD = 'start'
MANAGER_XML = '''<node><interface name="org.gnome.DisplayManager.Manager">
<method name="RegisterDisplay"><arg name="details" type="a{ss}" direction="in"/></method>
<method name="RegisterSession"><arg name="details" type="a{sv}" direction="in"/></method>
<method name="OpenReauthenticationChannel"><arg name="username" type="s" direction="in"/><arg name="address" type="s" direction="out"/></method>
<property name="Version" type="s" access="read"/>
</interface></node>'''
VERIFIER_XML = '''<node><interface name="org.gnome.DisplayManager.UserVerifier">
<method name="EnableExtensions"><arg name="extensions" type="as" direction="in"/></method>
<method name="BeginVerification"><arg name="service_name" type="s" direction="in"/></method>
<method name="BeginVerificationForUser"><arg name="service_name" type="s" direction="in"/><arg name="username" type="s" direction="in"/></method>
<method name="AnswerQuery"><arg name="service_name" type="s" direction="in"/><arg name="answer" type="s" direction="in"/></method>
<method name="Cancel"/>
</interface>
<interface name="org.gnome.DisplayManager.UserVerifier.ChoiceList">
<method name="SelectChoice"><arg name="service_name" type="s" direction="in"/><arg name="choice" type="s" direction="in"/></method>
</interface></node>'''
PATH = '/org/gnome/DisplayManager/Session'
IFACE = 'org.gnome.DisplayManager.UserVerifier'


def log(msg):
    print(msg, file=sys.stderr, flush=True)


def emit(conn, name, *args):
    sig = '(' + 's' * len(args) + ')'
    conn.emit_signal(None, PATH, IFACE, name, GLib.Variant(sig, args) if args else None)


def verifier_call(conn, sender, path, iface, method, params, inv):
    args = params.unpack()
    log(f'verifier {method}{args}')
    inv.return_value(None)
    if method in ('BeginVerification', 'BeginVerificationForUser'):
        service = args[0]
        if service != 'gdm-password':
            GLib.idle_add(lambda: emit(conn, 'ServiceUnavailable', service, '') and False)
            return
        GLib.idle_add(lambda: (emit(conn, 'ConversationStarted', service),
                               emit(conn, 'SecretInfoQuery', service, 'Password:')) and False)
    elif method == 'AnswerQuery':
        service, answer = args
        if answer == PASSWORD:
            GLib.idle_add(lambda: emit(conn, 'VerificationComplete', service) and False)
        else:
            GLib.idle_add(lambda: (emit(conn, 'Problem', service, 'Sorry, that didn’t work.'),
                                   emit(conn, 'VerificationFailed', service)) and False)


CONNECTIONS = []  # GDBusServer leaves keeping the connection alive to us


def new_connection(server, conn):
    CONNECTIONS.append(conn)
    node = Gio.DBusNodeInfo.new_for_xml(VERIFIER_XML)
    for iface in node.interfaces:
        conn.register_object(PATH, iface, verifier_call, None, None)
    def trace(c, msg, incoming):
        log(f"{'<-' if incoming else '->'} {msg.get_message_type().value_nick} {msg.get_interface()}.{msg.get_member()} {msg.get_path()} {msg.get_body()}")
        return msg
    conn.add_filter(trace)
    log('reauthentication channel connected')
    return True


SOCKET = '/run/fake-gdm-reauth'
try:
    os.unlink(SOCKET)
except OSError:
    pass
server = Gio.DBusServer.new_sync(f'unix:path={SOCKET}', Gio.DBusServerFlags.NONE,
                                 Gio.dbus_generate_guid(), None, None)
server.connect('new-connection', new_connection)
server.start()
os.chmod(SOCKET, 0o666)


def manager_call(conn, sender, path, iface, method, params, inv):
    log(f'manager {method}')
    if method == 'OpenReauthenticationChannel':
        inv.return_value(GLib.Variant('(s)', (server.get_client_address(),)))
    else:
        inv.return_value(None)


def on_bus(conn, name):
    info = Gio.DBusNodeInfo.new_for_xml(MANAGER_XML).interfaces[0]
    conn.register_object('/org/gnome/DisplayManager/Manager', info, manager_call,
                         lambda *a: GLib.Variant('s', '50.0'), None)


Gio.bus_own_name(Gio.BusType.SYSTEM, 'org.gnome.DisplayManager', 0, on_bus, None, None)
GLib.MainLoop().run()
