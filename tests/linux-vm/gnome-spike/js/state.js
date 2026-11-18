// Reports input sources, the OSK's group and the labels of the shown level.
(async () => {
    const {getInputSourceManager} = await import('resource:///org/gnome/shell/ui/status/keyboard.js');
    const ism = getInputSourceManager();
    const kbd = Main.keyboard._keyboard;
    const cur = ism.currentSource;
    const r = {
        sources: Object.values(ism.inputSources).map(s => `${s.type}:${s.id}=>${s.xkbId}`),
        current: cur ? `${cur.type}:${cur.id} xkbId=${cur.xkbId}` : null,
        oskEnabled: !!kbd,
        oskVisible: Main.keyboard.visible,
        purpose: Main.inputMethod.contentPurpose,
        imFocus: !!Main.inputMethod.currentFocus,
        keyFocus: global.stage.key_focus?.toString() ?? null,
        focusWindow: global.display.focus_window?.title ?? null,
        extState: Main.extensionManager.lookup('osk@no.divvun.spike')?.state,
        locked: Main.screenShield?.locked ?? null,
        sessionMode: Main.sessionMode.currentMode,
    };
    if (kbd) {
        r.group = kbd._keyboardController.getCurrentGroup();
        const labels = [];
        const walk = a => {
            if (a.keyButton) {
                const b = a.keyButton;
                labels.push(b.label ?? b.get_child()?.icon_name ?? '?');
                return;
            }
            for (const c of a.get_children()) walk(c);
        };
        if (kbd._currentPage) walk(kbd._currentPage);
        r.shownLevelKeys = labels.join(' ');
    }
    return r;
})()
