// Taps the OSK key labelled ARG (or with icon ARG) on the shown level, by touch.
(async () => {
    const {default: Clutter} = await import('gi://Clutter');
    const {default: GLib} = await import('gi://GLib');
    const kbd = Main.keyboard._keyboard;
    let found = null;
    const walk = a => {
        if (found) return;
        if (a.keyButton) {
            const b = a.keyButton;
            if ((b.label ?? b.get_child()?.icon_name) === ARG) found = b;
            return;
        }
        for (const c of a.get_children()) walk(c);
    };
    walk(kbd._currentPage);
    if (!found) return `no key ${ARG}`;
    const [x, y] = found.get_transformed_position();
    const [w, h] = found.get_transformed_size();
    const seat = global.stage.context.get_backend().get_default_seat();
    globalThis._spikeTouch ??= seat.create_virtual_device(Clutter.InputDeviceType.TOUCHSCREEN_DEVICE);
    const t = globalThis._spikeTouch;
    const wait = ms => new Promise(r => GLib.timeout_add(GLib.PRIORITY_DEFAULT, ms, () => { r(); return GLib.SOURCE_REMOVE; }));
    t.notify_touch_down(GLib.get_monotonic_time(), 0, x + w / 2, y + h / 2);
    await wait(60);
    t.notify_touch_up(GLib.get_monotonic_time(), 0);
    await wait(500);
    return `tapped key ${ARG} at ${Math.round(x + w / 2)},${Math.round(y + h / 2)}`;
})()
