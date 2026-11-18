// Clicks with a virtual pointer at ARG = "x,y" (stage coordinates).
(async () => {
    const {default: Clutter} = await import('gi://Clutter');
    const {default: GLib} = await import('gi://GLib');
    const seat = global.stage.context.get_backend().get_default_seat();
    globalThis._spikePointer ??= seat.create_virtual_device(Clutter.InputDeviceType.POINTER_DEVICE);
    const p = globalThis._spikePointer;
    const [x, y] = ARG.split(',').map(Number);
    const wait = ms => new Promise(r => GLib.timeout_add(GLib.PRIORITY_DEFAULT, ms, () => { r(); return GLib.SOURCE_REMOVE; }));
    p.notify_absolute_motion(GLib.get_monotonic_time(), x, y);
    await wait(50);
    p.notify_button(GLib.get_monotonic_time(), Clutter.BUTTON_PRIMARY, Clutter.ButtonState.PRESSED);
    await wait(60);
    p.notify_button(GLib.get_monotonic_time(), Clutter.BUTTON_PRIMARY, Clutter.ButtonState.RELEASED);
    await wait(300);
    return `clicked ${x},${y}`;
})()
