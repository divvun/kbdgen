// Presses the evdev key codes ARG ("100,17"), in order, then releases them in reverse.
(async () => {
    const {default: Clutter} = await import('gi://Clutter');
    const {default: GLib} = await import('gi://GLib');
    const seat = global.stage.context.get_backend().get_default_seat();
    globalThis._spikeKbd ??= seat.create_virtual_device(Clutter.InputDeviceType.KEYBOARD_DEVICE);
    const k = globalThis._spikeKbd;
    const wait = ms => new Promise(r => GLib.timeout_add(GLib.PRIORITY_DEFAULT, ms, () => { r(); return GLib.SOURCE_REMOVE; }));
    const codes = ARG.split(',').map(Number);
    for (const c of codes) { k.notify_key(GLib.get_monotonic_time(), c, Clutter.KeyState.PRESSED); await wait(40); }
    for (const c of codes.reverse()) { k.notify_key(GLib.get_monotonic_time(), c, Clutter.KeyState.RELEASED); await wait(40); }
    await wait(250);
    return `chord ${ARG}`;
})()
