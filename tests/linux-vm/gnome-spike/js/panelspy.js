// Logs input-panel-state requests from the input method to the journal/log.
(() => {
    if (globalThis._spikePanelSpy) return 'already';
    globalThis._spikePanelSpy = Main.inputMethod.connect('input-panel-state', (o, s) => console.log(`spike: input-panel-state ${s}`));
    return 'installed';
})()
