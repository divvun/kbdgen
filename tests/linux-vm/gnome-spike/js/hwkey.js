// Presses and releases evdev key code ARG on a virtual keyboard (a "hardware" key).
(async () => {
    const {default: Clutter} = await import('gi://Clutter');
    const {default: GLib} = await import('gi://GLib');
    const seat = global.stage.context.get_backend().get_default_seat();
    globalThis._spikeKbd ??= seat.create_virtual_device(Clutter.InputDeviceType.KEYBOARD_DEVICE);
    const k = globalThis._spikeKbd;
    const wait = ms => new Promise(r => GLib.timeout_add(GLib.PRIORITY_DEFAULT, ms, () => { r(); return GLib.SOURCE_REMOVE; }));
    k.notify_key(GLib.get_monotonic_time(), Number(ARG), Clutter.KeyState.PRESSED);
    await wait(40);
    k.notify_key(GLib.get_monotonic_time(), Number(ARG), Clutter.KeyState.RELEASED);
    await wait(300);
    return `hardware key ${ARG}`;
})()
