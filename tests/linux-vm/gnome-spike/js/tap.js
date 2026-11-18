// Taps with a virtual touchscreen at ARG = "x,y" (stage coordinates).
(async () => {
    const {default: Clutter} = await import('gi://Clutter');
    const {default: GLib} = await import('gi://GLib');
    const seat = global.stage.context.get_backend().get_default_seat();
    globalThis._spikeTouch ??= seat.create_virtual_device(Clutter.InputDeviceType.TOUCHSCREEN_DEVICE);
    const t = globalThis._spikeTouch;
    const [x, y] = ARG.split(',').map(Number);
    const wait = ms => new Promise(r => GLib.timeout_add(GLib.PRIORITY_DEFAULT, ms, () => { r(); return GLib.SOURCE_REMOVE; }));
    t.notify_touch_down(GLib.get_monotonic_time(), 0, x, y);
    await wait(60);
    t.notify_touch_up(GLib.get_monotonic_time(), 0);
    await wait(400);
    return `tapped ${x},${y}`;
})()
