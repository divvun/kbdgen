// Labels of the OSK's shown level, without opening anything.
(() => {
    const kbd = Main.keyboard._keyboard;
    if (!kbd) return 'no keyboard';
    const labels = [];
    const walk = a => {
        if (a.keyButton) { labels.push(a.keyButton.label ?? a.keyButton.get_child()?.icon_name ?? '?'); return; }
        for (const c of a.get_children()) walk(c);
    };
    if (kbd._currentPage) walk(kbd._currentPage);
    return `visible=${Main.keyboard.visible} group=${kbd._keyboardController.getCurrentGroup()} keys=${labels.slice(0, 12).join(' ')}`;
})()
