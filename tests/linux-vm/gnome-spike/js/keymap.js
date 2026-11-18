// The keysyms mutter's current keymap gives at AD01 (xkb 24) and AE12 (xkb 21), level 1.
(() => {
    const km = global.stage.context.get_backend().get_default_seat().get_keymap();
    const r = {};
    for (const [name, code] of [['AD01', 24], ['AE12', 21], ['AD12', 35]]) {
        const [ok, keys, keyvals] = km.get_entries_for_keycode?.(code) ?? [false];
        r[name] = ok ? keyvals.map(k => k.toString(16)).join(',') : 'n/a';
    }
    return r;
})()
